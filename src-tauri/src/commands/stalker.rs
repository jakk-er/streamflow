use crate::db;
use crate::error::{CommandError, CommandResult};
use crate::net::stalker::auth::StalkerCredentials;
use crate::net::stalker::{auth as stalker_auth, content as stalker_content};
use crate::state::AppState;
use crate::types::{
    Channel, Playlist, SeriesDetails, StalkerAuthOutcome, StalkerCategory, StalkerContentItem,
    StalkerContentPage, StalkerContentType, VodContentType, VodDetails,
};
use tauri::State;

pub(crate) async fn load_playlist(state: &AppState, playlist_id: &str) -> CommandResult<Playlist> {
    let id = playlist_id.to_string();
    db::with_conn(&state.db, move |conn| Ok(db::playlists::get(conn, &id)?))
        .await?
        .ok_or_else(|| CommandError::NotFound(format!("playlist {playlist_id} not found")))
}

/// A **full** portal's identity includes serial/device-id/signature; a
/// **simple** (token-free) portal's doesn't. Stripping them here, the one
/// place `StalkerCredentials` gets built, is what makes
/// `identity::build_api_headers` omit the `SN` header/`__cfduid` cookie for
/// simple portals without a separate flag threaded everywhere.
pub(crate) fn build_creds(playlist: &Playlist) -> CommandResult<StalkerCredentials<'_>> {
    let portal_url = playlist
        .stalker_endpoint
        .as_deref()
        .or(playlist.portal_url.as_deref())
        .ok_or_else(|| CommandError::Internal("Playlist has no Stalker portal URL".into()))?;
    let mac_address = playlist
        .mac_address
        .as_deref()
        .ok_or_else(|| CommandError::Internal("Playlist has no MAC address".into()))?;
    if playlist.is_full_stalker_portal() {
        Ok(StalkerCredentials {
            portal_url,
            mac_address,
            serial_number: playlist.stalker_serial_number.as_deref(),
            device_id: playlist.stalker_device_id1.as_deref(),
            device_id2: playlist.stalker_device_id2.as_deref(),
            signature1: playlist.stalker_signature1.as_deref(),
            signature2: playlist.stalker_signature2.as_deref(),
        })
    } else {
        Ok(StalkerCredentials {
            portal_url,
            mac_address,
            serial_number: None,
            device_id: None,
            device_id2: None,
            signature1: None,
            signature2: None,
        })
    }
}

fn stored_session(playlist: &Playlist) -> stalker_auth::StoredStalkerSession<'_> {
    stalker_auth::StoredStalkerSession {
        token: playlist.stalker_token.as_deref(),
        fingerprint: playlist.stalker_session_identity.as_deref(),
        watchdog_timeout: playlist.stalker_watchdog_timeout,
        timeslot: playlist.stalker_timeslot,
    }
}

/// Re-authenticates from scratch and persists the resulting session - the
/// recovery half of the content commands' retry-once-on-auth-failure
/// behavior. A simple (token-free) portal has no token to expire, so an
/// `Auth` error there just propagates rather than attempting a full-portal
/// handshake it was never supposed to need.
pub(crate) async fn reauthenticate(state: &AppState, playlist_id: &str, playlist: &Playlist) -> CommandResult<String> {
    if !playlist.is_full_stalker_portal() {
        return Err(CommandError::Auth(
            "This Stalker portal rejected the request and doesn't use session tokens — try refreshing the playlist.".into(),
        ));
    }
    let creds = build_creds(playlist)?;
    let outcome = stalker_auth::authenticate(
        &state.http,
        &creds,
        stored_session(playlist),
        playlist.username.as_deref(),
        playlist.password.as_deref(),
    )
    .await?;
    persist_outcome(state, playlist_id, &outcome, None, None).await?;
    match outcome {
        StalkerAuthOutcome::Success { session } => Ok(session.token),
        _ => Err(CommandError::Auth(
            "The Stalker portal session expired and couldn't be renewed automatically — try logging in again.".into(),
        )),
    }
}

/// A simple (token-free) portal has no token - an empty string flows through
/// `identity::build_api_headers` as "no token" (it filters empty values
/// before adding `Authorization`), so calls work unchanged without an
/// `Option<&str>` threaded through `content.rs`.
pub(crate) fn require_token(playlist: &Playlist) -> CommandResult<String> {
    if !playlist.is_full_stalker_portal() {
        return Ok(String::new());
    }
    playlist
        .stalker_token
        .clone()
        .ok_or_else(|| CommandError::Auth("Not authenticated with this Stalker portal yet.".into()))
}

async fn persist_outcome(
    state: &AppState,
    playlist_id: &str,
    outcome: &StalkerAuthOutcome,
    username: Option<&str>,
    password: Option<&str>,
) -> CommandResult<()> {
    let StalkerAuthOutcome::Success { session } = outcome else {
        return Ok(());
    };
    let session = session.clone();
    let id = playlist_id.to_string();
    let username = username.map(str::to_string);
    let password = password.map(str::to_string);
    db::with_conn(&state.db, move |conn| {
        db::playlists::update_stalker_session(conn, &id, &session)?;
        if let (Some(u), Some(p)) = (&username, &password) {
            db::playlists::update_stalker_credentials(conn, &id, u, p)?;
        }
        Ok(())
    })
    .await
}

/// How long a successfully-recovered ITV category is trusted before it's
/// worth re-checking - matches `scheduler::is_due`'s daily auto-refresh
/// cadence rather than an arbitrary number. Older (or never synced) is
/// treated as needing a fresh crawl.
const RECOVERY_REVALIDATION_HOURS: i64 = 24;

fn is_recovery_state_fresh(state: &db::itv_recovery::CategoryState, now: chrono::DateTime<chrono::Utc>) -> bool {
    if state.status != db::itv_recovery::RecoveryStatus::Synced {
        return false;
    }
    let Some(last_synced_at) = &state.last_synced_at else { return false };
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(last_synced_at) else { return false };
    now.signed_duration_since(parsed) < chrono::Duration::hours(RECOVERY_REVALIDATION_HOURS)
}

/// Category-aware replacement for "bulk fetch, wipe the WHOLE playlist,
/// insert, then unconditionally recover every missing category": the wipe
/// preserves any recovery category still fresh (see
/// `RECOVERY_REVALIDATION_HOURS`), and recovery only runs for categories
/// neither in the bulk results nor still fresh. On a normal refresh with
/// nothing censored changed, this skips the ~30s recovery crawl entirely.
/// Called from every full ITV sync entry point.
pub(crate) async fn sync_channels_category_aware(
    state: &AppState,
    playlist_id: &str,
    creds: &StalkerCredentials<'_>,
    token: &str,
) -> CommandResult<Vec<Channel>> {
    let (channels, bulk_category_ids) = stalker_content::get_all_channels(&state.http, creds, token).await?;

    let id = playlist_id.to_string();
    let recovery_state = db::with_conn(&state.db, move |conn| Ok(db::itv_recovery::list_state(conn, &id)?)).await?;
    let now = chrono::Utc::now();
    let fresh_ids: std::collections::HashSet<String> = recovery_state
        .iter()
        .filter(|(_, s)| is_recovery_state_fresh(s, now))
        .map(|(id, _)| id.clone())
        .collect();
    if !fresh_ids.is_empty() {
        // `info!`, not `error!` - a normal, successful outcome, filtered out
        // under this app's errors-only subscriber (`lib.rs`). Kept as the
        // one place that would make "how much did this sync skip
        // re-crawling" observable if the log level is ever raised.
        let total_preserved_channels: i64 = fresh_ids.iter().filter_map(|id| recovery_state.get(id)).map(|s| s.channel_count).sum();
        let oldest_attempt = fresh_ids.iter().filter_map(|id| recovery_state.get(id)).map(|s| s.last_attempt_at.as_str()).min();
        tracing::info!(
            "Stalker channel sync for playlist {playlist_id}: skipping recovery re-crawl for {} categor{} still fresh (< {RECOVERY_REVALIDATION_HOURS}h old), preserving {total_preserved_channels} previously recovered channels (oldest last attempted: {})",
            fresh_ids.len(),
            if fresh_ids.len() == 1 { "y" } else { "ies" },
            oldest_attempt.unwrap_or("unknown"),
        );
    }

    let id = playlist_id.to_string();
    let channels_for_db = channels.clone();
    let preserved: Vec<String> = fresh_ids.iter().cloned().collect();
    db::with_conn(&state.db, move |conn| {
        db::channels::delete_by_playlist_except_categories(conn, &id, &preserved)?;
        db::channels::insert_channels(conn, &id, &channels_for_db)?;
        // A fresh COUNT, not `channels.len()` - the delete preserved fresh
        // recovery categories, so the true total is bulk + preserved.
        let total = db::channels::count_by_playlist(conn, &id)?;
        db::playlists::update_count(conn, &id, total)?;
        Ok(())
    })
    .await?;

    // "Already covered" = the bulk sync's results plus whatever's still
    // fresh from a previous recovery - so `find_missing_itv_categories`
    // reports exactly what needs a crawl now.
    let already_covered: std::collections::HashSet<String> = bulk_category_ids.union(&fresh_ids).cloned().collect();
    spawn_censored_itv_recovery(state, playlist_id, creds, token, already_covered);

    Ok(channels)
}

/// Recovering adult/"censored" ITV genres a portal excludes from the fast
/// sync is slow (a separate paginated crawl per missing genre), so it always
/// runs detached, never blocking the triggering command - a prior revision
/// ran this inline and blocked `add_stalker_playlist` for minutes on a
/// portal with many small genres.
///
/// `category_ids` is "every category id already covered" (bulk results
/// unioned with anything still fresh from a previous recovery), passed in
/// rather than reloaded from the DB - this also lets presence tracking work
/// by the portal's own category id instead of `Channel.group.title`.
/// `creds`/`token` are cloned to owned data since the spawned task must
/// outlive the caller's borrows.
///
/// Three properties this task must keep (each was a real bug once):
/// 1. Each category commits the moment it finishes, not batched at the end -
///    quitting mid-run used to discard everything fetched so far.
/// 2. A `channels-updated` event follows each commit, or new rows stay
///    invisible until something tells the frontend to re-read.
/// 3. Only one run per playlist at a time (`recovery_inflight`) - the
///    portal tolerates roughly one connection at a time.
pub(crate) fn spawn_censored_itv_recovery(
    state: &AppState,
    playlist_id: &str,
    creds: &StalkerCredentials<'_>,
    token: &str,
    category_ids: std::collections::HashSet<String>,
) {
    let inflight = state.recovery_inflight.clone();
    {
        let mut guard = inflight.lock().unwrap();
        if !guard.insert(format!("itv:{playlist_id}")) {
            return;
        }
    }

    let http = state.http.clone();
    let db = state.db.clone();
    let app = state.app.clone();
    let portal_url = creds.portal_url.to_string();
    let mac_address = creds.mac_address.to_string();
    let serial_number = creds.serial_number.map(str::to_string);
    let device_id = creds.device_id.map(str::to_string);
    let device_id2 = creds.device_id2.map(str::to_string);
    let signature1 = creds.signature1.map(str::to_string);
    let signature2 = creds.signature2.map(str::to_string);
    let token = token.to_string();
    let playlist_id = playlist_id.to_string();

    tauri::async_runtime::spawn(async move {
        let creds = StalkerCredentials {
            portal_url: &portal_url,
            mac_address: &mac_address,
            serial_number: serial_number.as_deref(),
            device_id: device_id.as_deref(),
            device_id2: device_id2.as_deref(),
            signature1: signature1.as_deref(),
            signature2: signature2.as_deref(),
        };

        run_censored_itv_recovery(&http, &db, app.as_ref(), &creds, &token, &playlist_id, category_ids).await;

        inflight.lock().unwrap().remove(&format!("itv:{playlist_id}"));
    });
}

/// The body of `spawn_censored_itv_recovery`'s detached task, split out only
/// so the in-flight guard is released on every exit path without threading an
/// early `return` through it.
async fn run_censored_itv_recovery(
    http: &reqwest::Client,
    db: &db::DbPool,
    app: Option<&tauri::AppHandle>,
    creds: &StalkerCredentials<'_>,
    token: &str,
    playlist_id: &str,
    category_ids: std::collections::HashSet<String>,
) {
    let missing = match stalker_content::find_missing_itv_categories(http, creds, token, &category_ids).await {
        Ok(missing) => missing,
        Err(e) => {
            // `error!`, not `warn!` - deliberately: this represents real
            // content the user cannot see, not a routine, absorbed hiccup.
            // Kept visible under an errors-only log filter for that reason.
            tracing::error!("Censored ITV category recovery failed for playlist {playlist_id}: {e}");
            return;
        }
    };
    if missing.missing_ids.is_empty() {
        return;
    }

    let name_of = |id: &String| missing.category_names.get(id).cloned().unwrap_or_else(|| id.clone());
    // `info!`, not `error!` - attempting a recovery crawl isn't itself a
    // problem; filtered out under this app's errors-only subscriber.
    tracing::info!(
        "Censored ITV category recovery: attempting {} categor{} missing from the fast sync: {:?}",
        missing.missing_ids.len(),
        if missing.missing_ids.len() == 1 { "y" } else { "ies" },
        missing.missing_ids.iter().map(name_of).collect::<Vec<_>>()
    );

    let mut total_recovered = 0usize;

    for category_id in &missing.missing_ids {
        let name = name_of(category_id);

        // Set the instant the crawl starts - see `mark_in_progress`'s doc
        // comment for why a crash mid-crawl needs no separate cleanup.
        {
            let pid = playlist_id.to_string();
            let cid = category_id.clone();
            let at = chrono::Utc::now().to_rfc3339();
            let _ = db::with_conn(db, move |conn| Ok(db::itv_recovery::mark_in_progress(conn, &pid, &cid, &at)?)).await;
        }

        let channels = match stalker_content::crawl_itv_category(http, creds, token, category_id, &missing.category_names).await {
            Ok(channels) => channels,
            Err(e) => {
                // Logged per-category rather than aborting the whole run -
                // one flaky category must not take every other one down.
                tracing::error!("Censored ITV category recovery: failed to fetch category '{name}' ({category_id}): {e}");
                // Failed fetch never reaches `replace_category_channels` -
                // whatever was already stored is left untouched.
                let pid = playlist_id.to_string();
                let cid = category_id.clone();
                let at = chrono::Utc::now().to_rfc3339();
                let _ = db::with_conn(db, move |conn| Ok(db::itv_recovery::mark_failed(conn, &pid, &cid, &at)?)).await;
                continue;
            }
        };

        // An empty `channels` is a valid result (category genuinely has zero
        // items) and still replaces whatever was there; only a fetch
        // failure above skips the replace.
        let count = channels.len();
        let pid = playlist_id.to_string();
        let cid = category_id.clone();
        let synced_at = chrono::Utc::now().to_rfc3339();
        let result = db::with_conn(db, move |conn| {
            db::channels::replace_category_channels(conn, &pid, &cid, &channels)?;
            let total = db::channels::count_by_playlist(conn, &pid)?;
            db::playlists::update_count(conn, &pid, total)?;
            db::itv_recovery::mark_synced(conn, &pid, &cid, &synced_at, count as i64)?;
            Ok(())
        })
        .await;
        match result {
            Ok(()) => {
                total_recovered += count;
                notify_channels_updated(app, playlist_id);
            }
            Err(e) => {
                tracing::error!("Censored ITV category recovery DB write failed for playlist {playlist_id} ('{name}'): {e}");
                let pid = playlist_id.to_string();
                let cid = category_id.clone();
                let at = chrono::Utc::now().to_rfc3339();
                let _ = db::with_conn(db, move |conn| Ok(db::itv_recovery::mark_failed(conn, &pid, &cid, &at)?)).await;
            }
        }
    }

    if total_recovered == 0 {
        // `warn!`, not `error!` - no transport/portal failure, just zero
        // channels across every missing category (genres may genuinely be
        // empty, or the portal quietly excludes this content per-category
        // too). Not a confirmed failure, so kept below `error!`.
        tracing::warn!("Censored ITV category recovery for playlist {playlist_id}: ran but found 0 channels across every category missing from the fast sync - portal may exclude this content from the per-category endpoint too");
    }
}

/// Tells the frontend that `playlist_id`'s channel rows changed underneath
/// it - `channelStore` re-reads when this names the playlist it has loaded,
/// the only way rows written after render become visible without a restart.
pub(crate) fn notify_channels_updated(app: Option<&tauri::AppHandle>, playlist_id: &str) {
    let Some(app) = app else { return };
    // Emit failure means no webview is listening (shutting down, or a
    // detached scheduler run with no window) - nothing to recover from.
    let _ = tauri::Emitter::emit(app, "channels-updated", playlist_id);
}

#[tauri::command]
pub async fn stalker_auth(
    state: State<'_, AppState>,
    playlist_id: String,
    username: Option<String>,
    password: Option<String>,
) -> CommandResult<StalkerAuthOutcome> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    let creds = build_creds(&playlist)?;
    let outcome = stalker_auth::authenticate(&state.http, &creds, stored_session(&playlist), username.as_deref(), password.as_deref()).await?;
    persist_outcome(&state, &playlist_id, &outcome, username.as_deref(), password.as_deref()).await?;
    Ok(outcome)
}

#[tauri::command]
pub async fn stalker_do_auth(
    state: State<'_, AppState>,
    playlist_id: String,
    username: String,
    password: String,
) -> CommandResult<StalkerAuthOutcome> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    let creds = build_creds(&playlist)?;
    let outcome = stalker_auth::authenticate(&state.http, &creds, stored_session(&playlist), Some(&username), Some(&password)).await?;
    persist_outcome(&state, &playlist_id, &outcome, Some(&username), Some(&password)).await?;
    Ok(outcome)
}

#[tauri::command]
pub async fn stalker_refresh_token(state: State<'_, AppState>, playlist_id: String) -> CommandResult<StalkerAuthOutcome> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    let creds = build_creds(&playlist)?;
    let outcome = stalker_auth::authenticate(
        &state.http,
        &creds,
        stored_session(&playlist),
        playlist.username.as_deref(),
        playlist.password.as_deref(),
    )
    .await?;
    persist_outcome(&state, &playlist_id, &outcome, None, None).await?;
    Ok(outcome)
}

#[tauri::command]
pub async fn stalker_watchdog_ping(state: State<'_, AppState>, playlist_id: String, init: Option<bool>) -> CommandResult<()> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    let creds = build_creds(&playlist)?;
    let token = require_token(&playlist)?;
    stalker_auth::watchdog_ping(&state.http, &creds, &token, init.unwrap_or(false)).await
}

#[tauri::command]
pub async fn stalker_get_categories(
    state: State<'_, AppState>,
    playlist_id: String,
    content_type: StalkerContentType,
) -> CommandResult<Vec<StalkerCategory>> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    let creds = build_creds(&playlist)?;
    let token = require_token(&playlist)?;
    match stalker_content::get_categories(&state.http, &creds, &token, content_type).await {
        Err(CommandError::Auth(_)) => {
            // Session/token can be invalidated out from under us anytime
            // (e.g. another device on the shared MAC re-authenticated) -
            // retry once with stored credentials before giving up.
            let new_token = reauthenticate(&state, &playlist_id, &playlist).await?;
            stalker_content::get_categories(&state.http, &creds, &new_token, content_type).await
        }
        other => other,
    }
}

#[tauri::command]
pub async fn stalker_get_content(
    state: State<'_, AppState>,
    playlist_id: String,
    content_type: StalkerContentType,
    category_id: Option<String>,
    page: i64,
) -> CommandResult<StalkerContentPage> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    let creds = build_creds(&playlist)?;
    let token = require_token(&playlist)?;
    match stalker_content::get_content(&state.http, &creds, &token, content_type, category_id.as_deref(), page, None).await {
        Err(CommandError::Auth(_)) => {
            let new_token = reauthenticate(&state, &playlist_id, &playlist).await?;
            stalker_content::get_content(&state.http, &creds, &new_token, content_type, category_id.as_deref(), page, None).await
        }
        other => other,
    }
}

#[tauri::command]
#[allow(unused_variables)]
pub async fn stalker_get_vod_info(
    state: State<'_, AppState>,
    playlist_id: String,
    content_type: StalkerContentType,
    item: StalkerContentItem,
) -> CommandResult<VodDetails> {
    // Stalker has no single-item lookup endpoint - the catalog row already
    // IS the detail, so this needs no portal round trip at all.
    Ok(stalker_content::item_to_vod_details(&item, content_type))
}

/// Cache-aside against `vod_items.detail_json` (see `xtream_get_series_info`)
/// - `get_series_details` is slow per-series network work. Always keyed
/// under `VodContentType::Series` regardless of the row's own Stalker
/// `content_type`, matching how `vod_sync_playlist` stores series rows.
#[tauri::command]
pub async fn stalker_get_series_info(
    state: State<'_, AppState>,
    playlist_id: String,
    content_type: StalkerContentType,
    item: StalkerContentItem,
) -> CommandResult<SeriesDetails> {
    let id = playlist_id.clone();
    let item_id = item.id.clone();
    if let Some(cached) = db::with_conn(&state.db, move |conn| Ok(db::vod::get_detail_json(conn, &id, VodContentType::Series, &item_id)?)).await? {
        if let Ok(detail) = serde_json::from_str::<SeriesDetails>(&cached) {
            return Ok(detail);
        }
    }

    let playlist = load_playlist(&state, &playlist_id).await?;
    let creds = build_creds(&playlist)?;
    let token = require_token(&playlist)?;
    let detail = match stalker_content::get_series_details(&state.http, &creds, &token, content_type, &item).await {
        Err(CommandError::Auth(_)) => {
            let new_token = reauthenticate(&state, &playlist_id, &playlist).await?;
            stalker_content::get_series_details(&state.http, &creds, &new_token, content_type, &item).await?
        }
        other => other?,
    };

    if let Ok(json) = serde_json::to_string(&detail) {
        let id = playlist_id.clone();
        let item_id = item.id.clone();
        let _ = db::with_conn(&state.db, move |conn| Ok(db::vod::set_detail_json(conn, &id, VodContentType::Series, &item_id, &json)?)).await;
    }
    Ok(detail)
}

#[tauri::command]
pub async fn stalker_resolve_playback(
    state: State<'_, AppState>,
    playlist_id: String,
    content_type: StalkerContentType,
    cmd: String,
    use_http_tmp_link: Option<String>,
    use_load_balancing: Option<String>,
    series: Option<String>,
) -> CommandResult<String> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    let creds = build_creds(&playlist)?;
    let token = require_token(&playlist)?;
    let result = stalker_content::resolve_playback(
        &state.http,
        &creds,
        &token,
        content_type,
        &cmd,
        use_http_tmp_link.as_deref(),
        use_load_balancing.as_deref(),
        series.as_deref(),
    )
    .await;
    match result {
        Err(CommandError::Auth(_)) => {
            let new_token = reauthenticate(&state, &playlist_id, &playlist).await?;
            stalker_content::resolve_playback(
                &state.http,
                &creds,
                &new_token,
                content_type,
                &cmd,
                use_http_tmp_link.as_deref(),
                use_load_balancing.as_deref(),
                series.as_deref(),
            )
            .await
        }
        other => other,
    }
}

#[tauri::command]
pub async fn stalker_resolve_vod_episode(
    state: State<'_, AppState>,
    playlist_id: String,
    content_type: StalkerContentType,
    cmd: String,
    series: Option<String>,
) -> CommandResult<String> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    let creds = build_creds(&playlist)?;
    let token = require_token(&playlist)?;
    match stalker_content::resolve_vod_episode(&state.http, &creds, &token, content_type, &cmd, series.as_deref()).await {
        Err(CommandError::Auth(_)) => {
            let new_token = reauthenticate(&state, &playlist_id, &playlist).await?;
            stalker_content::resolve_vod_episode(&state.http, &creds, &new_token, content_type, &cmd, series.as_deref()).await
        }
        other => other,
    }
}

#[tauri::command]
pub async fn stalker_stream_headers(state: State<'_, AppState>, playlist_id: String) -> CommandResult<Vec<(String, String)>> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    let creds = build_creds(&playlist)?;
    let token = require_token(&playlist)?;
    Ok(stalker_content::get_stream_headers(&state.http, &creds, &token).await)
}

/// Fetches AND persists the full ITV + radio channel list. The frontend
/// (`stalker-session.svelte.ts`'s `afterSuccess`, after every auth/reconnect)
/// discards the return value and calls it purely for the DB side effect -
/// previously this fetched but never wrote to the DB, so a reconnect
/// silently left a stale channel list.
#[tauri::command]
pub async fn stalker_get_channels(state: State<'_, AppState>, playlist_id: String) -> CommandResult<Vec<Channel>> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    let creds = build_creds(&playlist)?;
    let token = require_token(&playlist)?;
    match sync_channels_category_aware(&state, &playlist_id, &creds, &token).await {
        Err(CommandError::Auth(_)) => {
            let new_token = reauthenticate(&state, &playlist_id, &playlist).await?;
            sync_channels_category_aware(&state, &playlist_id, &creds, &new_token).await
        }
        other => other,
    }
}

/// Fetches the playlist's bulk 7-day EPG in ONE request, correlates entries
/// back to stored channels via the portal's numeric channel id (stashed in
/// `channel.raw`), and stores them through the same `epg_channels`/
/// `epg_programs` pipeline XMLTV imports use, scoped to its own `source_url`.
/// Channels with no recoverable id (e.g. old rows) are silently skipped.
#[tauri::command]
pub async fn stalker_sync_epg(state: State<'_, AppState>, playlist_id: String) -> CommandResult<()> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    let creds = build_creds(&playlist)?;
    let token = require_token(&playlist)?;

    let bulk = stalker_content::get_epg_info(&state.http, &creds, &token, 168).await?;
    if bulk.is_empty() {
        return Ok(());
    }

    let id = playlist_id.clone();
    let channels = db::with_conn(&state.db, move |conn| Ok(db::channels::list_by_playlist(conn, &id)?)).await?;

    let mut epg_channels = Vec::new();
    let mut programs = Vec::new();
    for channel in &channels {
        let stalker_id = channel
            .raw
            .as_deref()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
            .and_then(|v| v.get("stalker_channel_id").and_then(|s| s.as_str().map(str::to_string)));
        let Some(stalker_id) = stalker_id else { continue };
        let Some(channel_programs) = bulk.get(&stalker_id) else { continue };

        let channel_key = channel
            .tvg
            .id
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("stalker-epg-chan:{}", channel.id));
        epg_channels.push(crate::parsers::xmltv::ParsedEpgChannel {
            id: channel_key.clone(),
            display_name: channel.name.clone(),
            icon_url: None,
        });
        for p in channel_programs {
            programs.push(crate::parsers::xmltv::ParsedEpgProgram { channel_id: channel_key.clone(), ..p.clone() });
        }
    }
    if epg_channels.is_empty() {
        return Ok(());
    }

    let source_url = format!("stalker-epg:{playlist_id}");
    let parsed = crate::parsers::xmltv::ParsedEpg { channels: epg_channels, programs };
    db::with_conn(&state.db, move |conn| Ok(db::epg::store(conn, &source_url, &parsed)?)).await
}

/// A quick "now/next" peek at one ITV channel's upcoming programs, fetched
/// fresh and not persisted. `stalker_channel_id` is the portal's own numeric
/// id (recoverable from a stored channel's `raw` JSON).
#[tauri::command]
pub async fn stalker_get_short_epg(
    state: State<'_, AppState>,
    playlist_id: String,
    stalker_channel_id: String,
    size: Option<i64>,
) -> CommandResult<Vec<crate::types::EpgProgram>> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    let creds = build_creds(&playlist)?;
    let token = require_token(&playlist)?;
    let programs = stalker_content::get_short_epg(&state.http, &creds, &token, &stalker_channel_id, size.unwrap_or(10), "").await?;
    Ok(programs
        .into_iter()
        .map(|p| crate::types::EpgProgram {
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

/// Derives `(deviceId1, deviceId2)` from a MAC address (StbEmu-compatible
/// SHA-256) for the "Advanced device identity" form's opt-in prefill only -
/// never invoked automatically at auth/request time, so a portal is never
/// silently probed with a guessed device id.
#[tauri::command]
pub fn stalker_derive_device_ids(mac_address: String) -> Option<(String, String)> {
    crate::net::stalker::identity::derive_device_ids(&mac_address)
}
