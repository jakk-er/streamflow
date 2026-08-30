use rusqlite::Connection;
use std::collections::HashMap;

/// A row existing at all means this ITV category id is known to be missing
/// from the Stalker bulk `get_all_channels` endpoint (a "censored"/excluded
/// genre). See `commands::stalker::sync_channels_category_aware` for how
/// this decides whether a category needs re-crawling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStatus {
    InProgress,
    Synced,
    Failed,
}

impl RecoveryStatus {
    /// Unknown strings collapse to `Failed` rather than panicking — purely
    /// defensive, since the column's CHECK constraint already guarantees
    /// one of the three known values.
    fn from_str(s: &str) -> Self {
        match s {
            "in_progress" => RecoveryStatus::InProgress,
            "synced" => RecoveryStatus::Synced,
            _ => RecoveryStatus::Failed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CategoryState {
    pub status: RecoveryStatus,
    pub channel_count: i64,
    /// Only ever set by `mark_synced` — a failed attempt never touches it,
    /// so it always reflects the last confirmed-good sync, not last attempt.
    pub last_synced_at: Option<String>,
    pub last_attempt_at: String,
}

pub fn list_state(conn: &Connection, playlist_id: &str) -> rusqlite::Result<HashMap<String, CategoryState>> {
    let mut stmt = conn.prepare(
        "SELECT category_id, status, channel_count, last_synced_at, last_attempt_at
         FROM itv_recovery_category_state WHERE playlist_id = ?1",
    )?;
    let rows = stmt.query_map([playlist_id], |row| {
        let category_id: String = row.get(0)?;
        let status: String = row.get(1)?;
        Ok((
            category_id,
            CategoryState {
                status: RecoveryStatus::from_str(&status),
                channel_count: row.get(2)?,
                last_synced_at: row.get(3)?,
                last_attempt_at: row.get(4)?,
            },
        ))
    })?;
    rows.collect()
}

/// Set the instant a category's crawl starts. If the app crashes mid-crawl,
/// the row is just left at `in_progress` with a stale timestamp — freshness
/// checks only trust `Synced` rows, so it's naturally retried, no cleanup
/// needed.
pub fn mark_in_progress(conn: &Connection, playlist_id: &str, category_id: &str, attempt_at: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO itv_recovery_category_state (playlist_id, category_id, status, last_attempt_at)
         VALUES (?1, ?2, 'in_progress', ?3)
         ON CONFLICT(playlist_id, category_id) DO UPDATE SET
            status = 'in_progress', last_attempt_at = excluded.last_attempt_at",
        rusqlite::params![playlist_id, category_id, attempt_at],
    )?;
    Ok(())
}

/// `channel_count` here is informational (visibility into what a category
/// held as of its last successful sync) — it is NOT used to decide
/// freshness; only `last_synced_at` age is.
pub fn mark_synced(
    conn: &Connection,
    playlist_id: &str,
    category_id: &str,
    synced_at: &str,
    channel_count: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO itv_recovery_category_state (playlist_id, category_id, status, channel_count, last_synced_at, last_attempt_at)
         VALUES (?1, ?2, 'synced', ?3, ?4, ?4)
         ON CONFLICT(playlist_id, category_id) DO UPDATE SET
            status = 'synced', channel_count = excluded.channel_count,
            last_synced_at = excluded.last_synced_at, last_attempt_at = excluded.last_attempt_at",
        rusqlite::params![playlist_id, category_id, channel_count, synced_at],
    )?;
    Ok(())
}

/// Deliberately doesn't touch `last_synced_at`/`channel_count` — a failed
/// attempt must never erase the last known-good timestamp, or every
/// subsequent sync would treat this category as never-synced and retry
/// forever despite good data still sitting in `channels`.
pub fn mark_failed(conn: &Connection, playlist_id: &str, category_id: &str, attempt_at: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO itv_recovery_category_state (playlist_id, category_id, status, last_attempt_at)
         VALUES (?1, ?2, 'failed', ?3)
         ON CONFLICT(playlist_id, category_id) DO UPDATE SET
            status = 'failed', last_attempt_at = excluded.last_attempt_at",
        rusqlite::params![playlist_id, category_id, attempt_at],
    )?;
    Ok(())
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

    #[test]
    fn in_progress_then_synced_round_trips() {
        let conn = test_conn();
        mark_in_progress(&conn, "pl1", "cat-a", "2026-01-01T00:00:00Z").unwrap();
        let state = list_state(&conn, "pl1").unwrap();
        assert_eq!(state["cat-a"].status, RecoveryStatus::InProgress);
        assert_eq!(state["cat-a"].last_synced_at, None);

        mark_synced(&conn, "pl1", "cat-a", "2026-01-01T00:00:05Z", 14).unwrap();
        let state = list_state(&conn, "pl1").unwrap();
        assert_eq!(state["cat-a"].status, RecoveryStatus::Synced);
        assert_eq!(state["cat-a"].channel_count, 14);
        assert_eq!(state["cat-a"].last_synced_at.as_deref(), Some("2026-01-01T00:00:05Z"));
    }

    #[test]
    fn failed_attempt_preserves_prior_last_synced_at() {
        let conn = test_conn();
        mark_synced(&conn, "pl1", "cat-a", "2026-01-01T00:00:00Z", 20).unwrap();
        mark_in_progress(&conn, "pl1", "cat-a", "2026-01-02T00:00:00Z").unwrap();
        mark_failed(&conn, "pl1", "cat-a", "2026-01-02T00:00:05Z").unwrap();

        let state = list_state(&conn, "pl1").unwrap();
        assert_eq!(state["cat-a"].status, RecoveryStatus::Failed);
        // The successful sync from a day earlier must still be intact.
        assert_eq!(state["cat-a"].last_synced_at.as_deref(), Some("2026-01-01T00:00:00Z"));
        assert_eq!(state["cat-a"].channel_count, 20);
        assert_eq!(state["cat-a"].last_attempt_at, "2026-01-02T00:00:05Z");
    }

    #[test]
    fn list_state_is_scoped_per_playlist() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO playlists (id, title, playlist_type, import_date, last_usage) VALUES ('pl2', 'Test2', 'stalker', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        mark_synced(&conn, "pl1", "cat-a", "2026-01-01T00:00:00Z", 5).unwrap();
        mark_synced(&conn, "pl2", "cat-a", "2026-01-01T00:00:00Z", 9).unwrap();

        assert_eq!(list_state(&conn, "pl1").unwrap()["cat-a"].channel_count, 5);
        assert_eq!(list_state(&conn, "pl2").unwrap()["cat-a"].channel_count, 9);
    }
}
