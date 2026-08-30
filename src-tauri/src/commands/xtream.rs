use crate::db;
use crate::error::{CommandError, CommandResult};
use crate::net::xtream;
use crate::parsers::xmltv::{ParsedEpg, ParsedEpgChannel};
use crate::state::AppState;
use crate::types::{EpgProgram, SeriesDetails, VodContentType, VodDetails, XtreamCategory, XtreamStream, XtreamUserInfo};
use tauri::State;

/// Resolves the `(server_url, username, password)` a playlist's Xtream calls
/// need — every `xtream_*` command starts from a playlist id, never raw
/// credentials, since the frontend already has the playlist loaded.
async fn xtream_credentials(state: &AppState, playlist_id: &str) -> CommandResult<(String, String, String)> {
    let id = playlist_id.to_string();
    let playlist = db::with_conn(&state.db, move |conn| Ok(db::playlists::get(conn, &id)?))
        .await?
        .ok_or_else(|| CommandError::NotFound(format!("playlist {playlist_id} not found")))?;

    let server_url = playlist
        .server_url
        .ok_or_else(|| CommandError::Internal("Playlist has no Xtream server URL".into()))?;
    let username = playlist
        .username
        .ok_or_else(|| CommandError::Internal("Playlist has no Xtream username".into()))?;
    let password = playlist
        .password
        .ok_or_else(|| CommandError::Internal("Playlist has no Xtream password".into()))?;

    Ok((server_url, username, password))
}

#[tauri::command]
pub async fn xtream_auth(state: State<'_, AppState>, playlist_id: String) -> CommandResult<XtreamUserInfo> {
    let (server_url, username, password) = xtream_credentials(&state, &playlist_id).await?;
    xtream::get_account_info(&state.http, &server_url, &username, &password).await
}

#[tauri::command]
pub async fn xtream_get_categories(
    state: State<'_, AppState>,
    playlist_id: String,
    stream_type: String,
) -> CommandResult<Vec<XtreamCategory>> {
    let (server_url, username, password) = xtream_credentials(&state, &playlist_id).await?;
    xtream::get_categories(&state.http, &server_url, &username, &password, &stream_type).await
}

#[tauri::command]
pub async fn xtream_get_streams(
    state: State<'_, AppState>,
    playlist_id: String,
    stream_type: String,
    category_id: Option<String>,
) -> CommandResult<Vec<XtreamStream>> {
    let (server_url, username, password) = xtream_credentials(&state, &playlist_id).await?;
    xtream::get_streams(&state.http, &server_url, &username, &password, &stream_type, category_id.as_deref()).await
}

/// Reads `vod_items.detail_json` first, only hitting the network on a cache
/// miss, then persists the result. Full detail costs one provider HTTP call
/// per item with no bulk alternative, so it's fetched lazily here rather
/// than during the bulk catalog sync. A cache miss on a stale/removed item
/// just costs a live fetch with nothing persisted after.
#[tauri::command]
pub async fn xtream_get_vod_info(
    state: State<'_, AppState>,
    playlist_id: String,
    vod_id: String,
) -> CommandResult<VodDetails> {
    let id = playlist_id.clone();
    let vid = vod_id.clone();
    if let Some(cached) = db::with_conn(&state.db, move |conn| Ok(db::vod::get_detail_json(conn, &id, VodContentType::Movie, &vid)?)).await? {
        if let Ok(detail) = serde_json::from_str::<VodDetails>(&cached) {
            return Ok(detail);
        }
    }

    let (server_url, username, password) = xtream_credentials(&state, &playlist_id).await?;
    let detail = xtream::get_vod_info(&state.http, &server_url, &username, &password, &vod_id).await?;

    if let Ok(json) = serde_json::to_string(&detail) {
        let id = playlist_id.clone();
        let vid = vod_id.clone();
        let _ = db::with_conn(&state.db, move |conn| Ok(db::vod::set_detail_json(conn, &id, VodContentType::Movie, &vid, &json)?)).await;
    }
    Ok(detail)
}

/// Same cache-aside shape as `xtream_get_vod_info` — see its doc comment.
#[tauri::command]
pub async fn xtream_get_series_info(
    state: State<'_, AppState>,
    playlist_id: String,
    series_id: String,
) -> CommandResult<SeriesDetails> {
    let id = playlist_id.clone();
    let sid = series_id.clone();
    if let Some(cached) = db::with_conn(&state.db, move |conn| Ok(db::vod::get_detail_json(conn, &id, VodContentType::Series, &sid)?)).await? {
        if let Ok(detail) = serde_json::from_str::<SeriesDetails>(&cached) {
            return Ok(detail);
        }
    }

    let (server_url, username, password) = xtream_credentials(&state, &playlist_id).await?;
    let detail = xtream::get_series_info(&state.http, &server_url, &username, &password, &series_id).await?;

    if let Ok(json) = serde_json::to_string(&detail) {
        let id = playlist_id.clone();
        let sid = series_id.clone();
        let _ = db::with_conn(&state.db, move |conn| Ok(db::vod::set_detail_json(conn, &id, VodContentType::Series, &sid, &json)?)).await;
    }
    Ok(detail)
}

/// Builds a catch-up (archive replay) playback URL for one live stream at
/// an EPG program's `[start_timestamp, stop_timestamp)` window (Unix
/// seconds) — see `catchup::resolve_xtream_catchup_url`'s doc comment for
/// exactly which URL scheme/variant this produces and why.
#[tauri::command]
pub async fn xtream_resolve_catchup_url(
    state: State<'_, AppState>,
    playlist_id: String,
    stream_id: i64,
    start_timestamp: i64,
    stop_timestamp: i64,
) -> CommandResult<String> {
    let id = playlist_id.clone();
    let playlist = db::with_conn(&state.db, move |conn| Ok(db::playlists::get(conn, &id)?))
        .await?
        .ok_or_else(|| CommandError::NotFound(format!("playlist {playlist_id} not found")))?;
    let server_url = playlist
        .server_url
        .ok_or_else(|| CommandError::Internal("Playlist has no Xtream server URL".into()))?;
    let username = playlist
        .username
        .ok_or_else(|| CommandError::Internal("Playlist has no Xtream username".into()))?;
    let password = playlist
        .password
        .ok_or_else(|| CommandError::Internal("Playlist has no Xtream password".into()))?;

    Ok(crate::catchup::resolve_xtream_catchup_url(
        &server_url,
        &username,
        &password,
        stream_id,
        start_timestamp,
        stop_timestamp,
        playlist.server_timezone.as_deref(),
    ))
}

/// Whether an Xtream live stream supports catch-up (archive replay)
/// playback at all — a pure gate for showing/hiding a "watch from start"
/// affordance, mirroring `m3u_is_catchup_supported` for M3U/Xtream's own
/// `tv_archive`/`tv_archive_duration` fields.
#[tauri::command]
pub fn xtream_catchup_available(tv_archive: Option<i64>, tv_archive_duration: Option<i64>) -> bool {
    crate::catchup::xtream_catchup_available(tv_archive, tv_archive_duration)
}

/// Extracts `(xtream_stream_id, tv_archive, tv_archive_duration)` stashed in
/// a channel's `raw` JSON by `xtream::stream_to_channel` — `None` for any
/// channel that isn't an Xtream live channel (wrong shape, or missing
/// entirely for M3U/Stalker rows).
fn xtream_channel_meta(raw: Option<&str>) -> Option<(i64, Option<i64>, Option<i64>)> {
    let value: serde_json::Value = serde_json::from_str(raw?).ok()?;
    let stream_id = value.get("xtream_stream_id")?.as_i64()?;
    let tv_archive = value.get("tv_archive").and_then(|v| v.as_i64());
    let tv_archive_duration = value.get("tv_archive_duration").and_then(|v| v.as_i64());
    Some((stream_id, tv_archive, tv_archive_duration))
}

/// Whether a stored channel (by unified `Channel.id`) supports Xtream
/// catch-up - convenience wrapper for callers with only a channel id, not
/// the raw `tv_archive`/`tv_archive_duration` values.
#[tauri::command]
pub async fn xtream_channel_catchup_available(state: State<'_, AppState>, channel_id: String) -> CommandResult<bool> {
    let lookup_id = channel_id.clone();
    let channel = db::with_conn(&state.db, move |conn| Ok(db::channels::get_by_id(conn, &lookup_id)?))
        .await?
        .ok_or_else(|| CommandError::NotFound(format!("channel {channel_id} not found")))?;
    let Some((_, tv_archive, tv_archive_duration)) = xtream_channel_meta(channel.raw.as_deref()) else {
        return Ok(false);
    };
    Ok(crate::catchup::xtream_catchup_available(tv_archive, tv_archive_duration))
}

/// Resolves a catch-up URL for a stored channel at an EPG program's
/// `[start_timestamp, stop_timestamp)` window - looks up the stream id and
/// owning playlist's credentials itself. `None` means not catch-up-eligible
/// (distinct from a genuine resolution failure, which still errors).
#[tauri::command]
pub async fn xtream_resolve_catchup_url_for_channel(
    state: State<'_, AppState>,
    channel_id: String,
    start_timestamp: i64,
    stop_timestamp: i64,
) -> CommandResult<Option<String>> {
    let lookup_id = channel_id.clone();
    let channel = db::with_conn(&state.db, move |conn| Ok(db::channels::get_by_id(conn, &lookup_id)?))
        .await?
        .ok_or_else(|| CommandError::NotFound(format!("channel {channel_id} not found")))?;
    let Some((stream_id, ..)) = xtream_channel_meta(channel.raw.as_deref()) else {
        return Ok(None);
    };

    let lookup_id = channel_id.clone();
    let playlist_id = db::with_conn(&state.db, move |conn| Ok(db::channels::get_playlist_id(conn, &lookup_id)?))
        .await?
        .ok_or_else(|| CommandError::NotFound(format!("channel {channel_id} not found")))?;
    let playlist = db::with_conn(&state.db, move |conn| Ok(db::playlists::get(conn, &playlist_id)?))
        .await?
        .ok_or_else(|| CommandError::NotFound("Owning playlist not found".into()))?;

    let server_url = playlist
        .server_url
        .ok_or_else(|| CommandError::Internal("Playlist has no Xtream server URL".into()))?;
    let username = playlist
        .username
        .ok_or_else(|| CommandError::Internal("Playlist has no Xtream username".into()))?;
    let password = playlist
        .password
        .ok_or_else(|| CommandError::Internal("Playlist has no Xtream password".into()))?;

    Ok(Some(crate::catchup::resolve_xtream_catchup_url(
        &server_url,
        &username,
        &password,
        stream_id,
        start_timestamp,
        stop_timestamp,
        playlist.server_timezone.as_deref(),
    )))
}

/// A quick "now/next"-style peek at one stream's upcoming EPG, fetched
/// fresh and returned directly — not persisted, for callers that just want
/// a few upcoming programs immediately (e.g. a channel-list row preview)
/// without waiting on/triggering a full sync.
#[tauri::command]
pub async fn xtream_get_short_epg(
    state: State<'_, AppState>,
    playlist_id: String,
    stream_id: i64,
    limit: Option<i64>,
) -> CommandResult<Vec<EpgProgram>> {
    let (server_url, username, password) = xtream_credentials(&state, &playlist_id).await?;
    let programs = xtream::get_short_epg(&state.http, &server_url, &username, &password, stream_id, limit.unwrap_or(10), "").await?;
    Ok(programs
        .into_iter()
        .map(|p| EpgProgram {
            id: uuid::Uuid::new_v4().to_string(),
            channel_id: String::new(),
            start: p.start,
            stop: p.stop,
            title: p.title,
            description: p.description,
            category: p.category,
            icon: p.icon_url,
        })
        .collect())
}

/// Fetches the full multi-day EPG for one Xtream channel and stores it
/// through the same `epg_channels`/`epg_programs` pipeline XMLTV imports use
/// (its own synthetic `source_url` so it never collides with an XMLTV
/// guide) - existing EPG lookups find it via `tvg_id` or a display-name
/// match, with no changes needed.
#[tauri::command]
pub async fn xtream_sync_epg(
    state: State<'_, AppState>,
    playlist_id: String,
    channel_id: String,
    stream_id: i64,
) -> CommandResult<()> {
    let (server_url, username, password) = xtream_credentials(&state, &playlist_id).await?;
    let lookup_id = channel_id.clone();
    let channel = db::with_conn(&state.db, move |conn| Ok(db::channels::get_by_id(conn, &lookup_id)?))
        .await?
        .ok_or_else(|| CommandError::NotFound(format!("channel {channel_id} not found")))?;
    let channel_key = channel
        .tvg
        .id
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("xtream-epg-stream:{stream_id}"));

    let programs = xtream::get_full_epg(&state.http, &server_url, &username, &password, stream_id, &channel_key).await?;
    let source_url = format!("xtream-epg:{playlist_id}:{stream_id}");
    let parsed = ParsedEpg {
        channels: vec![ParsedEpgChannel { id: channel_key, display_name: channel.name.clone(), icon_url: None }],
        programs,
    };
    db::with_conn(&state.db, move |conn| Ok(db::epg::store(conn, &source_url, &parsed)?)).await
}
