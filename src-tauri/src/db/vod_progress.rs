use crate::types::{VodContentType, VodProgressEpisodeRef, VodWatchProgress};
use chrono::Utc;
use rusqlite::{named_params, Connection, OptionalExtension, Row};
use std::collections::HashMap;

/// Plain unconditional upsert — the 60s/2% "worth tracking" threshold is
/// enforced by the caller (`commands::vod_progress::vod_save_progress`),
/// matching how `save_playback_position`'s setting check lives in its
/// command wrapper, not `db::favorites`.
pub fn upsert(
    conn: &Connection,
    playlist_id: &str,
    content_type: VodContentType,
    vod_item_id: &str,
    episode: Option<&VodProgressEpisodeRef>,
    position_seconds: i64,
    total_seconds: i64,
    title: &str,
    cover: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO vod_watch_progress (
            id, playlist_id, content_type, vod_item_id, episode_id, season_number,
            episode_number, episode_title, position_seconds, total_seconds, title, cover, updated_at
        ) VALUES (
            :id, :playlist_id, :content_type, :vod_item_id, :episode_id, :season_number,
            :episode_number, :episode_title, :position_seconds, :total_seconds, :title, :cover, :updated_at
        )
        ON CONFLICT (playlist_id, content_type, vod_item_id) DO UPDATE SET
            episode_id = excluded.episode_id,
            season_number = excluded.season_number,
            episode_number = excluded.episode_number,
            episode_title = excluded.episode_title,
            position_seconds = excluded.position_seconds,
            total_seconds = excluded.total_seconds,
            title = excluded.title,
            cover = excluded.cover,
            updated_at = excluded.updated_at",
        named_params! {
            ":id": uuid::Uuid::new_v4().to_string(),
            ":playlist_id": playlist_id,
            ":content_type": content_type.as_str(),
            ":vod_item_id": vod_item_id,
            ":episode_id": episode.map(|e| e.id.as_str()),
            ":season_number": episode.map(|e| e.season_number),
            ":episode_number": episode.and_then(|e| e.episode_number),
            ":episode_title": episode.map(|e| e.title.as_str()),
            ":position_seconds": position_seconds,
            ":total_seconds": total_seconds,
            ":title": title,
            ":cover": cover,
            ":updated_at": Utc::now().to_rfc3339(),
        },
    )?;
    Ok(())
}

/// Used both when a movie completes (spec: clear its resume state entirely,
/// not just mark it done) and when a series has no next episode left
/// (fully watched, nothing to continue).
pub fn delete(conn: &Connection, playlist_id: &str, content_type: VodContentType, vod_item_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM vod_watch_progress WHERE playlist_id = ?1 AND content_type = ?2 AND vod_item_id = ?3",
        rusqlite::params![playlist_id, content_type.as_str(), vod_item_id],
    )?;
    Ok(())
}

pub fn get(
    conn: &Connection,
    playlist_id: &str,
    content_type: VodContentType,
    vod_item_id: &str,
) -> rusqlite::Result<Option<VodWatchProgress>> {
    conn.query_row(
        "SELECT * FROM vod_watch_progress WHERE playlist_id = ?1 AND content_type = ?2 AND vod_item_id = ?3",
        rusqlite::params![playlist_id, content_type.as_str(), vod_item_id],
        row_to_progress,
    )
    .optional()
}

/// One query for however many cards a `VodGrid` batch is currently
/// rendering, rather than one round trip per poster.
pub fn get_bulk(
    conn: &Connection,
    playlist_id: &str,
    content_type: VodContentType,
    vod_item_ids: &[String],
) -> rusqlite::Result<HashMap<String, VodWatchProgress>> {
    if vod_item_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = vod_item_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT * FROM vod_watch_progress WHERE playlist_id = ? AND content_type = ? AND vod_item_id IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql)?;
    let content_type_str = content_type.as_str();
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&playlist_id, &content_type_str];
    for id in vod_item_ids {
        params.push(id);
    }
    let rows = stmt.query_map(params.as_slice(), row_to_progress)?;
    let mut out = HashMap::new();
    for row in rows {
        let progress = row?;
        out.insert(progress.vod_item_id.clone(), progress);
    }
    Ok(out)
}

/// Cross-playlist, newest-first — matches `db::favorites::recently_watched`'s
/// existing shape for the same kind of "Continue Watching"/history rail.
pub fn list_continue_watching(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<VodWatchProgress>> {
    let mut stmt = conn.prepare("SELECT * FROM vod_watch_progress ORDER BY updated_at DESC LIMIT ?1")?;
    let rows = stmt.query_map([limit], row_to_progress)?;
    rows.collect()
}

fn row_to_progress(row: &Row) -> rusqlite::Result<VodWatchProgress> {
    let content_type_str: String = row.get("content_type")?;
    let content_type = if content_type_str == "series" { VodContentType::Series } else { VodContentType::Movie };
    Ok(VodWatchProgress {
        id: row.get("id")?,
        playlist_id: row.get("playlist_id")?,
        content_type,
        vod_item_id: row.get("vod_item_id")?,
        episode_id: row.get("episode_id")?,
        season_number: row.get("season_number")?,
        episode_number: row.get("episode_number")?,
        episode_title: row.get("episode_title")?,
        position_seconds: row.get("position_seconds")?,
        total_seconds: row.get("total_seconds")?,
        title: row.get("title")?,
        cover: row.get("cover")?,
        updated_at: row.get("updated_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::schema::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO playlists (id, title, playlist_type, import_date, last_usage) VALUES ('pl1', 'Test', 'xtream', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn upsert_then_get_round_trips_a_movie() {
        let conn = test_conn();
        upsert(&conn, "pl1", VodContentType::Movie, "movie-1", None, 120, 6000, "Some Movie", Some("http://x/cover.jpg")).unwrap();

        let found = get(&conn, "pl1", VodContentType::Movie, "movie-1").unwrap().unwrap();
        assert_eq!(found.position_seconds, 120);
        assert_eq!(found.total_seconds, 6000);
        assert_eq!(found.title, "Some Movie");
        assert!(found.episode_id.is_none());
    }

    #[test]
    fn upsert_on_same_vod_item_updates_in_place_not_duplicates() {
        let conn = test_conn();
        upsert(&conn, "pl1", VodContentType::Movie, "movie-1", None, 100, 6000, "Some Movie", None).unwrap();
        upsert(&conn, "pl1", VodContentType::Movie, "movie-1", None, 300, 6000, "Some Movie", None).unwrap();

        let all = list_continue_watching(&conn, 10).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].position_seconds, 300);
    }

    #[test]
    fn series_upsert_tracks_the_current_episode() {
        let conn = test_conn();
        let episode = VodProgressEpisodeRef {
            id: "ep-1".to_string(),
            season_number: 1,
            episode_number: Some(3),
            title: "Episode Three".to_string(),
        };
        upsert(&conn, "pl1", VodContentType::Series, "series-1", Some(&episode), 400, 1200, "Some Series", None).unwrap();

        let found = get(&conn, "pl1", VodContentType::Series, "series-1").unwrap().unwrap();
        assert_eq!(found.episode_id.as_deref(), Some("ep-1"));
        assert_eq!(found.season_number, Some(1));
        assert_eq!(found.episode_number, Some(3));
        assert_eq!(found.episode_title.as_deref(), Some("Episode Three"));
    }

    #[test]
    fn delete_removes_the_row_entirely_not_just_marks_it() {
        let conn = test_conn();
        upsert(&conn, "pl1", VodContentType::Movie, "movie-1", None, 100, 6000, "Some Movie", None).unwrap();
        delete(&conn, "pl1", VodContentType::Movie, "movie-1").unwrap();

        assert!(get(&conn, "pl1", VodContentType::Movie, "movie-1").unwrap().is_none());
        assert!(list_continue_watching(&conn, 10).unwrap().is_empty());
    }

    #[test]
    fn get_bulk_returns_only_matching_ids_keyed_by_vod_item_id() {
        let conn = test_conn();
        upsert(&conn, "pl1", VodContentType::Movie, "movie-1", None, 100, 6000, "Movie One", None).unwrap();
        upsert(&conn, "pl1", VodContentType::Movie, "movie-2", None, 200, 6000, "Movie Two", None).unwrap();
        upsert(&conn, "pl1", VodContentType::Movie, "movie-3", None, 300, 6000, "Movie Three", None).unwrap();

        let ids = vec!["movie-1".to_string(), "movie-3".to_string(), "movie-nonexistent".to_string()];
        let found = get_bulk(&conn, "pl1", VodContentType::Movie, &ids).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found.get("movie-1").unwrap().position_seconds, 100);
        assert_eq!(found.get("movie-3").unwrap().position_seconds, 300);
        assert!(!found.contains_key("movie-2"));
    }

    #[test]
    fn get_bulk_with_empty_ids_returns_empty_without_querying() {
        let conn = test_conn();
        let found = get_bulk(&conn, "pl1", VodContentType::Movie, &[]).unwrap();
        assert!(found.is_empty());
    }

    #[test]
    fn list_continue_watching_orders_newest_first() {
        let conn = test_conn();
        upsert(&conn, "pl1", VodContentType::Movie, "movie-1", None, 100, 6000, "First Saved", None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        upsert(&conn, "pl1", VodContentType::Movie, "movie-2", None, 100, 6000, "Second Saved", None).unwrap();

        let all = list_continue_watching(&conn, 10).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].title, "Second Saved");
        assert_eq!(all[1].title, "First Saved");
    }
}
