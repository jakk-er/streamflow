use crate::commands::stalker::sync_channels_category_aware;
use crate::db;
use crate::error::{CommandError, CommandResult};
use crate::net::stalker::auth::StalkerCredentials;
use crate::net::stalker::{auth as stalker_auth, identity as stalker_identity};
use crate::net::xtream;
use crate::parsers::m3u;
use crate::state::AppState;
use crate::types::{Channel, Playlist, PlaylistType, StalkerAuthOutcome};
use chrono::Utc;
use serde::Serialize;
use std::collections::HashMap;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StalkerAddResult {
    pub playlist: Playlist,
    pub outcome: StalkerAuthOutcome,
}

async fn fetch_m3u_text(
    http: &reqwest::Client,
    url: &str,
    user_agent: Option<&str>,
) -> CommandResult<String> {
    let mut request = http.get(url);
    if let Some(ua) = user_agent.filter(|s| !s.is_empty()) {
        request = request.header(reqwest::header::USER_AGENT, ua);
    }
    let response = request
        .send()
        .await
        .map_err(|e| CommandError::Api(format!("Failed to fetch playlist: {e}")))?;
    if !response.status().is_success() {
        return Err(CommandError::Api(format!(
            "Playlist server responded with status {}",
            response.status()
        )));
    }
    response
        .text()
        .await
        .map_err(|e| CommandError::Api(format!("Failed to read playlist response: {e}")))
}

/// Fetches Xtream live categories + streams and maps them into the unified
/// `Channel` shape. Shared by `add_xtream_playlist` and `refresh_playlist`'s
/// Xtream branch. Category lookup failing (some panels omit it) degrades to
/// blank group titles rather than failing the whole sync.
async fn fetch_xtream_live_channels(
    http: &reqwest::Client,
    server_url: &str,
    username: &str,
    password: &str,
    format: &str,
) -> CommandResult<Vec<Channel>> {
    let categories = xtream::get_categories(http, server_url, username, password, "live")
        .await
        .unwrap_or_default();
    let category_names: HashMap<String, String> = categories
        .into_iter()
        .map(|c| (c.category_id, c.category_name))
        .collect();

    let streams = xtream::get_streams(http, server_url, username, password, "live", None).await?;
    Ok(streams
        .iter()
        .map(|s| {
            xtream::stream_to_channel(
                s,
                category_names.get(&s.category_id).map(String::as_str),
                server_url,
                username,
                password,
                format,
            )
        })
        .collect())
}

/// Xtream has no per-playlist header UI (unlike M3U's `#EXTVLCOPT` tags) -
/// without a default, requests went out with no `User-Agent`/`Referer`/
/// `Origin`, which some CDNs 403 (hotlink protection). Defaults to the VLC
/// UA proven to work against `player_api.php` and the panel's base URL as
/// Referer/Origin. Only fills a gap, never overwrites an existing value.
fn apply_xtream_default_headers(playlist: &mut Playlist, server_url: &str) {
    let is_empty = |v: &Option<String>| v.as_deref().map(str::is_empty).unwrap_or(true);
    if is_empty(&playlist.user_agent) {
        playlist.user_agent = Some(xtream::XTREAM_USER_AGENT.to_string());
    }
    let origin = server_url.trim_end_matches('/').to_string();
    if is_empty(&playlist.referrer) {
        playlist.referrer = Some(origin.clone());
    }
    if is_empty(&playlist.origin) {
        playlist.origin = Some(origin);
    }
}

/// Applies M3U-header-detected EPG URLs to a playlist. `detectedEpgUrls`
/// always reflects the latest parse, but the *enabled* set (`epgUrls`) is
/// only auto-populated the first time (up to 5 URLs) so a later refresh
/// doesn't silently re-enable a URL the user disabled or touch a set
/// they've customized.
fn apply_detected_epg_urls(playlist: &mut Playlist, detected: Vec<String>) {
    if detected.is_empty() {
        return;
    }
    let disabled: std::collections::HashSet<String> = playlist.disabled_epg_urls.clone().unwrap_or_default().into_iter().collect();
    playlist.detected_epg_urls = Some(detected.clone());
    if playlist.epg_urls.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
        let enabled: Vec<String> = detected.into_iter().filter(|u| !disabled.contains(u)).take(5).collect();
        if !enabled.is_empty() {
            playlist.epg_urls = Some(enabled);
        }
    }
}

/// Best-effort background fetch of a playlist's enabled EPG URLs, mirroring
/// iptvnator's playlist-local EPG auto-fetch on import/refresh — never
/// blocks the calling command; failures are logged, not surfaced (an EPG
/// source being temporarily down shouldn't fail a playlist import).
fn spawn_playlist_epg_fetch(state: &AppState, epg_urls: Option<&[String]>) {
    let Some(urls) = epg_urls.filter(|u| !u.is_empty()) else { return };
    let http = state.http.clone();
    let db = state.db.clone();
    let urls = urls.to_vec();
    tauri::async_runtime::spawn(async move {
        for url in urls {
            if let Err(e) = crate::commands::epg::fetch_and_store_epg(&http, &db, &url).await {
                tracing::warn!("Playlist-declared EPG fetch failed for {url}: {e}");
            }
        }
    });
}

/// Non-blocking VOD/series catalog sync, spawned after a playlist's channel
/// sync/add succeeds. Must run in the background: the VOD/series catalog is
/// typically far larger than the live channel list, and add/refresh must
/// not hang on it.
fn spawn_vod_sync(state: &AppState, playlist: Playlist) {
    let scoped_state = AppState::detached(state.db.clone(), state.http.clone(), state.app.clone());
    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::commands::vod::vod_sync_playlist(&scoped_state, &playlist).await {
            tracing::warn!("VOD sync failed for playlist '{}' ({}): {e}", playlist.title, playlist.id);
        }
    });
}

fn empty_playlist(id: String, title: String, playlist_type: PlaylistType, now: String) -> Playlist {
    Playlist {
        id,
        title,
        filename: None,
        playlist_type,
        import_date: now.clone(),
        last_usage: now,
        count: 0,
        url: None,
        user_agent: None,
        referrer: None,
        origin: None,
        file_path: None,
        epg_urls: None,
        detected_epg_urls: None,
        manual_epg_urls: None,
        disabled_epg_urls: None,
        auto_refresh: false,
        update_date: None,
        update_state: None,
        position: None,
        is_temporary: None,
        server_url: None,
        username: None,
        password: None,
        mac_address: None,
        portal_url: None,
        is_full_stalker_portal: None,
        server_timezone: None,
        stalker_token: None,
        stalker_session_identity: None,
        stalker_watchdog_timeout: None,
        stalker_timeslot: None,
        stalker_serial_number: None,
        stalker_device_id1: None,
        stalker_device_id2: None,
        stalker_signature1: None,
        stalker_signature2: None,
        stalker_account_info: None,
        hidden_group_titles: None,
        stalker_login_completed: None,
        stalker_not_valid: None,
        stalker_endpoint: None,
    }
}

#[tauri::command]
pub async fn import_m3u_playlist(
    state: State<'_, AppState>,
    url: String,
    title: String,
    user_agent: Option<String>,
    auto_refresh: Option<bool>,
) -> CommandResult<Playlist> {
    let trimmed_url = url.trim().to_string();
    if trimmed_url.is_empty() {
        return Err(CommandError::Api("Playlist URL is required".into()));
    }

    let content = fetch_m3u_text(&state.http, &trimmed_url, user_agent.as_deref()).await?;
    let parsed = m3u::parse(&content);
    if parsed.items.is_empty() {
        return Err(CommandError::InvalidResponse("No channels found in playlist".into()));
    }

    let now = Utc::now().to_rfc3339();
    let mut playlist = empty_playlist(uuid::Uuid::new_v4().to_string(), title, PlaylistType::M3u, now);
    playlist.url = Some(trimmed_url);
    playlist.user_agent = user_agent;
    playlist.count = parsed.items.len() as i64;
    playlist.auto_refresh = auto_refresh.unwrap_or(false);
    apply_detected_epg_urls(&mut playlist, parsed.detected_epg_urls);

    let result = playlist.clone();
    let items = parsed.items;
    db::with_conn(&state.db, move |conn| {
        db::playlists::insert(conn, &playlist)?;
        db::channels::insert_m3u_items(conn, &playlist.id, &items)?;
        Ok(())
    })
    .await?;

    spawn_playlist_epg_fetch(&state, result.epg_urls.as_deref());
    Ok(result)
}

/// Edits an existing M3U playlist: re-fetches and re-parses the (possibly
/// changed) URL before writing anything, so a typo doesn't overwrite a
/// working playlist with a broken one.
#[tauri::command]
pub async fn update_m3u_playlist(
    state: State<'_, AppState>,
    id: String,
    url: String,
    title: String,
    user_agent: Option<String>,
    auto_refresh: Option<bool>,
) -> CommandResult<Playlist> {
    let trimmed_url = url.trim().to_string();
    if trimmed_url.is_empty() {
        return Err(CommandError::Api("Playlist URL is required".into()));
    }

    let existing = db::with_conn(&state.db, {
        let id = id.clone();
        move |conn| Ok(db::playlists::get(conn, &id)?)
    })
    .await?
    .ok_or_else(|| CommandError::NotFound(format!("playlist {id} not found")))?;

    let content = fetch_m3u_text(&state.http, &trimmed_url, user_agent.as_deref()).await?;
    let parsed = m3u::parse(&content);
    if parsed.items.is_empty() {
        return Err(CommandError::InvalidResponse("No channels found in playlist".into()));
    }

    let mut playlist = existing;
    playlist.title = title;
    playlist.url = Some(trimmed_url);
    playlist.user_agent = user_agent;
    playlist.count = parsed.items.len() as i64;
    playlist.update_date = Some(Utc::now().to_rfc3339());
    if let Some(auto_refresh) = auto_refresh {
        playlist.auto_refresh = auto_refresh;
    }
    apply_detected_epg_urls(&mut playlist, parsed.detected_epg_urls);

    let result = playlist.clone();
    let items = parsed.items;
    db::with_conn(&state.db, move |conn| {
        db::playlists::update(conn, &playlist)?;
        db::channels::delete_by_playlist(conn, &playlist.id)?;
        db::channels::insert_m3u_items(conn, &playlist.id, &items)?;
        Ok(())
    })
    .await?;

    spawn_playlist_epg_fetch(&state, result.epg_urls.as_deref());
    Ok(result)
}

#[tauri::command]
pub async fn add_xtream_playlist(
    state: State<'_, AppState>,
    base_url: String,
    username: String,
    password: String,
    title: String,
    user_agent: Option<String>,
    auto_refresh: Option<bool>,
) -> CommandResult<Playlist> {
    let server_url = xtream::normalize_server_url(&base_url)?;
    let user_info = xtream::get_account_info(&state.http, &server_url, &username, &password).await?;
    let format = xtream::preferred_live_format(user_info.allowed_output_formats.as_deref());

    let channels = fetch_xtream_live_channels(&state.http, &server_url, &username, &password, &format).await?;

    let now = Utc::now().to_rfc3339();
    let mut playlist = empty_playlist(uuid::Uuid::new_v4().to_string(), title, PlaylistType::Xtream, now);
    playlist.server_url = Some(server_url.clone());
    playlist.username = Some(username);
    playlist.password = Some(password);
    playlist.user_agent = user_agent;
    playlist.count = channels.len() as i64;
    playlist.auto_refresh = auto_refresh.unwrap_or(false);
    apply_xtream_default_headers(&mut playlist, &server_url);
    playlist.server_timezone = user_info.server_info.as_ref().and_then(|s| s.timezone.clone());

    let result = playlist.clone();
    db::with_conn(&state.db, move |conn| {
        db::playlists::insert(conn, &playlist)?;
        db::channels::insert_channels(conn, &playlist.id, &channels)?;
        Ok(())
    })
    .await?;

    spawn_vod_sync(&state, result.clone());
    Ok(result)
}

/// Edits an existing Xtream playlist: re-authenticates and re-fetches the
/// live channel list before writing anything, so a typo doesn't overwrite a
/// working playlist with a broken one.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn update_xtream_playlist(
    state: State<'_, AppState>,
    id: String,
    base_url: String,
    username: String,
    password: String,
    title: String,
    user_agent: Option<String>,
    auto_refresh: Option<bool>,
) -> CommandResult<Playlist> {
    let existing = db::with_conn(&state.db, {
        let id = id.clone();
        move |conn| Ok(db::playlists::get(conn, &id)?)
    })
    .await?
    .ok_or_else(|| CommandError::NotFound(format!("playlist {id} not found")))?;

    let server_url = xtream::normalize_server_url(&base_url)?;
    let user_info = xtream::get_account_info(&state.http, &server_url, &username, &password).await?;
    let format = xtream::preferred_live_format(user_info.allowed_output_formats.as_deref());
    let channels = fetch_xtream_live_channels(&state.http, &server_url, &username, &password, &format).await?;

    let mut playlist = existing;
    playlist.title = title;
    playlist.server_url = Some(server_url.clone());
    playlist.username = Some(username);
    playlist.password = Some(password);
    playlist.user_agent = user_agent;
    playlist.count = channels.len() as i64;
    playlist.update_date = Some(Utc::now().to_rfc3339());
    if let Some(auto_refresh) = auto_refresh {
        playlist.auto_refresh = auto_refresh;
    }
    apply_xtream_default_headers(&mut playlist, &server_url);
    playlist.server_timezone = user_info.server_info.as_ref().and_then(|s| s.timezone.clone());

    let result = playlist.clone();
    db::with_conn(&state.db, move |conn| {
        db::playlists::update(conn, &playlist)?;
        db::channels::delete_by_playlist(conn, &playlist.id)?;
        db::channels::insert_channels(conn, &playlist.id, &channels)?;
        Ok(())
    })
    .await?;

    spawn_vod_sync(&state, result.clone());
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn add_stalker_playlist(
    state: State<'_, AppState>,
    server_url: String,
    mac_address: String,
    title: String,
    user_agent: Option<String>,
    username: Option<String>,
    password: Option<String>,
    device_id1: Option<String>,
    device_id2: Option<String>,
    serial_number: Option<String>,
    signature1: Option<String>,
    signature2: Option<String>,
    auto_refresh: Option<bool>,
) -> CommandResult<StalkerAddResult> {
    let portal_url = server_url.trim().to_string();
    if portal_url.is_empty() {
        return Err(CommandError::Api("Portal URL is required".into()));
    }
    // Canonicalized (uppercase, colon-separated) - a real portal keys
    // MAC-bound records on this exact form, so a differently-formatted MAC
    // would produce a different `Cookie: mac=...`/SHA-1 `prehash`.
    let mac = stalker_identity::normalize_mac(&mac_address);
    if mac.is_empty() {
        return Err(CommandError::Api("MAC address is required".into()));
    }

    // The typed URL is almost never the real API script (portals serve an
    // HTML shell at the bare URL) - discover the working endpoint and portal
    // MODE first (many reseller panels skip the handshake/token flow
    // entirely - "simple" portals; see `discover_portal_endpoint`). Its
    // session is used as-is: re-authenticating here would put a second
    // `get_profile` on the wire, which permanently rebinds `device_id` to
    // the MAC and used to break portals with advanced identity fields set.
    let discovered = stalker_auth::discover_portal_endpoint(
        &state.http,
        &StalkerCredentials {
            portal_url: &portal_url,
            mac_address: &mac,
            serial_number: serial_number.as_deref(),
            device_id: device_id1.as_deref(),
            device_id2: device_id2.as_deref(),
            signature1: signature1.as_deref(),
            signature2: signature2.as_deref(),
        },
        Default::default(),
        username.as_deref(),
        password.as_deref(),
    )
    .await?;
    let endpoint = discovered.endpoint;
    let is_full_portal = discovered.full_portal;

    // A simple portal's identity is narrower (no serial/device-id/signature)
    // - matches `commands::stalker::build_creds`'s stripping rule.
    let creds = if is_full_portal {
        StalkerCredentials {
            portal_url: &endpoint,
            mac_address: &mac,
            serial_number: serial_number.as_deref(),
            device_id: device_id1.as_deref(),
            device_id2: device_id2.as_deref(),
            signature1: signature1.as_deref(),
            signature2: signature2.as_deref(),
        }
    } else {
        StalkerCredentials {
            portal_url: &endpoint,
            mac_address: &mac,
            serial_number: None,
            device_id: None,
            device_id2: None,
            signature1: None,
            signature2: None,
        }
    };
    let outcome = match discovered.outcome {
        Some(outcome) => outcome,
        // No handshake for a simple portal - synthesized as a `Success`
        // outcome so every downstream step runs unchanged.
        None => StalkerAuthOutcome::Success {
            session: crate::types::StalkerSessionInfo {
                token: String::new(),
                endpoint: endpoint.clone(),
                full_portal: false,
                watchdog_timeout: 0,
                timeslot: 0,
                not_valid: false,
                login_completed: true,
                session_fingerprint: String::new(),
            },
        },
    };

    let now = Utc::now().to_rfc3339();
    let mut playlist = empty_playlist(uuid::Uuid::new_v4().to_string(), title, PlaylistType::Stalker, now);
    // `portal_url` keeps the user's original input (for display); the
    // discovered `.php` endpoint is what every future call actually uses.
    playlist.portal_url = Some(portal_url);
    playlist.stalker_endpoint = Some(endpoint.clone());
    playlist.mac_address = Some(mac.clone());
    playlist.user_agent = user_agent;
    playlist.username = username;
    playlist.password = password;
    playlist.stalker_device_id1 = device_id1.clone();
    playlist.stalker_device_id2 = device_id2.clone();
    playlist.stalker_serial_number = serial_number.clone();
    playlist.stalker_signature1 = signature1.clone();
    playlist.stalker_signature2 = signature2.clone();
    playlist.auto_refresh = auto_refresh.unwrap_or(false);

    if let StalkerAuthOutcome::Success { ref session } = outcome {
        playlist.stalker_token = Some(session.token.clone());
        playlist.stalker_endpoint = Some(session.endpoint.clone());
        playlist.is_full_stalker_portal = Some(session.full_portal);
        playlist.stalker_watchdog_timeout = Some(session.watchdog_timeout);
        playlist.stalker_timeslot = Some(session.timeslot);
        playlist.stalker_not_valid = Some(session.not_valid);
        playlist.stalker_login_completed = Some(session.login_completed);
        playlist.stalker_session_identity = Some(session.session_fingerprint.clone());
    }

    let result = playlist.clone();
    db::with_conn(&state.db, move |conn| {
        db::playlists::insert(conn, &playlist)?;
        Ok(())
    })
    .await?;

    // A successful auth implicitly triggers a full channel sync (mirrors
    // `add_xtream_playlist`). Best-effort: a failure here doesn't fail the
    // add since the playlist row was already created; retry via
    // `refresh_playlist`.
    if let StalkerAuthOutcome::Success { ref session } = outcome {
        let _ = sync_channels_category_aware(&state, &result.id, &creds, &session.token).await;
        // No eager VOD sync - Stalker VOD/series browsing is remote-first/
        // lazy (see `vod_get_items_live`); eagerly crawling the whole
        // catalog would reintroduce a 20-30 minute wait. Manual `vod_sync`
        // remains available.
    }

    Ok(StalkerAddResult { playlist: result, outcome })
}

/// Edits an existing Stalker playlist: re-discovers the endpoint and
/// re-authenticates before writing anything, so a typo doesn't overwrite a
/// working playlist. Otherwise mirrors `add_stalker_playlist`, including the
/// `loginRequired` outcome (row updated either way; frontend re-prompts via
/// `StalkerLoginForm`) and non-blocking censored-category recovery.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn update_stalker_playlist(
    state: State<'_, AppState>,
    id: String,
    server_url: String,
    mac_address: String,
    title: String,
    user_agent: Option<String>,
    username: Option<String>,
    password: Option<String>,
    device_id1: Option<String>,
    device_id2: Option<String>,
    serial_number: Option<String>,
    signature1: Option<String>,
    signature2: Option<String>,
    auto_refresh: Option<bool>,
) -> CommandResult<StalkerAddResult> {
    let portal_url = server_url.trim().to_string();
    if portal_url.is_empty() {
        return Err(CommandError::Api("Portal URL is required".into()));
    }
    let mac = stalker_identity::normalize_mac(&mac_address);
    if mac.is_empty() {
        return Err(CommandError::Api("MAC address is required".into()));
    }

    let existing = db::with_conn(&state.db, {
        let id = id.clone();
        move |conn| Ok(db::playlists::get(conn, &id)?)
    })
    .await?
    .ok_or_else(|| CommandError::NotFound(format!("playlist {id} not found")))?;

    // Passes the existing stored session so an edit that only fixes, say,
    // the title doesn't force a needless fresh token - `authenticate()`'s
    // fingerprint check reuses it when everything still matches, avoiding a
    // `get_profile` call that would re-bind `device_id`.
    let discovered = stalker_auth::discover_portal_endpoint(
        &state.http,
        &StalkerCredentials {
            portal_url: &portal_url,
            mac_address: &mac,
            serial_number: serial_number.as_deref(),
            device_id: device_id1.as_deref(),
            device_id2: device_id2.as_deref(),
            signature1: signature1.as_deref(),
            signature2: signature2.as_deref(),
        },
        stalker_auth::StoredStalkerSession {
            token: existing.stalker_token.as_deref(),
            fingerprint: existing.stalker_session_identity.as_deref(),
            watchdog_timeout: existing.stalker_watchdog_timeout,
            timeslot: existing.stalker_timeslot,
        },
        username.as_deref(),
        password.as_deref(),
    )
    .await?;
    let endpoint = discovered.endpoint;
    let is_full_portal = discovered.full_portal;

    let creds = if is_full_portal {
        StalkerCredentials {
            portal_url: &endpoint,
            mac_address: &mac,
            serial_number: serial_number.as_deref(),
            device_id: device_id1.as_deref(),
            device_id2: device_id2.as_deref(),
            signature1: signature1.as_deref(),
            signature2: signature2.as_deref(),
        }
    } else {
        StalkerCredentials {
            portal_url: &endpoint,
            mac_address: &mac,
            serial_number: None,
            device_id: None,
            device_id2: None,
            signature1: None,
            signature2: None,
        }
    };
    let outcome = match discovered.outcome {
        Some(outcome) => outcome,
        None => StalkerAuthOutcome::Success {
            session: crate::types::StalkerSessionInfo {
                token: String::new(),
                endpoint: endpoint.clone(),
                full_portal: false,
                watchdog_timeout: 0,
                timeslot: 0,
                not_valid: false,
                login_completed: true,
                session_fingerprint: String::new(),
            },
        },
    };

    let mut playlist = existing;
    playlist.title = title;
    playlist.portal_url = Some(portal_url);
    playlist.stalker_endpoint = Some(endpoint.clone());
    playlist.mac_address = Some(mac.clone());
    playlist.user_agent = user_agent;
    playlist.username = username;
    playlist.password = password;
    playlist.stalker_device_id1 = device_id1.clone();
    playlist.stalker_device_id2 = device_id2.clone();
    playlist.stalker_serial_number = serial_number.clone();
    playlist.stalker_signature1 = signature1.clone();
    playlist.stalker_signature2 = signature2.clone();
    playlist.update_date = Some(Utc::now().to_rfc3339());
    if let Some(auto_refresh) = auto_refresh {
        playlist.auto_refresh = auto_refresh;
    }

    if let StalkerAuthOutcome::Success { ref session } = outcome {
        playlist.stalker_token = Some(session.token.clone());
        playlist.stalker_endpoint = Some(session.endpoint.clone());
        playlist.is_full_stalker_portal = Some(session.full_portal);
        playlist.stalker_watchdog_timeout = Some(session.watchdog_timeout);
        playlist.stalker_timeslot = Some(session.timeslot);
        playlist.stalker_not_valid = Some(session.not_valid);
        playlist.stalker_login_completed = Some(session.login_completed);
        playlist.stalker_session_identity = Some(session.session_fingerprint.clone());
    }

    let result = playlist.clone();
    db::with_conn(&state.db, move |conn| {
        db::playlists::update(conn, &playlist)?;
        Ok(())
    })
    .await?;

    if let StalkerAuthOutcome::Success { ref session } = outcome {
        let _ = sync_channels_category_aware(&state, &result.id, &creds, &session.token).await;
        // No eager VOD sync here - see the identical comment in
        // `add_stalker_playlist`.
    }

    Ok(StalkerAddResult { playlist: result, outcome })
}

#[tauri::command]
pub async fn get_playlists(state: State<'_, AppState>) -> CommandResult<Vec<Playlist>> {
    db::with_conn(&state.db, |conn| Ok(db::playlists::list(conn)?)).await
}

#[tauri::command]
pub async fn delete_playlist(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    db::with_conn(&state.db, move |conn| Ok(db::playlists::delete(conn, &id)?)).await
}

#[tauri::command]
pub async fn refresh_playlist(state: State<'_, AppState>, id: String) -> CommandResult<Playlist> {
    refresh_playlist_inner(&state, &id).await
}

/// The actual refresh logic, factored out so the daily auto-refresh
/// scheduler (no Tauri IPC context, so no `State<'_, AppState>`) can call
/// the same code path directly.
pub(crate) async fn refresh_playlist_inner(state: &AppState, id: &str) -> CommandResult<Playlist> {
    let refreshed = refresh_playlist_rows(state, id).await?;
    // Every branch below rewrites this playlist's channel rows; a channel
    // list already on screen is stale the moment that lands.
    crate::commands::stalker::notify_channels_updated(state.app.as_ref(), id);
    Ok(refreshed)
}

async fn refresh_playlist_rows(state: &AppState, id: &str) -> CommandResult<Playlist> {
    let id = id.to_string();
    let lookup_id = id.clone();
    let existing = db::with_conn(&state.db, move |conn| Ok(db::playlists::get(conn, &lookup_id)?))
        .await?
        .ok_or_else(|| CommandError::NotFound(format!("playlist {id} not found")))?;

    match existing.playlist_type {
        PlaylistType::M3u => {
            let url = existing
                .url
                .clone()
                .ok_or_else(|| CommandError::Internal("M3U playlist has no URL".into()))?;
            let content = fetch_m3u_text(&state.http, &url, existing.user_agent.as_deref()).await?;
            let parsed = m3u::parse(&content);

            let now = Utc::now().to_rfc3339();
            let mut updated = existing.clone();
            updated.count = parsed.items.len() as i64;
            updated.update_date = Some(now.clone());
            updated.last_usage = now;
            apply_detected_epg_urls(&mut updated, parsed.detected_epg_urls);

            let count = parsed.items.len() as i64;
            let items = parsed.items;
            let updated_for_db = updated.clone();
            db::with_conn(&state.db, move |conn| {
                db::playlists::update(conn, &updated_for_db)?;
                db::channels::delete_by_playlist(conn, &id)?;
                db::channels::insert_m3u_items(conn, &id, &items)?;
                db::playlists::update_count(conn, &id, count)?;
                Ok(())
            })
            .await?;

            spawn_playlist_epg_fetch(state, updated.epg_urls.as_deref());
            Ok(updated)
        }
        PlaylistType::Xtream => {
            let server_url = existing
                .server_url
                .clone()
                .ok_or_else(|| CommandError::Internal("Xtream playlist has no server URL".into()))?;
            let username = existing
                .username
                .clone()
                .ok_or_else(|| CommandError::Internal("Xtream playlist has no username".into()))?;
            let password = existing
                .password
                .clone()
                .ok_or_else(|| CommandError::Internal("Xtream playlist has no password".into()))?;

            let user_info = xtream::get_account_info(&state.http, &server_url, &username, &password).await?;
            let format = xtream::preferred_live_format(user_info.allowed_output_formats.as_deref());
            let channels =
                fetch_xtream_live_channels(&state.http, &server_url, &username, &password, &format).await?;

            let now = Utc::now().to_rfc3339();
            let mut updated = existing.clone();
            updated.count = channels.len() as i64;
            updated.update_date = Some(now.clone());
            updated.last_usage = now;
            apply_xtream_default_headers(&mut updated, &server_url);
            updated.server_timezone = user_info.server_info.as_ref().and_then(|s| s.timezone.clone());

            let count = channels.len() as i64;
            let updated_for_db = updated.clone();
            db::with_conn(&state.db, move |conn| {
                db::playlists::update(conn, &updated_for_db)?;
                db::channels::delete_by_playlist(conn, &id)?;
                db::channels::insert_channels(conn, &id, &channels)?;
                db::playlists::update_count(conn, &id, count)?;
                Ok(())
            })
            .await?;

            spawn_vod_sync(state, updated.clone());
            Ok(updated)
        }
        PlaylistType::Stalker => {
            let portal_url = existing
                .stalker_endpoint
                .clone()
                .or_else(|| existing.portal_url.clone())
                .ok_or_else(|| CommandError::Internal("Stalker playlist has no portal URL".into()))?;
            let mac = existing
                .mac_address
                .clone()
                .ok_or_else(|| CommandError::Internal("Stalker playlist has no MAC address".into()))?;
            let is_full_portal = existing.is_full_stalker_portal();
            let creds = if is_full_portal {
                StalkerCredentials {
                    portal_url: &portal_url,
                    mac_address: &mac,
                    serial_number: existing.stalker_serial_number.as_deref(),
                    device_id: existing.stalker_device_id1.as_deref(),
                    device_id2: existing.stalker_device_id2.as_deref(),
                    signature1: existing.stalker_signature1.as_deref(),
                    signature2: existing.stalker_signature2.as_deref(),
                }
            } else {
                StalkerCredentials {
                    portal_url: &portal_url,
                    mac_address: &mac,
                    serial_number: None,
                    device_id: None,
                    device_id2: None,
                    signature1: None,
                    signature2: None,
                }
            };

            let session = if is_full_portal {
                let stored = stalker_auth::StoredStalkerSession {
                    token: existing.stalker_token.as_deref(),
                    fingerprint: existing.stalker_session_identity.as_deref(),
                    watchdog_timeout: existing.stalker_watchdog_timeout,
                    timeslot: existing.stalker_timeslot,
                };
                let outcome = stalker_auth::authenticate(
                    &state.http,
                    &creds,
                    stored,
                    existing.username.as_deref(),
                    existing.password.as_deref(),
                )
                .await?;
                let StalkerAuthOutcome::Success { session } = outcome else {
                    return Err(CommandError::Auth(
                        "Couldn't refresh this Stalker playlist — re-authentication is required.".into(),
                    ));
                };
                session
            } else {
                crate::types::StalkerSessionInfo {
                    token: String::new(),
                    endpoint: portal_url.clone(),
                    full_portal: false,
                    watchdog_timeout: 0,
                    timeslot: 0,
                    not_valid: false,
                    login_completed: true,
                    session_fingerprint: String::new(),
                }
            };

            // Persisted separately from the channel sync below -
            // `sync_channels_category_aware` doesn't own auth session columns.
            let id_for_session = id.clone();
            let session_for_db = session.clone();
            db::with_conn(&state.db, move |conn| Ok(db::playlists::update_stalker_session(conn, &id_for_session, &session_for_db)?))
                .await?;

            // Bulk channels + a background recovery crawl for any missing,
            // non-fresh categories - never blocks this refresh itself.
            let bulk_channels = sync_channels_category_aware(state, &id, &creds, &session.token).await?;

            let now = Utc::now().to_rfc3339();
            let mut updated = existing.clone();
            // Reflects only the bulk portion for now - the still-running
            // background recovery crawl updates `playlists.count` again
            // itself once it commits.
            updated.count = bulk_channels.len() as i64;
            updated.update_date = Some(now.clone());
            updated.last_usage = now;
            updated.stalker_token = Some(session.token.clone());
            updated.stalker_watchdog_timeout = Some(session.watchdog_timeout);
            updated.stalker_timeslot = Some(session.timeslot);
            // No eager VOD sync here - see the identical comment in
            // `add_stalker_playlist`.

            Ok(updated)
        }
    }
}
