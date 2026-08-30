use rusqlite::Connection;

/// Adds a column to an already-existing table exactly once. Unlike `CREATE
/// TABLE IF NOT EXISTS`, `ALTER TABLE ADD COLUMN` has no `IF NOT EXISTS`
/// form in SQLite, so existence is checked manually via `pragma_table_info`
/// before running it.
fn ensure_column(conn: &Connection, table: &str, column: &str, ddl_type: &str) -> rusqlite::Result<()> {
    let has_column: bool = conn
        .prepare(&format!("SELECT 1 FROM pragma_table_info('{table}') WHERE name = '{column}'"))?
        .exists([])?;
    if !has_column {
        conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {ddl_type}"), [])?;
    }
    Ok(())
}

/// Forward-only, idempotent migrations — safe to run on every startup.
pub fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS playlists (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            filename TEXT,
            playlist_type TEXT NOT NULL CHECK (playlist_type IN ('m3u', 'xtream', 'stalker')),
            import_date TEXT NOT NULL,
            last_usage TEXT NOT NULL,
            count INTEGER NOT NULL DEFAULT 0,
            url TEXT,
            user_agent TEXT,
            referrer TEXT,
            origin TEXT,
            file_path TEXT,
            epg_urls TEXT,
            detected_epg_urls TEXT,
            manual_epg_urls TEXT,
            disabled_epg_urls TEXT,
            auto_refresh INTEGER NOT NULL DEFAULT 0,
            update_date TEXT,
            update_state TEXT,
            position INTEGER,
            is_temporary INTEGER,
            server_url TEXT,
            username TEXT,
            password TEXT,
            mac_address TEXT,
            portal_url TEXT,
            is_full_stalker_portal INTEGER,
            server_timezone TEXT,
            stalker_token TEXT,
            stalker_session_identity TEXT,
            stalker_watchdog_timeout INTEGER,
            stalker_timeslot INTEGER,
            stalker_serial_number TEXT,
            stalker_device_id1 TEXT,
            stalker_device_id2 TEXT,
            stalker_signature1 TEXT,
            stalker_signature2 TEXT,
            stalker_account_info TEXT,
            hidden_group_titles TEXT,
            stalker_login_completed INTEGER,
            stalker_not_valid INTEGER,
            stalker_endpoint TEXT
        );

        CREATE TABLE IF NOT EXISTS channels (
            id TEXT PRIMARY KEY,
            playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            url TEXT NOT NULL,
            group_title TEXT,
            tvg_id TEXT,
            tvg_name TEXT,
            tvg_url TEXT,
            tvg_logo TEXT,
            tvg_rec TEXT,
            http_referrer TEXT,
            http_user_agent TEXT,
            http_origin TEXT,
            radio TEXT NOT NULL DEFAULT '0',
            catchup_type TEXT,
            catchup_source TEXT,
            catchup_days TEXT,
            timeshift TEXT,
            drm_json TEXT,
            raw_json TEXT,
            channel_number INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_channels_playlist ON channels(playlist_id);
        CREATE INDEX IF NOT EXISTS idx_channels_tvg_id ON channels(tvg_id);

        CREATE VIRTUAL TABLE IF NOT EXISTS channels_fts USING fts5(
            name,
            content = 'channels',
            content_rowid = 'rowid',
            tokenize = 'trigram remove_diacritics 1'
        );
        CREATE TRIGGER IF NOT EXISTS channels_ai AFTER INSERT ON channels BEGIN
            INSERT INTO channels_fts(rowid, name) VALUES (new.rowid, new.name);
        END;
        CREATE TRIGGER IF NOT EXISTS channels_ad AFTER DELETE ON channels BEGIN
            INSERT INTO channels_fts(channels_fts, rowid, name) VALUES ('delete', old.rowid, old.name);
        END;
        CREATE TRIGGER IF NOT EXISTS channels_au AFTER UPDATE ON channels BEGIN
            INSERT INTO channels_fts(channels_fts, rowid, name) VALUES ('delete', old.rowid, old.name);
            INSERT INTO channels_fts(rowid, name) VALUES (new.rowid, new.name);
        END;

        CREATE TABLE IF NOT EXISTS epg_channels (
            id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            icon_url TEXT,
            source_url TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS epg_programs (
            id TEXT PRIMARY KEY,
            channel_id TEXT NOT NULL,
            start TEXT NOT NULL,
            stop TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT,
            category TEXT,
            icon_url TEXT,
            source_url TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_epg_programs_dedup
            ON epg_programs(channel_id, start, title, source_url);
        CREATE INDEX IF NOT EXISTS idx_epg_programs_time_range
            ON epg_programs(channel_id, start, stop);

        CREATE TABLE IF NOT EXISTS favorites (
            id TEXT PRIMARY KEY,
            channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
            playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
            favorite_type TEXT NOT NULL CHECK (favorite_type IN ('channel', 'global')),
            created_at TEXT NOT NULL,
            UNIQUE(channel_id, playlist_id)
        );

        CREATE TABLE IF NOT EXISTS watch_history (
            id TEXT PRIMARY KEY,
            channel_id TEXT REFERENCES channels(id) ON DELETE CASCADE,
            playlist_id TEXT REFERENCES playlists(id) ON DELETE CASCADE,
            item_type TEXT,
            position_seconds INTEGER NOT NULL DEFAULT 0,
            total_seconds INTEGER NOT NULL DEFAULT 0,
            watched_at TEXT NOT NULL,
            UNIQUE(channel_id, playlist_id)
        );
        CREATE INDEX IF NOT EXISTS idx_watch_history_watched_at ON watch_history(watched_at DESC);

        CREATE TABLE IF NOT EXISTS vod_categories (
            id TEXT PRIMARY KEY,
            playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
            content_type TEXT NOT NULL CHECK (content_type IN ('movie', 'series')),
            category_id TEXT NOT NULL,
            name TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_vod_categories_unique
            ON vod_categories(playlist_id, content_type, category_id);

        CREATE TABLE IF NOT EXISTS vod_items (
            id TEXT PRIMARY KEY,
            playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
            content_type TEXT NOT NULL CHECK (content_type IN ('movie', 'series')),
            provider_item_id TEXT NOT NULL,
            category_id TEXT,
            name TEXT NOT NULL,
            cover TEXT,
            rating TEXT,
            genre TEXT,
            release_date TEXT,
            container_extension TEXT,
            added TEXT,
            raw_json TEXT,
            detail_json TEXT
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_vod_items_provider
            ON vod_items(playlist_id, content_type, provider_item_id);
        CREATE INDEX IF NOT EXISTS idx_vod_items_category
            ON vod_items(playlist_id, content_type, category_id);

        CREATE TABLE IF NOT EXISTS vod_sync_state (
            playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
            content_type TEXT NOT NULL CHECK (content_type IN ('movie', 'series')),
            synced_at TEXT NOT NULL,
            PRIMARY KEY (playlist_id, content_type)
        );

        -- A row existing means this ITV category id is missing from the
        -- Stalker bulk `get_all_channels` endpoint (censored/excluded
        -- genre). `last_synced_at` is set only on success (empty result
        -- counts); `last_attempt_at` on every attempt, so a failed retry
        -- can't fabricate a fake "last known good". See db::itv_recovery.
        CREATE TABLE IF NOT EXISTS itv_recovery_category_state (
            playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
            category_id TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('in_progress', 'synced', 'failed')),
            channel_count INTEGER NOT NULL DEFAULT 0,
            last_synced_at TEXT,
            last_attempt_at TEXT NOT NULL,
            PRIMARY KEY (playlist_id, category_id)
        );

        -- One row per title being continued (movie, or series as a whole -
        -- never per episode). For a series only episode_* changes as you
        -- progress; `vod_item_id` always names the series. No FK to
        -- `vod_items` - title/cover are denormalized here so progress
        -- survives independent of the VOD cache row (like watch_history's
        -- channel_name/channel_logo).
        CREATE TABLE IF NOT EXISTS vod_watch_progress (
            id TEXT PRIMARY KEY,
            playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
            content_type TEXT NOT NULL CHECK (content_type IN ('movie', 'series')),
            vod_item_id TEXT NOT NULL,
            episode_id TEXT,
            season_number INTEGER,
            episode_number INTEGER,
            episode_title TEXT,
            position_seconds INTEGER NOT NULL DEFAULT 0,
            total_seconds INTEGER NOT NULL DEFAULT 0,
            title TEXT NOT NULL,
            cover TEXT,
            updated_at TEXT NOT NULL,
            UNIQUE(playlist_id, content_type, vod_item_id)
        );
        CREATE INDEX IF NOT EXISTS idx_vod_watch_progress_updated ON vod_watch_progress(updated_at DESC);

        CREATE TABLE IF NOT EXISTS downloads (
            id TEXT PRIMARY KEY,
            url TEXT NOT NULL,
            file_path TEXT NOT NULL,
            total_bytes INTEGER,
            downloaded_bytes INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL CHECK (
                status IN ('pending', 'downloading', 'paused', 'completed', 'failed', 'canceled')
            ),
            resume_validator TEXT,
            request_headers TEXT,
            error_message TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            payload TEXT NOT NULL
        );
        "#,
    )?;

    // Must run after the batch above: `channels`/`favorites` are created
    // there without these columns (added later), so `ensure_column` adds
    // them uniformly for both fresh and pre-existing installs.
    ensure_column(conn, "channels", "category_id", "TEXT")?;
    ensure_column(conn, "favorites", "position", "INTEGER")?;

    // Must also run after the guards above - indexing a column that doesn't
    // exist yet would error, so this can't live in the CREATE TABLE batch.
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_channels_playlist_category ON channels(playlist_id, category_id)",
        [],
    )?;
    Ok(())
}
