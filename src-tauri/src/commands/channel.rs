use crate::db;
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;
use crate::types::Channel;
use tauri::State;

#[tauri::command]
pub async fn get_channels_by_playlist(
    state: State<'_, AppState>,
    playlist_id: String,
) -> CommandResult<Vec<Channel>> {
    db::with_conn(&state.db, move |conn| Ok(db::channels::list_by_playlist(conn, &playlist_id)?)).await
}

#[tauri::command]
pub async fn search_channels(
    state: State<'_, AppState>,
    query: String,
    playlist_id: Option<String>,
) -> CommandResult<Vec<Channel>> {
    db::with_conn(&state.db, move |conn| {
        Ok(db::channels::search(conn, &query, playlist_id.as_deref())?)
    })
    .await
}

#[tauri::command]
pub async fn get_channel_by_id(state: State<'_, AppState>, id: String) -> CommandResult<Channel> {
    let lookup_id = id.clone();
    db::with_conn(&state.db, move |conn| Ok(db::channels::get_by_id(conn, &lookup_id)?))
        .await?
        .ok_or_else(|| CommandError::NotFound(format!("channel {id} not found")))
}

/// Whether a channel supports catch-up (archive replay) — a cheap gate for
/// the "watch from start" affordance, callable without a specific EPG
/// program in hand.
#[tauri::command]
pub async fn m3u_is_catchup_supported(state: State<'_, AppState>, channel_id: String) -> CommandResult<bool> {
    let lookup_id = channel_id.clone();
    let channel = db::with_conn(&state.db, move |conn| Ok(db::channels::get_by_id(conn, &lookup_id)?))
        .await?
        .ok_or_else(|| CommandError::NotFound(format!("channel {channel_id} not found")))?;
    Ok(crate::catchup::is_m3u_catchup_supported(&channel))
}

/// Builds a playable catch-up URL for `channel_id` at an EPG program's start.
/// `program_start_timestamp` (Unix seconds) takes precedence over parsing
/// `program_start` when both are given. `None` if catch-up isn't supported
/// or the URL can't be built.
#[tauri::command]
pub async fn m3u_resolve_catchup_url(
    state: State<'_, AppState>,
    channel_id: String,
    program_start: String,
    program_start_timestamp: Option<i64>,
) -> CommandResult<Option<String>> {
    let lookup_id = channel_id.clone();
    let channel = db::with_conn(&state.db, move |conn| Ok(db::channels::get_by_id(conn, &lookup_id)?))
        .await?
        .ok_or_else(|| CommandError::NotFound(format!("channel {channel_id} not found")))?;
    Ok(crate::catchup::resolve_m3u_catchup_url(&channel, &program_start, program_start_timestamp, None))
}
