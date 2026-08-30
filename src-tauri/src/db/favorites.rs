use crate::types::{FavoriteChannel, FavoriteType, WatchHistoryItem};
use chrono::Utc;
use rusqlite::{named_params, Connection, OptionalExtension, Row};

/// Toggles a favorite for `(channel_id, playlist_id)`, returning the new
/// favorite state (`true` if it now exists, `false` if it was just removed).
pub fn toggle(
    conn: &Connection,
    channel_id: &str,
    playlist_id: &str,
    favorite_type: FavoriteType,
) -> rusqlite::Result<bool> {
    let existing_id: Option<String> = conn
        .query_row(
            "SELECT id FROM favorites WHERE channel_id = ?1 AND playlist_id = ?2",
            [channel_id, playlist_id],
            |row| row.get(0),
        )
        .optional()?;

    match existing_id {
        Some(id) => {
            conn.execute("DELETE FROM favorites WHERE id = ?1", [id])?;
            Ok(false)
        }
        None => {
            // Appended at the end of the current order, not left NULL, so a
            // new favorite doesn't jump ahead of drag-reordered ones (NULL
            // sorting last in `list()` is only a fallback for old rows).
            let next_position: i64 = conn.query_row(
                "SELECT COALESCE(MAX(position), -1) + 1 FROM favorites WHERE playlist_id = ?1",
                [playlist_id],
                |row| row.get(0),
            )?;
            conn.execute(
                "INSERT INTO favorites (id, channel_id, playlist_id, favorite_type, position, created_at)
                 VALUES (:id, :channel_id, :playlist_id, :favorite_type, :position, :created_at)",
                named_params! {
                    ":id": uuid::Uuid::new_v4().to_string(),
                    ":channel_id": channel_id,
                    ":playlist_id": playlist_id,
                    ":favorite_type": favorite_type.as_str(),
                    ":position": next_position,
                    ":created_at": Utc::now().to_rfc3339(),
                },
            )?;
            Ok(true)
        }
    }
}

pub fn list(conn: &Connection, playlist_id: Option<&str>) -> rusqlite::Result<Vec<FavoriteChannel>> {
    let mut stmt = match playlist_id {
        Some(_) => conn.prepare(
            "SELECT f.*, c.name AS channel_name, c.tvg_logo AS channel_logo
             FROM favorites f LEFT JOIN channels c ON c.id = f.channel_id
             WHERE f.playlist_id = ?1
             ORDER BY COALESCE(f.position, 2147483647) ASC, f.created_at DESC",
        )?,
        None => conn.prepare(
            "SELECT f.*, c.name AS channel_name, c.tvg_logo AS channel_logo
             FROM favorites f LEFT JOIN channels c ON c.id = f.channel_id
             ORDER BY COALESCE(f.position, 2147483647) ASC, f.created_at DESC",
        )?,
    };
    let rows = match playlist_id {
        Some(pid) => stmt.query_map([pid], row_to_favorite)?.collect(),
        None => stmt.query_map([], row_to_favorite)?.collect(),
    };
    rows
}

/// Applies a user drag-reorder: `ordered_channel_ids` is the full new order
/// for this playlist's favorites, front to back. Positions are (re)assigned
/// 0..N from that order; any favorite row not present in the list (e.g. a
/// race with a concurrent toggle) is left untouched rather than erroring.
pub fn reorder(conn: &Connection, playlist_id: &str, ordered_channel_ids: &[String]) -> rusqlite::Result<()> {
    for (index, channel_id) in ordered_channel_ids.iter().enumerate() {
        conn.execute(
            "UPDATE favorites SET position = ?1 WHERE playlist_id = ?2 AND channel_id = ?3",
            rusqlite::params![index as i64, playlist_id, channel_id],
        )?;
    }
    Ok(())
}

pub fn recently_watched(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<WatchHistoryItem>> {
    let mut stmt = conn.prepare(
        "SELECT h.*, c.name AS channel_name, c.tvg_logo AS channel_logo
         FROM watch_history h LEFT JOIN channels c ON c.id = h.channel_id
         ORDER BY h.watched_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], row_to_history)?;
    rows.collect()
}

pub fn remove_history_item(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM watch_history WHERE id = ?1", [id])?;
    Ok(())
}

pub fn clear_history(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM watch_history", [])?;
    Ok(())
}

pub fn save_playback_position(
    conn: &Connection,
    channel_id: &str,
    playlist_id: &str,
    position_seconds: i64,
    total_seconds: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO watch_history (id, channel_id, playlist_id, item_type, position_seconds, total_seconds, watched_at)
         VALUES (:id, :channel_id, :playlist_id, 'channel', :position_seconds, :total_seconds, :watched_at)
         ON CONFLICT(channel_id, playlist_id) DO UPDATE SET
            position_seconds = excluded.position_seconds,
            total_seconds = excluded.total_seconds,
            watched_at = excluded.watched_at",
        named_params! {
            ":id": uuid::Uuid::new_v4().to_string(),
            ":channel_id": channel_id,
            ":playlist_id": playlist_id,
            ":position_seconds": position_seconds,
            ":total_seconds": total_seconds,
            ":watched_at": Utc::now().to_rfc3339(),
        },
    )?;
    Ok(())
}

fn row_to_favorite(row: &Row) -> rusqlite::Result<FavoriteChannel> {
    let favorite_type_str: String = row.get("favorite_type")?;
    Ok(FavoriteChannel {
        id: row.get("id")?,
        channel_id: row.get("channel_id")?,
        playlist_id: row.get("playlist_id")?,
        favorite_type: FavoriteType::from_str(&favorite_type_str).unwrap_or(FavoriteType::Channel),
        created_at: row.get("created_at")?,
        channel_name: row.get("channel_name")?,
        channel_logo: row.get("channel_logo")?,
    })
}

fn row_to_history(row: &Row) -> rusqlite::Result<WatchHistoryItem> {
    Ok(WatchHistoryItem {
        id: row.get("id")?,
        channel_id: row.get("channel_id")?,
        playlist_id: row.get("playlist_id")?,
        item_type: row.get("item_type")?,
        position_seconds: row.get("position_seconds")?,
        total_seconds: row.get("total_seconds")?,
        watched_at: row.get("watched_at")?,
        channel_name: row.get("channel_name")?,
        channel_logo: row.get("channel_logo")?,
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
        for id in ["c1", "c2", "c3"] {
            conn.execute(
                "INSERT INTO channels (id, playlist_id, name, url) VALUES (?1, 'pl1', ?1, 'http://example.com')",
                [id],
            )
            .unwrap();
        }
        conn
    }

    #[test]
    fn toggle_assigns_increasing_positions_and_list_orders_by_them() {
        let conn = test_conn();
        toggle(&conn, "c1", "pl1", FavoriteType::Channel).unwrap();
        toggle(&conn, "c2", "pl1", FavoriteType::Channel).unwrap();
        toggle(&conn, "c3", "pl1", FavoriteType::Channel).unwrap();

        let favorites = list(&conn, Some("pl1")).unwrap();
        let ids: Vec<&str> = favorites.iter().map(|f| f.channel_id.as_str()).collect();
        assert_eq!(ids, vec!["c1", "c2", "c3"]);
    }

    #[test]
    fn reorder_persists_new_order_and_list_reflects_it() {
        let conn = test_conn();
        toggle(&conn, "c1", "pl1", FavoriteType::Channel).unwrap();
        toggle(&conn, "c2", "pl1", FavoriteType::Channel).unwrap();
        toggle(&conn, "c3", "pl1", FavoriteType::Channel).unwrap();

        reorder(&conn, "pl1", &["c3".to_string(), "c1".to_string(), "c2".to_string()]).unwrap();

        let favorites = list(&conn, Some("pl1")).unwrap();
        let ids: Vec<&str> = favorites.iter().map(|f| f.channel_id.as_str()).collect();
        assert_eq!(ids, vec!["c3", "c1", "c2"]);
    }

    #[test]
    fn re_favoriting_after_removal_appends_at_the_end() {
        let conn = test_conn();
        toggle(&conn, "c1", "pl1", FavoriteType::Channel).unwrap();
        toggle(&conn, "c2", "pl1", FavoriteType::Channel).unwrap();
        // Remove c1, then re-add it - it should land after c2, not reclaim
        // its old front-of-list position.
        toggle(&conn, "c1", "pl1", FavoriteType::Channel).unwrap();
        toggle(&conn, "c1", "pl1", FavoriteType::Channel).unwrap();

        let favorites = list(&conn, Some("pl1")).unwrap();
        let ids: Vec<&str> = favorites.iter().map(|f| f.channel_id.as_str()).collect();
        assert_eq!(ids, vec!["c2", "c1"]);
    }
}
