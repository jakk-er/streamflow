use crate::commands::stalker as stalker_cmds;
use crate::db;
use crate::error::{CommandError, CommandResult};
use crate::net::stalker::content as stalker_content;
use crate::net::stalker::auth::StalkerCredentials;
use crate::net::xtream;
use crate::state::AppState;
use crate::types::{Playlist, PlaylistType, StalkerContentItem, StalkerContentType, VodCatalogItem, VodContentType, VodLivePage, XtreamCategory, XtreamStream};
use std::collections::HashSet;
use tauri::State;

async fn load_playlist(state: &AppState, playlist_id: &str) -> CommandResult<Playlist> {
    let id = playlist_id.to_string();
    db::with_conn(&state.db, move |conn| Ok(db::playlists::get(conn, &id)?))
        .await?
        .ok_or_else(|| CommandError::NotFound(format!("playlist {playlist_id} not found")))
}

fn xtream_stream_type(content_type: VodContentType) -> &'static str {
    match content_type {
        VodContentType::Movie => "vod",
        VodContentType::Series => "series",
    }
}

fn stalker_content_type_of(content_type: VodContentType) -> StalkerContentType {
    match content_type {
        VodContentType::Movie => StalkerContentType::Vod,
        VodContentType::Series => StalkerContentType::Series,
    }
}

/// Xtream: reads the local cache, syncing once first if never synced -
/// Xtream's bulk sync is fast and complete, so eager mirroring is fine.
///
/// Stalker: browsing is remote-first/lazy instead (see `vod_get_items_live`)
/// - a portal's "get everything" endpoint often covers only a fraction of
/// categories, so eagerly mirroring the whole catalog meant a 20-30 minute
/// wait before anything was usable. Delegates to a live per-category fetch
/// instead; `ensure_synced`'s eager sync never triggers for Stalker.
#[tauri::command]
pub async fn vod_get_categories(state: State<'_, AppState>, playlist_id: String, content_type: VodContentType) -> CommandResult<Vec<XtreamCategory>> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    if playlist.playlist_type == PlaylistType::Stalker {
        return stalker_categories_live(&state, &playlist, content_type).await;
    }
    ensure_synced(&state, &playlist_id, content_type).await?;
    let id = playlist_id.clone();
    db::with_conn(&state.db, move |conn| Ok(db::vod::get_categories(conn, &id, content_type)?)).await
}

async fn stalker_categories_live(state: &AppState, playlist: &Playlist, content_type: VodContentType) -> CommandResult<Vec<XtreamCategory>> {
    let stalker_type = stalker_content_type_of(content_type);
    match stalker_categories_live_once(state, playlist, stalker_type, content_type).await {
        Err(CommandError::Auth(_)) => {
            stalker_cmds::reauthenticate(state, &playlist.id, playlist).await?;
            let refreshed = load_playlist(state, &playlist.id).await?;
            stalker_categories_live_once(state, &refreshed, stalker_type, content_type).await
        }
        other => other,
    }
}

/// One small, unpaginated portal call - plain delete-then-insert replace
/// every call (cheap, tens to a few hundred rows), returning what was
/// fetched directly instead of round-tripping through a DB read. The cache
/// write is best-effort - the portal's live list is the source of truth.
async fn stalker_categories_live_once(
    state: &AppState,
    playlist: &Playlist,
    stalker_type: StalkerContentType,
    content_type: VodContentType,
) -> CommandResult<Vec<XtreamCategory>> {
    let creds = stalker_cmds::build_creds(playlist)?;
    let token = stalker_cmds::require_token(playlist)?;
    let categories = stalker_content::get_categories(&state.http, &creds, &token, stalker_type).await?;

    let category_pairs: Vec<(String, String)> = categories.iter().map(|c| (c.id.clone(), c.title.clone())).collect();
    let playlist_id = playlist.id.clone();
    let pairs_for_db = category_pairs.clone();
    if let Err(e) = db::with_conn(&state.db, move |conn| {
        db::vod::delete_categories(conn, &playlist_id, content_type)?;
        db::vod::insert_categories(conn, &playlist_id, content_type, &pairs_for_db)?;
        Ok(())
    })
    .await
    {
        tracing::warn!("Failed to cache Stalker category list for playlist '{}': {e}", playlist.id);
    }

    Ok(category_pairs
        .into_iter()
        .map(|(category_id, category_name)| XtreamCategory { id: None, category_id, category_name, parent_id: 0, count: None })
        .collect())
}

/// Xtream: pure cache read, syncing once first if never synced.
///
/// Stalker: pure cache read, full stop - never triggers a network call. With
/// no category selected this is a view over whatever's cached so far (grows
/// only as a side effect of `vod_get_items_live` being called elsewhere), not
/// a claim of completeness. A specific category should go through
/// `vod_get_items_live` instead.
#[tauri::command]
pub async fn vod_get_items(
    state: State<'_, AppState>,
    playlist_id: String,
    content_type: VodContentType,
    category_id: Option<String>,
) -> CommandResult<Vec<VodCatalogItem>> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    if playlist.playlist_type != PlaylistType::Stalker {
        ensure_synced(&state, &playlist_id, content_type).await?;
    }
    let id = playlist_id.clone();
    db::with_conn(&state.db, move |conn| {
        Ok(db::vod::get_items(conn, &id, content_type, category_id.as_deref())?)
    })
    .await
}

/// Pure local-cache read by provider item id, never a network call - the
/// fallback the frontend's `vodStore.loadDetail` reaches for when its
/// in-memory Stalker item cache is empty (e.g. right after a restart).
/// `Ok(None)` means never cached; the caller's own "open it from the list"
/// error is correct there, since there's no id-based portal lookup to retry.
#[tauri::command]
pub async fn vod_get_cached_item(
    state: State<'_, AppState>,
    playlist_id: String,
    content_type: VodContentType,
    item_id: String,
) -> CommandResult<Option<VodCatalogItem>> {
    db::with_conn(&state.db, move |conn| Ok(db::vod::get_item(conn, &playlist_id, content_type, &item_id)?)).await
}

/// Dashboard "Trending" rail data - top-rated movies already cached locally,
/// no TMDB involved. Does not trigger a sync (unlike `vod_get_items`): a
/// dashboard rail shouldn't pay for a slow first Xtream sync.
#[tauri::command]
pub async fn vod_get_top_rated(
    state: State<'_, AppState>,
    playlist_id: String,
    limit: i64,
) -> CommandResult<Vec<VodCatalogItem>> {
    db::with_conn(&state.db, move |conn| Ok(db::vod::get_top_rated_movies(conn, &playlist_id, limit)?)).await
}

/// Remote-first, paginated single-category fetch for Stalker VOD/series -
/// the "click a category, see it near-instantly" replacement for the old
/// eager whole-catalog sync. Fetches one live page, maps it to the
/// provider-agnostic `VodCatalogItem` shape, and opportunistically upserts
/// into the local cache as a side effect (never a precondition). Stalker
/// only - Xtream's eager sync is already fast and complete.
///
/// `search`, when set, is forwarded to the portal as a title-search filter
/// instead of `category_id` scoping to one category - live-search callers
/// pass `category_id: "*"` to span the whole catalog.
#[tauri::command]
pub async fn vod_get_items_live(
    state: State<'_, AppState>,
    playlist_id: String,
    content_type: VodContentType,
    category_id: String,
    page: i64,
    search: Option<String>,
) -> CommandResult<VodLivePage> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    if playlist.playlist_type != PlaylistType::Stalker {
        return Err(CommandError::Internal("vod_get_items_live is only supported for Stalker playlists".into()));
    }
    let stalker_type = stalker_content_type_of(content_type);
    match vod_get_items_live_once(&state, &playlist, content_type, stalker_type, &category_id, page, search.as_deref()).await {
        Err(CommandError::Auth(_)) => {
            stalker_cmds::reauthenticate(&state, &playlist_id, &playlist).await?;
            let refreshed = load_playlist(&state, &playlist_id).await?;
            vod_get_items_live_once(&state, &refreshed, content_type, stalker_type, &category_id, page, search.as_deref()).await
        }
        other => other,
    }
}

async fn vod_get_items_live_once(
    state: &AppState,
    playlist: &Playlist,
    content_type: VodContentType,
    stalker_type: StalkerContentType,
    category_id: &str,
    page: i64,
    search: Option<&str>,
) -> CommandResult<VodLivePage> {
    let creds = stalker_cmds::build_creds(playlist)?;
    let token = stalker_cmds::require_token(playlist)?;
    let raw_page = stalker_content::get_content(&state.http, &creds, &token, stalker_type, Some(category_id), page, search).await?;

    let items: Vec<VodCatalogItem> = raw_page.data.iter().map(|i| stalker_item_to_catalog_item(i, content_type)).collect();

    // Awaited, not spawned detached: `set_detail_json` needs the row to
    // already exist by the time the user clicks into anything here.
    // Best-effort - a cache write failure must not fail the render.
    let playlist_id = playlist.id.clone();
    let cache_items = items.clone();
    if let Err(e) = db::with_conn(&state.db, move |conn| Ok(db::vod::upsert_items(conn, &playlist_id, content_type, &cache_items)?)).await {
        tracing::warn!("Opportunistic VOD cache write failed for playlist '{}' category {category_id}: {e}", playlist.id);
    }

    // `page` echoes back the page WE requested, not `raw_page.cur_page` -
    // some portals return `cur_page: 0` always, regardless of what was sent,
    // even though `data` genuinely differs per page. Trusting that echo made
    // `fetchLiveBatch`'s "load more" silently re-request page 1 forever.
    // `crawl_itv_pages`/`crawl_vod_or_series_pages` use the same fix.
    Ok(VodLivePage { items, page, total_pages: raw_page.total_pages, total_items: raw_page.total_items })
}

/// Explicit resync entry point. The manual "Refresh" path and daily
/// scheduler call `vod_sync_playlist` directly instead (no IPC context);
/// this command exists for a future direct "resync VOD now" UI affordance.
#[tauri::command]
pub async fn vod_sync(state: State<'_, AppState>, playlist_id: String) -> CommandResult<()> {
    let playlist = load_playlist(&state, &playlist_id).await?;
    vod_sync_playlist(&state, &playlist).await
}

async fn ensure_synced(state: &AppState, playlist_id: &str, content_type: VodContentType) -> CommandResult<()> {
    let id = playlist_id.to_string();
    let already = db::with_conn(&state.db, move |conn| Ok(db::vod::is_synced(conn, &id, content_type)?)).await?;
    if already {
        return Ok(());
    }
    let playlist = load_playlist(state, playlist_id).await?;
    vod_sync_playlist(state, &playlist).await
}

/// Full bulk sync for one playlist - both movie and series catalogs. Called
/// detached from playlist add/refresh, from the scheduler, and from
/// `ensure_synced`'s lazy first-visit path. M3U has no VOD API; that branch
/// is a deliberate no-op, not an error, in case anything ever calls it.
pub async fn vod_sync_playlist(state: &AppState, playlist: &Playlist) -> CommandResult<()> {
    match playlist.playlist_type {
        PlaylistType::Xtream => {
            sync_content_type_xtream(state, playlist, VodContentType::Movie).await?;
            sync_content_type_xtream(state, playlist, VodContentType::Series).await?;
        }
        PlaylistType::Stalker => {
            sync_content_type_stalker(state, playlist, VodContentType::Movie).await?;
            sync_content_type_stalker(state, playlist, VodContentType::Series).await?;
        }
        PlaylistType::M3u => {}
    }
    Ok(())
}

fn xtream_stream_to_catalog_item(stream: &XtreamStream, content_type: VodContentType) -> VodCatalogItem {
    let provider_id = match content_type {
        VodContentType::Series => stream.series_id.unwrap_or(stream.stream_id),
        VodContentType::Movie => stream.stream_id,
    };
    let cover = stream
        .cover
        .clone()
        .or_else(|| if stream.stream_icon.is_empty() { None } else { Some(stream.stream_icon.clone()) });

    VodCatalogItem {
        id: provider_id.to_string(),
        content_type,
        category_id: Some(stream.category_id.clone()),
        name: stream.name.clone(),
        cover,
        rating: stream.rating.clone(),
        genre: stream.genre.clone(),
        release_date: stream.release_date.clone(),
        container_extension: stream.container_extension.clone(),
        stalker_item: None,
    }
}

async fn sync_content_type_xtream(state: &AppState, playlist: &Playlist, content_type: VodContentType) -> CommandResult<()> {
    let server_url = playlist
        .server_url
        .as_deref()
        .ok_or_else(|| CommandError::Internal("Playlist has no Xtream server URL".into()))?;
    let username = playlist
        .username
        .as_deref()
        .ok_or_else(|| CommandError::Internal("Playlist has no Xtream username".into()))?;
    let password = playlist
        .password
        .as_deref()
        .ok_or_else(|| CommandError::Internal("Playlist has no Xtream password".into()))?;
    let stream_type = xtream_stream_type(content_type);

    let categories = xtream::get_categories(&state.http, server_url, username, password, stream_type).await?;
    let streams = xtream::get_streams(&state.http, server_url, username, password, stream_type, None).await?;

    let category_pairs: Vec<(String, String)> = categories.into_iter().map(|c| (c.category_id, c.category_name)).collect();
    let items: Vec<VodCatalogItem> = streams.iter().map(|s| xtream_stream_to_catalog_item(s, content_type)).collect();

    write_synced_catalog(state, &playlist.id, content_type, category_pairs, items).await
}

fn stalker_item_to_catalog_item(item: &StalkerContentItem, content_type: VodContentType) -> VodCatalogItem {
    VodCatalogItem {
        id: item.id.clone(),
        content_type,
        category_id: item.category_id.clone(),
        name: item.name.clone(),
        cover: item.screenshot_uri.clone().or_else(|| item.cover.clone()),
        rating: item.rating_imdb.clone(),
        genre: item.genres_str.clone(),
        release_date: item.year.clone(),
        // Stalker never declares a container extension up front - playback
        // always resolves fresh via `create_link` at play time regardless.
        container_extension: None,
        stalker_item: Some(item.clone()),
    }
}

async fn sync_content_type_stalker(state: &AppState, playlist: &Playlist, content_type: VodContentType) -> CommandResult<()> {
    let stalker_type = stalker_content_type_of(content_type);
    match sync_content_type_stalker_once(state, playlist, content_type, stalker_type).await {
        Err(CommandError::Auth(_)) => {
            // Session/token can be invalidated out from under us anytime -
            // same retry-once policy every other Stalker command uses.
            stalker_cmds::reauthenticate(state, &playlist.id, playlist).await?;
            let refreshed = load_playlist(state, &playlist.id).await?;
            sync_content_type_stalker_once(state, &refreshed, content_type, stalker_type).await
        }
        other => other,
    }
}

async fn sync_content_type_stalker_once(
    state: &AppState,
    playlist: &Playlist,
    content_type: VodContentType,
    stalker_type: StalkerContentType,
) -> CommandResult<()> {
    let creds = stalker_cmds::build_creds(playlist)?;
    let token = stalker_cmds::require_token(playlist)?;

    let categories = stalker_content::get_categories(&state.http, &creds, &token, stalker_type).await?;
    let raw_items = stalker_content::crawl_vod_or_series_pages(&state.http, &creds, &token, stalker_type, "*").await?;

    // Captured from the raw items BEFORE mapping to `VodCatalogItem` erases
    // `category_id`/`id` down to what recovery actually needs to diff
    // against - see `spawn_censored_vod_recovery`'s doc comment for why.
    let existing_category_ids: HashSet<String> = raw_items.iter().filter_map(|i| i.category_id.clone()).collect();
    let existing_ids: HashSet<String> = raw_items.iter().map(|i| i.id.clone()).collect();

    let category_pairs: Vec<(String, String)> = categories.into_iter().map(|c| (c.id, c.title)).collect();
    let items: Vec<VodCatalogItem> = raw_items.iter().map(|i| stalker_item_to_catalog_item(i, content_type)).collect();

    write_synced_catalog(state, &playlist.id, content_type, category_pairs, items).await?;

    spawn_censored_vod_recovery(state, &playlist.id, &creds, &token, stalker_type, content_type, existing_category_ids, existing_ids);

    Ok(())
}

/// Recovering adult/"censored" VOD/series genres excluded from the fast
/// wildcard sync is slow (a paginated crawl per missing genre), so - like
/// `commands::stalker::spawn_censored_itv_recovery` - it always runs
/// detached. This matters more here: `sync_content_type_stalker_once` also
/// runs inline on `ensure_synced`'s lazy first-visit sync (awaited directly
/// while the user watches a loading spinner), so inline recovery would block
/// that spinner for however long the crawl takes. `creds`/`token` are
/// cloned to owned data since the spawned task must outlive the borrows.
///
/// Same three properties as the ITV version: each category commits as it
/// finishes, a `vod-updated` event follows each commit, and
/// `recovery_inflight` caps one run per playlist+content-type.
#[allow(clippy::too_many_arguments)]
fn spawn_censored_vod_recovery(
    state: &AppState,
    playlist_id: &str,
    creds: &StalkerCredentials<'_>,
    token: &str,
    stalker_type: StalkerContentType,
    content_type: VodContentType,
    existing_category_ids: HashSet<String>,
    existing_ids: HashSet<String>,
) {
    let key = format!("vod:{playlist_id}:{}", content_type.as_str());
    let inflight = state.recovery_inflight.clone();
    {
        let mut guard = inflight.lock().unwrap();
        if !guard.insert(key.clone()) {
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

        run_censored_vod_recovery(
            &http,
            &db,
            app.as_ref(),
            &creds,
            &token,
            &playlist_id,
            stalker_type,
            content_type,
            existing_category_ids,
            existing_ids,
        )
        .await;

        inflight.lock().unwrap().remove(&key);
    });
}

/// The body of `spawn_censored_vod_recovery`'s detached task, split out only
/// so the in-flight guard is released on every exit path without threading an
/// early `return` through it.
#[allow(clippy::too_many_arguments)]
async fn run_censored_vod_recovery(
    http: &reqwest::Client,
    db: &db::DbPool,
    app: Option<&tauri::AppHandle>,
    creds: &StalkerCredentials<'_>,
    token: &str,
    playlist_id: &str,
    stalker_type: StalkerContentType,
    content_type: VodContentType,
    existing_category_ids: HashSet<String>,
    existing_ids: HashSet<String>,
) {
    let missing = match stalker_content::find_missing_vod_or_series_categories(
        http,
        creds,
        token,
        stalker_type,
        &existing_category_ids,
    )
    .await
    {
        Ok(missing) => missing,
        Err(e) => {
            // `error!`, not `warn!` - real content missing, not a routine
            // hiccup - kept visible under an errors-only filter.
            tracing::error!("Censored {} category recovery failed for playlist {playlist_id}: {e}", stalker_type.as_str());
            return;
        }
    };
    if missing.is_empty() {
        return;
    }

    // `info!`, not `error!` - attempting a recovery crawl isn't itself a
    // problem. Filtered out under this app's errors-only subscriber.
    tracing::info!(
        "Censored {} category recovery: attempting {} categor{} missing from the fast sync: {:?}",
        stalker_type.as_str(),
        missing.len(),
        if missing.len() == 1 { "y" } else { "ies" },
        missing
    );

    let mut seen_ids = existing_ids;
    let mut total_recovered = 0usize;

    for category_id in &missing {
        let recovered = match stalker_content::crawl_vod_or_series_pages(http, creds, token, stalker_type, category_id).await {
            Ok(recovered) => recovered,
            Err(e) => {
                // Logged per-category rather than aborting the whole recovery
                // run - one flaky category must not silently take every other
                // missing category down with it.
                tracing::error!(
                    "Censored {} category recovery: failed to fetch category {category_id}: {e}",
                    stalker_type.as_str()
                );
                continue;
            }
        };
        let items: Vec<VodCatalogItem> = recovered
            .iter()
            .filter(|item| seen_ids.insert(item.id.clone()))
            .map(|i| stalker_item_to_catalog_item(i, content_type))
            .collect();
        if items.is_empty() {
            continue;
        }

        let count = items.len();
        let pid = playlist_id.to_string();
        // Plain insert, no preceding `delete_items` - the main sync already
        // wrote (and deleted-then-inserted) its own rows; this only adds what
        // that pass never saw at all, matching `insert_items`'s own "nothing
        // to conflict with" append semantics.
        let result = db::with_conn(db, move |conn| Ok(db::vod::insert_items(conn, &pid, content_type, &items)?)).await;
        match result {
            Ok(_) => {
                total_recovered += count;
                notify_vod_updated(app, playlist_id, content_type);
            }
            Err(e) => {
                tracing::error!(
                    "Censored {} category recovery DB write failed for playlist {playlist_id} (category {category_id}): {e}",
                    stalker_type.as_str()
                );
            }
        }
    }

    if total_recovered == 0 {
        // `warn!`, not `error!` - no transport/portal failure, just empty
        // results for every missing category. Not a confirmed failure.
        tracing::warn!(
            "Censored {} category recovery for playlist {playlist_id}: ran but found 0 items across every category missing from the fast sync",
            stalker_type.as_str()
        );
    }
}

/// Tells the frontend that `playlist_id`'s stored VOD/series catalog changed
/// underneath it - see `commands::stalker::notify_channels_updated` for why
/// rows written after the catalog was rendered are otherwise never seen.
fn notify_vod_updated(app: Option<&tauri::AppHandle>, playlist_id: &str, content_type: VodContentType) {
    let Some(app) = app else { return };
    let _ = tauri::Emitter::emit(app, "vod-updated", (playlist_id, content_type.as_str()));
}

async fn write_synced_catalog(
    state: &AppState,
    playlist_id: &str,
    content_type: VodContentType,
    category_pairs: Vec<(String, String)>,
    items: Vec<VodCatalogItem>,
) -> CommandResult<()> {
    let playlist_id = playlist_id.to_string();
    let synced_at = chrono::Utc::now().to_rfc3339();

    // Some providers' flat stream/content lists repeat the same item once
    // per category it's filed under (same `stream_id`/id, different
    // `category_id`) instead of listing it once. `insert_items` is a plain
    // INSERT (see its own doc comment) that relies on the batch itself being
    // free of duplicates - `delete_items` only clears what's already in the
    // table, not collisions within the new batch. Keeping the first
    // occurrence here (rather than deduping upstream in each provider's own
    // sync fn) covers both Xtream and Stalker at their one shared choke
    // point and is a no-op for every provider that never repeats an id.
    let mut seen_ids = HashSet::new();
    let items: Vec<VodCatalogItem> = items.into_iter().filter(|item| seen_ids.insert(item.id.clone())).collect();

    db::with_conn(&state.db, move |conn| {
        db::vod::delete_categories(conn, &playlist_id, content_type)?;
        db::vod::insert_categories(conn, &playlist_id, content_type, &category_pairs)?;
        db::vod::delete_items(conn, &playlist_id, content_type)?;
        db::vod::insert_items(conn, &playlist_id, content_type, &items)?;
        db::vod::mark_synced(conn, &playlist_id, content_type, &synced_at)?;
        Ok(())
    })
    .await
}
