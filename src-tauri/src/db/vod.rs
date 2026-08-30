use crate::types::{StalkerContentItem, VodCatalogItem, VodContentType, XtreamCategory};
use rusqlite::{named_params, Connection, OptionalExtension, Row};

/// Chunked rather than one giant transaction (unlike `channels::insert_channels`)
/// to bound how long a connection is held from the `r2d2` pool and to cap
/// WAL growth — a VOD/series catalog can be much larger than a channel list.
const INSERT_CHUNK_SIZE: usize = 200;

pub fn delete_categories(conn: &Connection, playlist_id: &str, content_type: VodContentType) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM vod_categories WHERE playlist_id = ?1 AND content_type = ?2",
        rusqlite::params![playlist_id, content_type.as_str()],
    )?;
    Ok(())
}

pub fn delete_items(conn: &Connection, playlist_id: &str, content_type: VodContentType) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM vod_items WHERE playlist_id = ?1 AND content_type = ?2",
        rusqlite::params![playlist_id, content_type.as_str()],
    )?;
    Ok(())
}

/// `categories`: `(provider_category_id, name)` pairs, not `&[XtreamCategory]`
/// — Stalker's `StalkerCategory` has no `parent_id`/`count` to force into
/// that shape, and both providers reduce to these two stored columns anyway.
pub fn insert_categories(
    conn: &mut Connection,
    playlist_id: &str,
    content_type: VodContentType,
    categories: &[(String, String)],
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO vod_categories (id, playlist_id, content_type, category_id, name)
             VALUES (:id, :playlist_id, :content_type, :category_id, :name)",
        )?;
        for (category_id, name) in categories {
            stmt.execute(named_params! {
                ":id": uuid::Uuid::new_v4().to_string(),
                ":playlist_id": playlist_id,
                ":content_type": content_type.as_str(),
                ":category_id": category_id,
                ":name": name,
            })?;
        }
    }
    tx.commit()
}

/// The category NAME (not id) an item is filed under — used by
/// `commands::vod_progress::vod_save_progress` to keep adult/18+/XXX
/// category content out of watch-progress tracking (checked by name, since
/// there's no separate "is adult" flag). `None` if not cached locally;
/// callers treat "unknown" as "not adult" rather than failing the save.
pub fn category_name_for_item(
    conn: &Connection,
    playlist_id: &str,
    content_type: VodContentType,
    vod_item_id: &str,
) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT vc.name FROM vod_items vi
         JOIN vod_categories vc
           ON vc.playlist_id = vi.playlist_id AND vc.content_type = vi.content_type AND vc.category_id = vi.category_id
         WHERE vi.playlist_id = ?1 AND vi.content_type = ?2 AND vi.provider_item_id = ?3",
        rusqlite::params![playlist_id, content_type.as_str(), vod_item_id],
        |row| row.get(0),
    )
    .optional()
}

pub fn get_categories(conn: &Connection, playlist_id: &str, content_type: VodContentType) -> rusqlite::Result<Vec<XtreamCategory>> {
    let mut stmt = conn.prepare(
        "SELECT category_id, name FROM vod_categories WHERE playlist_id = ?1 AND content_type = ?2 ORDER BY name ASC",
    )?;
    let rows = stmt.query_map(rusqlite::params![playlist_id, content_type.as_str()], |row| {
        Ok(XtreamCategory {
            id: None,
            category_id: row.get(0)?,
            category_name: row.get(1)?,
            parent_id: 0,
            count: None,
        })
    })?;
    rows.collect()
}

/// Chunked insert — see `INSERT_CHUNK_SIZE`. Each chunk commits
/// independently, so a reader mid-sync can transiently see a
/// partially-repopulated catalog, same pre-existing window as
/// `channels::delete_by_playlist` + `insert_channels`.
pub fn insert_items(
    conn: &mut Connection,
    playlist_id: &str,
    content_type: VodContentType,
    items: &[VodCatalogItem],
) -> rusqlite::Result<()> {
    for chunk in items.chunks(INSERT_CHUNK_SIZE) {
        let tx = conn.transaction()?;
        {
            // Plain INSERT, not upsert: `delete_items` always runs first in
            // the sync flow (`vod_sync`), matching `channels::insert_channels`'s
            // own delete-then-insert convention exactly - there's nothing to
            // conflict with by the time this runs.
            let mut stmt = tx.prepare(
                "INSERT INTO vod_items (
                    id, playlist_id, content_type, provider_item_id, category_id, name, cover,
                    rating, genre, release_date, container_extension, raw_json
                ) VALUES (
                    :id, :playlist_id, :content_type, :provider_item_id, :category_id, :name, :cover,
                    :rating, :genre, :release_date, :container_extension, :raw_json
                )",
            )?;
            for item in chunk {
                let raw_json = item
                    .stalker_item
                    .as_ref()
                    .map(|i| serde_json::to_string(i).unwrap_or_default());
                stmt.execute(named_params! {
                    ":id": uuid::Uuid::new_v4().to_string(),
                    ":playlist_id": playlist_id,
                    ":content_type": content_type.as_str(),
                    ":provider_item_id": item.id,
                    ":category_id": item.category_id,
                    ":name": item.name,
                    ":cover": item.cover,
                    ":rating": item.rating,
                    ":genre": item.genre,
                    ":release_date": item.release_date,
                    ":container_extension": item.container_extension,
                    ":raw_json": raw_json,
                })?;
            }
        }
        tx.commit()?;
    }
    Ok(())
}

/// Opportunistic cache write for a live single-page Stalker fetch
/// (`commands::vod::vod_get_items_live`) — unlike `insert_items`, not
/// preceded by `delete_items`, so a plain INSERT would violate
/// `idx_vod_items_provider` the moment an item is seen twice. `ON CONFLICT
/// DO UPDATE` targets that index, deliberately leaving `detail_json` (must
/// not clobber `set_detail_json`'s cache) and `id` (keeps its identity
/// across re-fetches) untouched.
pub fn upsert_items(
    conn: &mut Connection,
    playlist_id: &str,
    content_type: VodContentType,
    items: &[VodCatalogItem],
) -> rusqlite::Result<()> {
    for chunk in items.chunks(INSERT_CHUNK_SIZE) {
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO vod_items (
                    id, playlist_id, content_type, provider_item_id, category_id, name, cover,
                    rating, genre, release_date, container_extension, raw_json
                ) VALUES (
                    :id, :playlist_id, :content_type, :provider_item_id, :category_id, :name, :cover,
                    :rating, :genre, :release_date, :container_extension, :raw_json
                )
                ON CONFLICT (playlist_id, content_type, provider_item_id) DO UPDATE SET
                    category_id = excluded.category_id,
                    name = excluded.name,
                    cover = excluded.cover,
                    rating = excluded.rating,
                    genre = excluded.genre,
                    release_date = excluded.release_date,
                    container_extension = excluded.container_extension,
                    raw_json = excluded.raw_json",
            )?;
            for item in chunk {
                let raw_json = item
                    .stalker_item
                    .as_ref()
                    .map(|i| serde_json::to_string(i).unwrap_or_default());
                stmt.execute(named_params! {
                    ":id": uuid::Uuid::new_v4().to_string(),
                    ":playlist_id": playlist_id,
                    ":content_type": content_type.as_str(),
                    ":provider_item_id": item.id,
                    ":category_id": item.category_id,
                    ":name": item.name,
                    ":cover": item.cover,
                    ":rating": item.rating,
                    ":genre": item.genre,
                    ":release_date": item.release_date,
                    ":container_extension": item.container_extension,
                    ":raw_json": raw_json,
                })?;
            }
        }
        tx.commit()?;
    }
    Ok(())
}

pub fn get_items(
    conn: &Connection,
    playlist_id: &str,
    content_type: VodContentType,
    category_id: Option<&str>,
) -> rusqlite::Result<Vec<VodCatalogItem>> {
    match category_id {
        Some(cat) => {
            let mut stmt = conn.prepare(
                "SELECT * FROM vod_items WHERE playlist_id = ?1 AND content_type = ?2 AND category_id = ?3 ORDER BY name ASC",
            )?;
            let rows = stmt.query_map(rusqlite::params![playlist_id, content_type.as_str(), cat], row_to_item)?;
            rows.collect()
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT * FROM vod_items WHERE playlist_id = ?1 AND content_type = ?2 ORDER BY name ASC",
            )?;
            let rows = stmt.query_map(rusqlite::params![playlist_id, content_type.as_str()], row_to_item)?;
            rows.collect()
        }
    }
}

/// A single cached catalog row by provider id — the persisted-cache fallback
/// `vod_get_cached_item` uses when the frontend's in-memory session cache
/// (`stalkerRawItems`) has nothing, e.g. right after a restart. Finds
/// anything previously browsed regardless of category.
pub fn get_item(
    conn: &Connection,
    playlist_id: &str,
    content_type: VodContentType,
    provider_item_id: &str,
) -> rusqlite::Result<Option<VodCatalogItem>> {
    conn.query_row(
        "SELECT * FROM vod_items WHERE playlist_id = ?1 AND content_type = ?2 AND provider_item_id = ?3",
        rusqlite::params![playlist_id, content_type.as_str(), provider_item_id],
        row_to_item,
    )
    .optional()
}

/// Top-rated movies for the dashboard's Trending rail, sourced from the
/// local `vod_items` cache. For Xtream/M3U (eager-synced) that's the whole
/// catalog; for Stalker (remote-first, see `vod_get_items_live`) only
/// already-browsed categories, so the rail can be sparse on a fresh Stalker
/// playlist — an accepted tradeoff.
pub fn get_top_rated_movies(conn: &Connection, playlist_id: &str, limit: i64) -> rusqlite::Result<Vec<VodCatalogItem>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM vod_items
         WHERE playlist_id = ?1 AND content_type = 'movie'
           AND rating IS NOT NULL AND TRIM(rating) != '' AND CAST(rating AS REAL) > 0
         ORDER BY CAST(rating AS REAL) DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![playlist_id, limit], row_to_item)?;
    rows.collect()
}

pub fn get_detail_json(
    conn: &Connection,
    playlist_id: &str,
    content_type: VodContentType,
    provider_item_id: &str,
) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT detail_json FROM vod_items WHERE playlist_id = ?1 AND content_type = ?2 AND provider_item_id = ?3",
        rusqlite::params![playlist_id, content_type.as_str(), provider_item_id],
        |row| row.get(0),
    )
    .optional()
    .map(|v: Option<Option<String>>| v.flatten())
}

/// Returns the number of rows updated (0 means the item isn't in the synced
/// catalog, e.g. a stale/never-synced id) — the caller just skips persisting
/// rather than inserting a partial standalone row.
pub fn set_detail_json(
    conn: &Connection,
    playlist_id: &str,
    content_type: VodContentType,
    provider_item_id: &str,
    detail_json: &str,
) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE vod_items SET detail_json = ?1 WHERE playlist_id = ?2 AND content_type = ?3 AND provider_item_id = ?4",
        rusqlite::params![detail_json, playlist_id, content_type.as_str(), provider_item_id],
    )
}

pub fn is_synced(conn: &Connection, playlist_id: &str, content_type: VodContentType) -> rusqlite::Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM vod_sync_state WHERE playlist_id = ?1 AND content_type = ?2",
        rusqlite::params![playlist_id, content_type.as_str()],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn mark_synced(conn: &Connection, playlist_id: &str, content_type: VodContentType, synced_at: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO vod_sync_state (playlist_id, content_type, synced_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(playlist_id, content_type) DO UPDATE SET synced_at = excluded.synced_at",
        rusqlite::params![playlist_id, content_type.as_str(), synced_at],
    )?;
    Ok(())
}

fn row_to_item(row: &Row) -> rusqlite::Result<VodCatalogItem> {
    let content_type: String = row.get("content_type")?;
    let raw_json: Option<String> = row.get("raw_json")?;
    let stalker_item: Option<StalkerContentItem> = raw_json.and_then(|s| serde_json::from_str(&s).ok());

    Ok(VodCatalogItem {
        id: row.get("provider_item_id")?,
        content_type: if content_type == "series" { VodContentType::Series } else { VodContentType::Movie },
        category_id: row.get("category_id")?,
        name: row.get("name")?,
        cover: row.get("cover")?,
        rating: row.get("rating")?,
        genre: row.get("genre")?,
        release_date: row.get("release_date")?,
        container_extension: row.get("container_extension")?,
        stalker_item,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::schema::run_migrations(&conn).unwrap();
        // FK enforcement is off for a bare in-memory connection, but the row
        // still needs to exist to reflect real usage.
        conn.execute(
            "INSERT INTO playlists (id, title, playlist_type, import_date, last_usage) VALUES ('pl1', 'Test', 'xtream', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn
    }

    fn sample_item(id: &str, category_id: &str) -> VodCatalogItem {
        VodCatalogItem {
            id: id.to_string(),
            content_type: VodContentType::Movie,
            category_id: Some(category_id.to_string()),
            name: format!("Movie {id}"),
            cover: Some("http://example.com/cover.jpg".to_string()),
            rating: Some("8.5".to_string()),
            genre: Some("Action".to_string()),
            release_date: Some("2024-01-01".to_string()),
            container_extension: Some("mp4".to_string()),
            stalker_item: None,
        }
    }

    #[test]
    fn inserts_and_reads_back_items_by_category() {
        let mut conn = test_conn();
        let items = vec![sample_item("1", "cat-a"), sample_item("2", "cat-a"), sample_item("3", "cat-b")];
        insert_items(&mut conn, "pl1", VodContentType::Movie, &items).unwrap();

        let all = get_items(&conn, "pl1", VodContentType::Movie, None).unwrap();
        assert_eq!(all.len(), 3);

        let cat_a = get_items(&conn, "pl1", VodContentType::Movie, Some("cat-a")).unwrap();
        assert_eq!(cat_a.len(), 2);
        assert!(cat_a.iter().all(|i| i.category_id.as_deref() == Some("cat-a")));

        // A different content_type never sees another type's rows, even for
        // the same playlist - the series catalog and movie catalog are
        // fully independent.
        let series = get_items(&conn, "pl1", VodContentType::Series, None).unwrap();
        assert!(series.is_empty());
    }

    #[test]
    fn top_rated_movies_orders_by_rating_excludes_series_and_unrated() {
        let mut conn = test_conn();
        let mut low = sample_item("1", "cat-a");
        low.rating = Some("4.0".to_string());
        let mut high = sample_item("2", "cat-a");
        high.rating = Some("9.2".to_string());
        let mut unrated = sample_item("3", "cat-a");
        unrated.rating = None;
        let mut zero = sample_item("4", "cat-a");
        zero.rating = Some("0".to_string());
        insert_items(&mut conn, "pl1", VodContentType::Movie, &[low, high, unrated, zero]).unwrap();

        let mut a_series = sample_item("5", "cat-a");
        a_series.content_type = VodContentType::Series;
        a_series.rating = Some("10".to_string());
        insert_items(&mut conn, "pl1", VodContentType::Series, &[a_series]).unwrap();

        let top = get_top_rated_movies(&conn, "pl1", 10).unwrap();
        let ids: Vec<&str> = top.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["2", "1"]);
    }

    #[test]
    fn top_rated_movies_respects_limit() {
        let mut conn = test_conn();
        let items: Vec<VodCatalogItem> = (0..10)
            .map(|i| {
                let mut item = sample_item(&i.to_string(), "cat-a");
                item.rating = Some((i as f64).to_string());
                item
            })
            .collect();
        insert_items(&mut conn, "pl1", VodContentType::Movie, &items).unwrap();

        let top = get_top_rated_movies(&conn, "pl1", 3).unwrap();
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].id, "9");
    }

    #[test]
    fn insert_items_chunks_beyond_a_single_batch() {
        let mut conn = test_conn();
        let items: Vec<VodCatalogItem> = (0..(INSERT_CHUNK_SIZE * 2 + 37)).map(|i| sample_item(&i.to_string(), "cat-a")).collect();
        let expected = items.len();
        insert_items(&mut conn, "pl1", VodContentType::Movie, &items).unwrap();

        let all = get_items(&conn, "pl1", VodContentType::Movie, None).unwrap();
        assert_eq!(all.len(), expected);
    }

    #[test]
    fn delete_then_insert_fully_replaces_the_catalog() {
        let mut conn = test_conn();
        insert_items(&mut conn, "pl1", VodContentType::Movie, &[sample_item("1", "cat-a")]).unwrap();
        delete_items(&conn, "pl1", VodContentType::Movie).unwrap();
        insert_items(&mut conn, "pl1", VodContentType::Movie, &[sample_item("2", "cat-a")]).unwrap();

        let all = get_items(&conn, "pl1", VodContentType::Movie, None).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "2");
    }

    #[test]
    fn upsert_items_updates_existing_row_and_preserves_detail_json() {
        let mut conn = test_conn();
        insert_items(&mut conn, "pl1", VodContentType::Movie, &[sample_item("1", "cat-a")]).unwrap();
        set_detail_json(&conn, "pl1", VodContentType::Movie, "1", "{\"plot\":\"x\"}").unwrap();

        let local_id_before: String =
            conn.query_row("SELECT id FROM vod_items WHERE provider_item_id = '1'", [], |row| row.get(0)).unwrap();

        // Re-seen with a changed name (e.g. the portal renamed it) and a
        // different category - this must update the existing row in place,
        // not insert a duplicate or clobber the detail cache set above.
        let mut changed = sample_item("1", "cat-b");
        changed.name = "Renamed Movie".to_string();
        upsert_items(&mut conn, "pl1", VodContentType::Movie, &[changed]).unwrap();

        let all = get_items(&conn, "pl1", VodContentType::Movie, None).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "Renamed Movie");
        assert_eq!(all[0].category_id.as_deref(), Some("cat-b"));
        assert_eq!(
            get_detail_json(&conn, "pl1", VodContentType::Movie, "1").unwrap(),
            Some("{\"plot\":\"x\"}".to_string())
        );

        let local_id_after: String =
            conn.query_row("SELECT id FROM vod_items WHERE provider_item_id = '1'", [], |row| row.get(0)).unwrap();
        assert_eq!(local_id_before, local_id_after);
    }

    #[test]
    fn detail_json_round_trips_and_reports_zero_rows_for_an_unknown_id() {
        let mut conn = test_conn();
        insert_items(&mut conn, "pl1", VodContentType::Movie, &[sample_item("1", "cat-a")]).unwrap();

        assert_eq!(get_detail_json(&conn, "pl1", VodContentType::Movie, "1").unwrap(), None);

        let updated = set_detail_json(&conn, "pl1", VodContentType::Movie, "1", "{\"plot\":\"x\"}").unwrap();
        assert_eq!(updated, 1);
        assert_eq!(get_detail_json(&conn, "pl1", VodContentType::Movie, "1").unwrap(), Some("{\"plot\":\"x\"}".to_string()));

        // An id the sync never saw (stale/removed) - the caller is expected
        // to treat 0 as "nothing to persist", not an error.
        let missed = set_detail_json(&conn, "pl1", VodContentType::Movie, "does-not-exist", "{}").unwrap();
        assert_eq!(missed, 0);
    }

    #[test]
    fn sync_state_starts_unsynced_and_persists_once_marked() {
        let conn = test_conn();
        assert!(!is_synced(&conn, "pl1", VodContentType::Movie).unwrap());

        mark_synced(&conn, "pl1", VodContentType::Movie, "2026-01-01T00:00:00Z").unwrap();
        assert!(is_synced(&conn, "pl1", VodContentType::Movie).unwrap());
        // Independent per content type - marking movies synced must not
        // make series look synced too.
        assert!(!is_synced(&conn, "pl1", VodContentType::Series).unwrap());

        // Re-marking (a resync) must not fail on the unique (playlist_id,
        // content_type) primary key.
        mark_synced(&conn, "pl1", VodContentType::Movie, "2026-01-02T00:00:00Z").unwrap();
        assert!(is_synced(&conn, "pl1", VodContentType::Movie).unwrap());
    }

    #[test]
    fn categories_round_trip() {
        let mut conn = test_conn();
        let categories = vec![("c1".to_string(), "Action".to_string()), ("c2".to_string(), "Comedy".to_string())];
        insert_categories(&mut conn, "pl1", VodContentType::Movie, &categories).unwrap();

        let read_back = get_categories(&conn, "pl1", VodContentType::Movie).unwrap();
        assert_eq!(read_back.len(), 2);
        let names: Vec<&str> = read_back.iter().map(|c| c.category_name.as_str()).collect();
        assert!(names.contains(&"Action"));
        assert!(names.contains(&"Comedy"));

        delete_categories(&conn, "pl1", VodContentType::Movie).unwrap();
        assert!(get_categories(&conn, "pl1", VodContentType::Movie).unwrap().is_empty());
    }
}
