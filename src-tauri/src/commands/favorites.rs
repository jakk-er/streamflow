use crate::db;
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;
use crate::types::{FavoriteChannel, FavoriteType, WatchHistoryItem};
use tauri::State;

#[tauri::command]
pub async fn toggle_favorite(
    state: State<'_, AppState>,
    channel_id: String,
    playlist_id: String,
    favorite_type: String,
) -> CommandResult<bool> {
    let favorite_type = FavoriteType::from_str(&favorite_type)
        .ok_or_else(|| CommandError::Api(format!("Invalid favorite type: {favorite_type}")))?;
    db::with_conn(&state.db, move |conn| {
        Ok(db::favorites::toggle(conn, &channel_id, &playlist_id, favorite_type)?)
    })
    .await
}

#[tauri::command]
pub async fn get_favorites(
    state: State<'_, AppState>,
    playlist_id: Option<String>,
) -> CommandResult<Vec<FavoriteChannel>> {
    db::with_conn(&state.db, move |conn| {
        Ok(db::favorites::list(conn, playlist_id.as_deref())?)
    })
    .await
}

#[tauri::command]
pub async fn reorder_favorites(
    state: State<'_, AppState>,
    playlist_id: String,
    ordered_channel_ids: Vec<String>,
) -> CommandResult<()> {
    db::with_conn(&state.db, move |conn| {
        Ok(db::favorites::reorder(conn, &playlist_id, &ordered_channel_ids)?)
    })
    .await
}

#[tauri::command]
pub async fn get_recently_watched(
    state: State<'_, AppState>,
    limit: i64,
) -> CommandResult<Vec<WatchHistoryItem>> {
    db::with_conn(&state.db, move |conn| Ok(db::favorites::recently_watched(conn, limit)?)).await
}

#[tauri::command]
pub async fn save_playback_position(
    state: State<'_, AppState>,
    channel_id: String,
    playlist_id: String,
    position_seconds: i64,
    total_seconds: i64,
) -> CommandResult<()> {
    db::with_conn(&state.db, move |conn| {
        if !db::read_settings(conn).track_watch_history {
            return Ok(());
        }
        Ok(db::favorites::save_playback_position(
            conn,
            &channel_id,
            &playlist_id,
            position_seconds,
            total_seconds,
        )?)
    })
    .await
}

#[tauri::command]
pub async fn remove_watch_history_item(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    db::with_conn(&state.db, move |conn| Ok(db::favorites::remove_history_item(conn, &id)?)).await
}

#[tauri::command]
pub async fn clear_watch_history(state: State<'_, AppState>) -> CommandResult<()> {
    db::with_conn(&state.db, |conn| Ok(db::favorites::clear_history(conn)?)).await
}
