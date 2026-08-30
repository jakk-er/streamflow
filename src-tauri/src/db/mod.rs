pub mod channels;
pub mod downloads;
pub mod epg;
pub mod favorites;
pub mod itv_recovery;
pub mod playlists;
pub mod schema;
pub mod vod;
pub mod vod_progress;

use crate::error::{CommandError, CommandResult};
use crate::types::AppSettings;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::path::Path;

pub type DbPool = r2d2::Pool<SqliteConnectionManager>;

/// Runs `f` against a pooled connection on a blocking thread, keeping the
/// pooled connection's lifetime entirely inside the blocking closure (never
/// held across an `.await`) so there's no `Send`-across-await hazard.
pub async fn with_conn<F, T>(pool: &DbPool, f: F) -> CommandResult<T>
where
    F: FnOnce(&mut Connection) -> CommandResult<T> + Send + 'static,
    T: Send + 'static,
{
    let pool = pool.clone();
    let join_result = tauri::async_runtime::spawn_blocking(move || -> CommandResult<T> {
        let mut conn = pool.get()?;
        f(&mut conn)
    })
    .await;

    match join_result {
        Ok(inner) => inner,
        Err(e) => Err(CommandError::Internal(format!("background task failed: {e}"))),
    }
}

/// Reads the single-row app settings blob, falling back to defaults if unset
/// or corrupt. Shared with anything that needs to check a setting
/// server-side without a second IPC round trip.
pub fn read_settings(conn: &Connection) -> AppSettings {
    let payload: Option<String> = conn
        .query_row("SELECT payload FROM settings WHERE id = 1", [], |row| row.get(0))
        .ok();
    match payload {
        Some(json) => serde_json::from_str(&json).unwrap_or_default(),
        None => AppSettings::default(),
    }
}

pub fn init_pool(db_path: &Path) -> anyhow::Result<DbPool> {
    let manager = SqliteConnectionManager::file(db_path).with_init(|conn: &mut Connection| {
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )
    });

    let pool = r2d2::Pool::builder().max_size(8).build(manager)?;

    {
        let conn = pool.get()?;
        schema::run_migrations(&conn)?;
    }

    Ok(pool)
}
