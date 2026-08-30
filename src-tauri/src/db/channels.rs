use crate::parsers::m3u::ParsedM3uItem;
use crate::types::{Channel, ChannelCatchup, ChannelDrm, ChannelGroup, ChannelHttp, ChannelTvg};
use rusqlite::{named_params, Connection, OptionalExtension, Row, Transaction};

/// Pulls the portal category id out of a channel's `raw` JSON blob (stashed
/// there by `net::stalker::content::parse_channel_row`). `None` for M3U/
/// Xtream rows, whose `raw` never carries this key - keeps `insert_channels`
/// provider-agnostic instead of needing a per-provider branch.
fn extract_category_id(raw: Option<&str>) -> Option<String> {
    let raw = raw?;
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    value.get("category_id").and_then(|v| v.as_str()).map(str::to_string)
}

/// Inserts freshly-parsed M3U items as channel rows for `playlist_id`,
/// returning the number of rows inserted. Runs inside one transaction so a
/// large playlist import is one durable write, not thousands.
pub fn insert_m3u_items(
    conn: &mut Connection,
    playlist_id: &str,
    items: &[ParsedM3uItem],
) -> rusqlite::Result<i64> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO channels (
                id, playlist_id, name, url, group_title, tvg_id, tvg_name, tvg_url, tvg_logo,
                tvg_rec, http_referrer, http_user_agent, http_origin, radio, catchup_type,
                catchup_source, catchup_days, timeshift, drm_json, raw_json, channel_number,
                category_id
            ) VALUES (
                :id, :playlist_id, :name, :url, :group_title, :tvg_id, :tvg_name, :tvg_url,
                :tvg_logo, :tvg_rec, :http_referrer, :http_user_agent, :http_origin, :radio,
                :catchup_type, :catchup_source, :catchup_days, :timeshift, :drm_json, :raw_json,
                :channel_number, :category_id
            )",
        )?;

        for (index, item) in items.iter().enumerate() {
            let id = uuid::Uuid::new_v4().to_string();
            let drm_json = item
                .drm
                .as_ref()
                .map(|d| serde_json::to_string(d).unwrap_or_default());
            let raw_json = if item.raw.is_empty() { None } else { Some(item.raw.clone()) };

            stmt.execute(named_params! {
                ":id": id,
                ":playlist_id": playlist_id,
                ":name": item.name,
                ":url": item.url,
                ":group_title": item.group_title,
                ":tvg_id": item.tvg_id,
                ":tvg_name": item.tvg_name,
                ":tvg_url": item.tvg_url,
                ":tvg_logo": item.tvg_logo,
                ":tvg_rec": item.tvg_rec,
                ":http_referrer": item.http_referrer,
                ":http_user_agent": item.http_user_agent,
                ":http_origin": Option::<String>::None,
                ":radio": item.radio,
                ":catchup_type": item.catchup_type,
                ":catchup_source": item.catchup_source,
                ":catchup_days": item.catchup_days,
                ":timeshift": item.timeshift,
                ":drm_json": drm_json,
                ":category_id": extract_category_id(raw_json.as_deref()),
                ":raw_json": raw_json,
                ":channel_number": (index as i64) + 1,
            })?;
        }
    }
    let count = items.len() as i64;
    tx.commit()?;
    Ok(count)
}

/// Shared insert loop for `insert_channels` and `replace_category_channels`
/// so their column lists can't drift apart.
fn insert_channels_in_tx(tx: &Transaction, playlist_id: &str, channels: &[Channel]) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO channels (
            id, playlist_id, name, url, group_title, tvg_id, tvg_name, tvg_url, tvg_logo,
            tvg_rec, http_referrer, http_user_agent, http_origin, radio, catchup_type,
            catchup_source, catchup_days, timeshift, drm_json, raw_json, channel_number,
            category_id
        ) VALUES (
            :id, :playlist_id, :name, :url, :group_title, :tvg_id, :tvg_name, :tvg_url,
            :tvg_logo, :tvg_rec, :http_referrer, :http_user_agent, :http_origin, :radio,
            :catchup_type, :catchup_source, :catchup_days, :timeshift, :drm_json, :raw_json,
            :channel_number, :category_id
        )",
    )?;

    for channel in channels {
        let drm_json = channel.drm.as_ref().map(|d| serde_json::to_string(d).unwrap_or_default());
        let (catchup_type, catchup_source, catchup_days) = match &channel.catchup {
            Some(c) => (c.r#type.clone(), c.source.clone(), c.days.clone()),
            None => (None, None, None),
        };
        let group_title = if channel.group.title.is_empty() {
            None
        } else {
            Some(channel.group.title.clone())
        };

        stmt.execute(named_params! {
            ":id": channel.id,
            ":playlist_id": playlist_id,
            ":name": channel.name,
            ":url": channel.url,
            ":group_title": group_title,
            ":tvg_id": channel.tvg.id,
            ":tvg_name": channel.tvg.name,
            ":tvg_url": channel.tvg.url,
            ":tvg_logo": channel.tvg.logo,
            ":tvg_rec": channel.tvg.rec,
            ":http_referrer": channel.http.referrer,
            ":http_user_agent": channel.http.user_agent,
            ":http_origin": channel.http.origin,
            ":radio": channel.radio,
            ":catchup_type": catchup_type,
            ":catchup_source": catchup_source,
            ":catchup_days": catchup_days,
            ":timeshift": channel.timeshift,
            ":drm_json": drm_json,
            ":category_id": extract_category_id(channel.raw.as_deref()),
            ":raw_json": channel.raw,
            ":channel_number": channel.channel_number,
        })?;
    }
    Ok(())
}

/// Inserts already-built `Channel` rows (Xtream/Stalker paths, where the
/// channel is assembled from a provider API response rather than parsed
/// text) — unlike `insert_m3u_items`, the caller has already generated ids.
pub fn insert_channels(conn: &mut Connection, playlist_id: &str, channels: &[Channel]) -> rusqlite::Result<i64> {
    let tx = conn.transaction()?;
    insert_channels_in_tx(&tx, playlist_id, channels)?;
    let count = channels.len() as i64;
    tx.commit()?;
    Ok(count)
}

pub fn delete_by_playlist(conn: &Connection, playlist_id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM channels WHERE playlist_id = ?1", [playlist_id])?;
    Ok(())
}

/// Deletes every channel row for `playlist_id` except those whose
/// `category_id` is in `preserved_category_ids` — lets a still-fresh
/// recovered category survive the bulk sync's wipe instead of needing a
/// full re-crawl (see `commands::stalker::sync_channels_category_aware`).
/// `category_id IS NULL` rows are always deleted: only a category confirmed
/// fresh in `itv_recovery_category_state` is ever preserved.
pub fn delete_by_playlist_except_categories(
    conn: &Connection,
    playlist_id: &str,
    preserved_category_ids: &[String],
) -> rusqlite::Result<()> {
    if preserved_category_ids.is_empty() {
        return delete_by_playlist(conn, playlist_id);
    }
    let placeholders = vec!["?"; preserved_category_ids.len()].join(",");
    let sql = format!(
        "DELETE FROM channels WHERE playlist_id = ? AND (category_id IS NULL OR category_id NOT IN ({placeholders}))"
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(preserved_category_ids.len() + 1);
    params.push(&playlist_id);
    for id in preserved_category_ids {
        params.push(id);
    }
    conn.execute(&sql, params.as_slice())?;
    Ok(())
}

/// Atomically replaces one ITV category's rows (delete+insert in one
/// transaction, so a reader never sees it half-gone). Only call after a
/// recovery fetch succeeded — an empty slice is a valid "zero items now"
/// result; a failed fetch must never reach here (see
/// `commands::stalker::run_censored_itv_recovery`).
pub fn replace_category_channels(
    conn: &mut Connection,
    playlist_id: &str,
    category_id: &str,
    channels: &[Channel],
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM channels WHERE playlist_id = ?1 AND category_id = ?2",
        rusqlite::params![playlist_id, category_id],
    )?;
    insert_channels_in_tx(&tx, playlist_id, channels)?;
    tx.commit()
}

/// Recomputed, not incremented — a background recovery task can run well
/// after the initial sync's count write, so a fresh `COUNT(*)` avoids the
/// two writers needing to coordinate.
pub fn count_by_playlist(conn: &Connection, playlist_id: &str) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM channels WHERE playlist_id = ?1", [playlist_id], |row| row.get(0))
}

pub fn list_by_playlist(conn: &Connection, playlist_id: &str) -> rusqlite::Result<Vec<Channel>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM channels WHERE playlist_id = ?1 ORDER BY channel_number ASC, rowid ASC",
    )?;
    let rows = stmt.query_map([playlist_id], row_to_channel)?;
    rows.collect()
}

pub fn get_by_id(conn: &Connection, id: &str) -> rusqlite::Result<Option<Channel>> {
    conn.query_row("SELECT * FROM channels WHERE id = ?1", [id], row_to_channel)
        .optional()
}

/// `Channel` doesn't carry its own `playlist_id` (nothing needed it until
/// Xtream catch-up resolution) — a narrow lookup instead of adding the
/// column everywhere `Channel` is read.
pub fn get_playlist_id(conn: &Connection, channel_id: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT playlist_id FROM channels WHERE id = ?1", [channel_id], |row| row.get(0))
        .optional()
}

/// Looks up a channel by its exact stream URL — used by the stream proxy to
/// find per-channel HTTP header overrides (from M3U `#EXTVLCOPT`/`#KODIPROP`)
/// when it only has the URL, not a channel id. VOD/series are never in this
/// table, so a VOD URL finds nothing and the caller falls back to
/// playlist-level headers.
pub fn find_by_playlist_and_url(conn: &Connection, playlist_id: &str, url: &str) -> rusqlite::Result<Option<Channel>> {
    conn.query_row(
        "SELECT * FROM channels WHERE playlist_id = ?1 AND url = ?2 LIMIT 1",
        rusqlite::params![playlist_id, url],
        row_to_channel,
    )
    .optional()
}

/// Trigram FTS5 substring search over channel names, scoped to a playlist
/// when given. Falls back to a plain `LIKE` scan if the FTS5 MATCH query
/// errors, rather than crashing search-as-you-type.
pub fn search(conn: &Connection, query: &str, playlist_id: Option<&str>) -> rusqlite::Result<Vec<Channel>> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return match playlist_id {
            Some(pid) => list_by_playlist(conn, pid),
            None => {
                let mut stmt = conn.prepare("SELECT * FROM channels ORDER BY channel_number ASC, rowid ASC")?;
                let rows = stmt.query_map([], row_to_channel)?;
                rows.collect()
            }
        };
    }

    // FTS5 phrase-quote the raw query so user-typed special characters
    // (", *, :, ...) can't be misread as MATCH query syntax.
    let fts_query = format!("\"{}\"", trimmed.replace('"', "\"\""));

    let fts_result: rusqlite::Result<Vec<Channel>> = (|| {
        let sql = match playlist_id {
            Some(_) => {
                "SELECT c.* FROM channels c
                 JOIN channels_fts f ON f.rowid = c.rowid
                 WHERE channels_fts MATCH ?1 AND c.playlist_id = ?2
                 ORDER BY rank"
            }
            None => {
                "SELECT c.* FROM channels c
                 JOIN channels_fts f ON f.rowid = c.rowid
                 WHERE channels_fts MATCH ?1
                 ORDER BY rank"
            }
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = match playlist_id {
            Some(pid) => stmt.query_map(rusqlite::params![fts_query, pid], row_to_channel)?.collect(),
            None => stmt.query_map(rusqlite::params![fts_query], row_to_channel)?.collect(),
        };
        rows
    })();

    if let Ok(results) = fts_result {
        return Ok(results);
    }

    let like_pattern = format!("%{}%", trimmed.replace('%', "\\%").replace('_', "\\_"));
    let sql = match playlist_id {
        Some(_) => {
            "SELECT * FROM channels WHERE name LIKE ?1 ESCAPE '\\' AND playlist_id = ?2
             ORDER BY channel_number ASC, rowid ASC"
        }
        None => "SELECT * FROM channels WHERE name LIKE ?1 ESCAPE '\\' ORDER BY channel_number ASC, rowid ASC",
    };
    let mut stmt = conn.prepare(sql)?;
    match playlist_id {
        Some(pid) => stmt
            .query_map(rusqlite::params![like_pattern, pid], row_to_channel)?
            .collect(),
        None => stmt.query_map(rusqlite::params![like_pattern], row_to_channel)?.collect(),
    }
}

fn row_to_channel(row: &Row) -> rusqlite::Result<Channel> {
    let drm_json: Option<String> = row.get("drm_json")?;
    let drm: Option<ChannelDrm> = drm_json.and_then(|s| serde_json::from_str(&s).ok());

    let catchup_type: Option<String> = row.get("catchup_type")?;
    let catchup_source: Option<String> = row.get("catchup_source")?;
    let catchup_days: Option<String> = row.get("catchup_days")?;
    let catchup = if catchup_type.is_some() || catchup_source.is_some() || catchup_days.is_some() {
        Some(ChannelCatchup {
            r#type: catchup_type,
            source: catchup_source,
            days: catchup_days,
        })
    } else {
        None
    };

    Ok(Channel {
        id: row.get("id")?,
        url: row.get("url")?,
        name: row.get("name")?,
        group: ChannelGroup {
            title: row.get::<_, Option<String>>("group_title")?.unwrap_or_default(),
        },
        tvg: ChannelTvg {
            id: row.get("tvg_id")?,
            name: row.get("tvg_name")?,
            url: row.get("tvg_url")?,
            logo: row.get("tvg_logo")?,
            rec: row.get("tvg_rec")?,
        },
        epg_params: None,
        timeshift: row.get("timeshift")?,
        catchup,
        http: ChannelHttp {
            referrer: row.get("http_referrer")?,
            user_agent: row.get("http_user_agent")?,
            origin: row.get("http_origin")?,
        },
        radio: row.get("radio")?,
        drm,
        raw: row.get("raw_json")?,
        channel_number: row.get("channel_number")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::schema::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO playlists (id, title, playlist_type, import_date, last_usage) VALUES ('pl1', 'Test', 'stalker', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn
    }

    /// `category_id: None` produces an M3U/Xtream-shaped row (no `raw` at
    /// all); `Some(id)` mirrors what `parse_channel_row` stashes for a
    /// Stalker row.
    fn sample_channel(name: &str, group_title: &str, category_id: Option<&str>) -> Channel {
        Channel {
            id: uuid::Uuid::new_v4().to_string(),
            url: format!("http://example.com/{name}"),
            name: name.to_string(),
            group: ChannelGroup { title: group_title.to_string() },
            tvg: ChannelTvg::default(),
            epg_params: None,
            timeshift: None,
            catchup: None,
            http: ChannelHttp::default(),
            radio: "0".to_string(),
            drm: None,
            raw: category_id.map(|id| serde_json::json!({ "category_id": id }).to_string()),
            channel_number: Some(1),
        }
    }

    #[test]
    fn insert_channels_extracts_category_id_from_raw_json() {
        let mut conn = test_conn();
        let channels = vec![
            sample_channel("Adult 1", "FOR ADULTS", Some("539")),
            sample_channel("Regular 1", "News", None),
        ];
        insert_channels(&mut conn, "pl1", &channels).unwrap();

        let category_id: Option<String> = conn
            .query_row("SELECT category_id FROM channels WHERE name = 'Adult 1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(category_id.as_deref(), Some("539"));

        let no_category: Option<String> = conn
            .query_row("SELECT category_id FROM channels WHERE name = 'Regular 1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(no_category, None);
    }

    #[test]
    fn delete_by_playlist_except_categories_preserves_only_listed_categories() {
        let mut conn = test_conn();
        let channels = vec![
            sample_channel("Bulk 1", "News", Some("100")),
            sample_channel("Recovered 1", "Adults", Some("539")),
            sample_channel("No Category", "Other", None),
        ];
        insert_channels(&mut conn, "pl1", &channels).unwrap();

        delete_by_playlist_except_categories(&conn, "pl1", &["539".to_string()]).unwrap();

        let remaining = list_by_playlist(&conn, "pl1").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].name, "Recovered 1");
    }

    #[test]
    fn delete_by_playlist_except_categories_falls_back_to_full_wipe_when_list_is_empty() {
        let mut conn = test_conn();
        insert_channels(&mut conn, "pl1", &[sample_channel("A", "News", Some("100"))]).unwrap();

        delete_by_playlist_except_categories(&conn, "pl1", &[]).unwrap();

        assert!(list_by_playlist(&conn, "pl1").unwrap().is_empty());
    }

    #[test]
    fn replace_category_channels_is_atomic_and_accepts_an_empty_result() {
        let mut conn = test_conn();
        insert_channels(
            &mut conn,
            "pl1",
            &[
                sample_channel("Old 1", "Adults", Some("539")),
                sample_channel("Old 2", "Adults", Some("539")),
                sample_channel("Untouched", "News", Some("100")),
            ],
        )
        .unwrap();

        // Non-empty replace: old rows for the category are gone, new ones
        // are there, the other category is untouched.
        replace_category_channels(&mut conn, "pl1", "539", &[sample_channel("New 1", "Adults", Some("539"))]).unwrap();
        let all = list_by_playlist(&conn, "pl1").unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|c| c.name == "New 1"));
        assert!(all.iter().any(|c| c.name == "Untouched"));

        // Empty replace: a confirmed-successful "this category is empty now"
        // result must clear the category's rows, not error or leave them.
        replace_category_channels(&mut conn, "pl1", "539", &[]).unwrap();
        let all = list_by_playlist(&conn, "pl1").unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "Untouched");
    }
}
