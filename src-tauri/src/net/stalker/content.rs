use super::auth::{stalker_get, StalkerCredentials};
use super::identity;
use crate::error::{CommandError, CommandResult};
use crate::types::{
    Channel, ChannelGroup, ChannelHttp, ChannelTvg, SeasonEpisode, SeasonInfo, SeriesDetails,
    StalkerCategory, StalkerContentItem, StalkerContentPage, StalkerContentType, StreamType, VodDetails,
};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn value_to_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// The pseudo-category every `get_genres`/`get_categories` response leads
/// with (`{"id": "*", "title": "All"}`) - a UI "no filter" affordance, not a
/// real genre (no row ever carries `tv_genre_id == "*"`). A recovery pass
/// that treated it like a real id concluded it was always missing and
/// re-crawled the entire catalog (~7000 duplicate rows) ahead of the
/// genuinely missing adult genres. Recovery skips it everywhere.
const WILDCARD_CATEGORY_ID: &str = "*";

fn category_action(content_type: StalkerContentType) -> &'static str {
    match content_type {
        StalkerContentType::Itv => "get_genres",
        _ => "get_categories",
    }
}

pub async fn get_categories(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    content_type: StalkerContentType,
) -> CommandResult<Vec<StalkerCategory>> {
    let headers = identity::build_api_headers(creds.mac_address, creds.serial_number, Some(token));
    let url = identity::build_request_url(
        creds.portal_url,
        &[("type", content_type.as_str()), ("action", category_action(content_type))],
    );
    let body = stalker_get(http, &url, &headers, 15).await?;
    let items = body.pointer("/js").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    Ok(items.iter().filter_map(parse_category).collect())
}

fn parse_category(v: &Value) -> Option<StalkerCategory> {
    Some(StalkerCategory {
        id: v.get("id").and_then(value_to_string)?,
        title: v.get("title").and_then(value_to_string).unwrap_or_default(),
        alias: v.get("alias").and_then(value_to_string),
    })
}

/// `search`, when non-empty, is passed as `get_ordered_list`'s own portal-
/// side title search, scoped to the requested category (`category_id:
/// Some("*")` for catalog-wide). Not every portal honors it - an
/// unrecognized param just returns the unfiltered list - so callers also
/// apply their own client-side name filter (see `vod.svelte.ts`'s
/// `filteredItems`).
pub async fn get_content(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    content_type: StalkerContentType,
    category_id: Option<&str>,
    page: i64,
    search: Option<&str>,
) -> CommandResult<StalkerContentPage> {
    let headers = identity::build_api_headers(creds.mac_address, creds.serial_number, Some(token));
    let page_str = page.to_string();
    let category = category_id.unwrap_or("*");
    let mut params: Vec<(&str, &str)> = vec![
        ("type", content_type.as_str()),
        ("action", "get_ordered_list"),
        ("sortby", "added"),
        ("p", &page_str),
        ("category", category),
    ];
    if matches!(content_type, StalkerContentType::Vod | StalkerContentType::Itv) {
        params.push(("genre", category));
    }
    if let Some(q) = search.filter(|s| !s.trim().is_empty()) {
        params.push(("search", q));
    }
    let url = identity::build_request_url(creds.portal_url, &params);
    let body = stalker_get(http, &url, &headers, 20).await?;
    parse_content_page(&body, creds.portal_url)
}

/// `total_pages` is computed from `total_items`/`max_page_items` rather than
/// trusted from a `total_pages` field - a portal that omits it made every
/// list look like it had just one page, no matter how large.
fn parse_content_page(body: &Value, portal_url: &str) -> CommandResult<StalkerContentPage> {
    let empty = Value::Null;
    let js = body.pointer("/js").unwrap_or(&empty);
    let data = js.get("data").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let items: Vec<StalkerContentItem> = data.iter().filter_map(|v| parse_content_item(v, portal_url)).collect();
    let item_count = items.len() as i64;

    let total_items = js.get("total_items").and_then(value_to_i64).unwrap_or(item_count).max(item_count);
    let max_page_items = js
        .get("max_page_items")
        .and_then(value_to_i64)
        .filter(|n| *n > 0)
        .unwrap_or_else(|| if item_count > 0 { item_count } else { 14 });
    let total_pages = ((total_items as f64) / (max_page_items as f64)).ceil().max(1.0) as i64;
    let cur_page = js.get("cur_page").and_then(value_to_i64).unwrap_or(1);

    Ok(StalkerContentPage {
        total_items,
        max_page_items,
        cur_page,
        total_pages,
        data: items,
    })
}

fn parse_content_item(v: &Value, portal_url: &str) -> Option<StalkerContentItem> {
    let series: Option<Vec<String>> = v
        .get("series")
        .and_then(|s| s.as_array())
        .map(|arr| arr.iter().filter_map(value_to_string).collect());
    let is_series = v
        .get("is_series")
        .and_then(value_to_i64)
        .map(|n| n != 0)
        .unwrap_or_else(|| series.as_ref().is_some_and(|s| !s.is_empty()));

    let screenshot_uri = v
        .get("screenshot_uri")
        .and_then(value_to_string)
        .map(|s| make_absolute_url(portal_url, &s));

    Some(StalkerContentItem {
        id: v.get("id").and_then(value_to_string)?,
        name: v.get("name").and_then(value_to_string).unwrap_or_default(),
        cmd: v.get("cmd").and_then(value_to_string),
        screenshot_uri,
        cover: v.get("cover").and_then(value_to_string).map(|s| make_absolute_url(portal_url, &s)),
        description: v.get("description").and_then(value_to_string),
        actors: v.get("actors").and_then(value_to_string),
        director: v.get("director").and_then(value_to_string),
        year: v.get("year").and_then(value_to_string),
        rating_imdb: v.get("rating_imdb").and_then(value_to_string),
        category_id: v.get("category_id").and_then(value_to_string),
        is_series,
        series,
        has_files: v.get("has_files").and_then(value_to_i64),
        use_http_tmp_link: v.get("use_http_tmp_link").and_then(value_to_string),
        use_load_balancing: v.get("use_load_balancing").and_then(value_to_string),
        genres_str: v.get("genres_str").and_then(value_to_string),
    })
}

fn truthy(v: Option<&str>) -> bool {
    matches!(v, Some(s) if s == "1" || s.eq_ignore_ascii_case("true"))
}

fn is_portal_local_host(host: &str) -> bool {
    host == "localhost" || host.ends_with(".localhost") || host.starts_with("127.") || host == "::1"
}

/// Strips a leading "solution token" some rows prefix onto their `cmd`
/// before the actual URL (e.g. `ffmpeg http://host/live.php` -> the URL).
fn normalize_stalker_cmd(cmd: &str) -> String {
    crate::net::url_utils::strip_solution_token(cmd)
}

/// Resolves a relative `screenshot_uri`/thumbnail path against the portal's
/// origin (scheme+host+port only, never its install path - portals serve
/// thumbnails off the plain origin regardless of API script location). An
/// already-absolute URL passes through; an unparseable base fails open to
/// the original path. Simpler than `resolve_relative_cmd` (install-path-
/// aware, for playback `cmd`) - these are two distinct rules.
fn make_absolute_url(portal_url: &str, relative: &str) -> String {
    if relative.is_empty() {
        return String::new();
    }
    if relative.starts_with("http://") || relative.starts_with("https://") {
        return relative.to_string();
    }
    let Ok(base) = reqwest::Url::parse(portal_url) else {
        return relative.to_string();
    };
    let port_suffix = base.port().map(|p| format!(":{p}")).unwrap_or_default();
    let path = if let Some(rest) = relative.strip_prefix('/') {
        format!("/{rest}")
    } else {
        format!("/{relative}")
    };
    format!("{}://{}{}{}", base.scheme(), base.host_str().unwrap_or(""), port_suffix, path)
}

/// Only calls `create_link` when there's concrete reason to think `cmd`
/// isn't already directly playable - otherwise the static `cmd` plays
/// without the extra round trip. No evidence either way (legacy rows with
/// neither flag) fails toward the safe path and calls it anyway.
fn needs_create_link(cmd: Option<&str>, use_http_tmp_link: Option<&str>, use_load_balancing: Option<&str>) -> bool {
    if truthy(use_http_tmp_link) || truthy(use_load_balancing) {
        return true;
    }
    let Some(cmd) = cmd else { return true };
    let normalized = normalize_stalker_cmd(cmd);
    match reqwest::Url::parse(&normalized) {
        Ok(url) => {
            if url.scheme() != "http" && url.scheme() != "https" {
                return true;
            }
            match url.host_str() {
                Some(host) => is_portal_local_host(host),
                None => true,
            }
        }
        Err(_) => true,
    }
}

fn resolve_relative_cmd(portal_url: &str, cmd: &str) -> CommandResult<String> {
    if cmd.starts_with("http://") || cmd.starts_with("https://") {
        return Ok(cmd.to_string());
    }
    let base = reqwest::Url::parse(portal_url).map_err(|_| CommandError::Internal("Invalid portal URL".into()))?;
    let origin = format!("{}://{}", base.scheme(), base.host_str().unwrap_or(""));
    if let Some(rest) = cmd.strip_prefix('/') {
        return Ok(format!("{origin}/{rest}"));
    }
    if cmd.starts_with('?') {
        // Query-only response: resolve against the portal's install base
        // path (strip the trailing portal.php/server/load.php segment).
        let path = base.path();
        let install_base = path.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
        return Ok(format!("{origin}{install_base}/{cmd}"));
    }
    Ok(format!("{origin}/{cmd}"))
}

async fn create_link(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    content_type: StalkerContentType,
    cmd: &str,
    series: Option<&str>,
) -> CommandResult<String> {
    let headers = identity::build_api_headers(creds.mac_address, creds.serial_number, Some(token));
    let type_str = if series.is_some() { "vod" } else { content_type.as_str() };
    let mut params: Vec<(&str, &str)> = vec![
        ("action", "create_link"),
        ("type", type_str),
        ("cmd", cmd),
        ("disable_ad", "0"),
        ("download", "0"),
    ];
    if let Some(series) = series {
        params.push(("series", series));
    }
    let url = identity::build_request_url(creds.portal_url, &params);
    let body = stalker_get(http, &url, &headers, 30).await?;
    let resolved = body
        .pointer("/js/cmd")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CommandError::InvalidResponse("The portal didn't return a playable link for this title.".into()))?;
    resolve_relative_cmd(creds.portal_url, &normalize_stalker_cmd(resolved))
}

#[allow(clippy::too_many_arguments)]
pub async fn resolve_playback(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    content_type: StalkerContentType,
    cmd: &str,
    use_http_tmp_link: Option<&str>,
    use_load_balancing: Option<&str>,
    series: Option<&str>,
) -> CommandResult<String> {
    if needs_create_link(Some(cmd), use_http_tmp_link, use_load_balancing) {
        create_link(http, creds, token, content_type, cmd, series).await
    } else {
        Ok(normalize_stalker_cmd(cmd))
    }
}

/// Series episodes always re-resolve via `create_link` (no flag-based skip)
/// — an eagerly-resolved episode URL from when the series detail loaded can
/// already be a dead temporary link by the time it's actually clicked.
pub async fn resolve_vod_episode(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    content_type: StalkerContentType,
    cmd: &str,
    series: Option<&str>,
) -> CommandResult<String> {
    create_link(http, creds, token, content_type, cmd, series).await
}

pub async fn get_stream_headers(
    _http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
) -> Vec<(String, String)> {
    identity::build_api_headers(creds.mac_address, creds.serial_number, Some(token))
}

/// Fetches every ITV channel into the unified `Channel` shape. `raw` carries
/// the portal's `use_http_tmp_link`/`use_load_balancing` flags as a
/// snake_case JSON blob - the frontend's `parseLinkFlags` reads those keys.
///
/// Tries the one-shot `get_all_channels` action first (sidesteps pagination),
/// falling back to crawling `get_ordered_list`. Category names are resolved
/// once via `get_genres` to fill `Channel.group.title`; lookup failure
/// degrades to blank titles rather than failing the sync.
///
/// Deliberately wildcard-only, matching iptvnator - real Ministra portals
/// exclude "censored" (adult) genres from this call. Recovering those is a
/// SEPARATE, slow, per-category operation (`find_missing_itv_categories` +
/// `crawl_itv_category`) that must never run inside this function - folding
/// it in here once blocked playlist import for minutes on adult-heavy portals.
///
/// Returns the mapped channels plus the raw ITV category ids they cover, so
/// recovery can tell exactly which ids came back empty rather than matching
/// on resolved titles (see `find_missing_itv_categories`'s doc for why that's buggy).
pub async fn get_all_channels(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
) -> CommandResult<(Vec<Channel>, std::collections::HashSet<String>)> {
    let categories = get_categories(http, creds, token, StalkerContentType::Itv).await.unwrap_or_default();
    let category_names: HashMap<String, String> = categories.into_iter().map(|c| (c.id, c.title)).collect();

    match try_get_all_channels_action(http, creds, token, &category_names).await {
        Ok((channels, ids)) if !channels.is_empty() => Ok((channels, ids)),
        _ => crawl_itv_pages(http, creds, token, "*", &category_names).await,
    }
}

/// Bulk crawl of an entire VOD/series catalog (`"*"` or one category id) -
/// the run-to-completion sibling of `get_content`'s single-page shape, for
/// the VOD cache's background sync. Mirrors `crawl_itv_pages`'s stop
/// conditions (zero new items, `cur_page >= total_pages`, 500-page cap) and
/// retry-once-per-page policy, but works with typed `StalkerContentItem`
/// directly since VOD/series need no `parse_channel_row`-style mapping.
pub async fn crawl_vod_or_series_pages(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    content_type: StalkerContentType,
    category: &str,
) -> CommandResult<Vec<StalkerContentItem>> {
    let mut all_items: Vec<StalkerContentItem> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    let mut page = 1i64;

    loop {
        let content_page = fetch_content_page_with_retry(http, creds, token, content_type, category, page).await?;
        let total_pages = content_page.total_pages;
        let cur_page = content_page.cur_page;
        let mut added = 0;
        for item in content_page.data {
            if seen_ids.insert(item.id.clone()) {
                all_items.push(item);
                added += 1;
            }
        }
        if added == 0 || cur_page >= total_pages || page > 500 {
            break;
        }
        page += 1;
    }

    Ok(all_items)
}

async fn fetch_content_page_with_retry(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    content_type: StalkerContentType,
    category: &str,
    page: i64,
) -> CommandResult<StalkerContentPage> {
    for attempt in 0..2 {
        match get_content(http, creds, token, content_type, Some(category), page, None).await {
            Ok(result) => return Ok(result),
            Err(e) if attempt == 0 => {
                tracing::warn!("Stalker {} page {page} fetch failed, retrying once: {e}", content_type.as_str());
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

fn extract_category_ids(rows: &[Value]) -> std::collections::HashSet<String> {
    rows.iter()
        .filter_map(|row| row.get("tv_genre_id").or_else(|| row.get("category_id")).and_then(value_to_string))
        .collect()
}

/// Lists ITV category ids entirely absent from `existing_category_ids` (the
/// fast sync's own `tv_genre_id`s) - genres a portal excludes from
/// `get_all_channels`/the `category=*` crawl, typically adult/"censored"
/// ones. `crawl_itv_category` fetches each individually.
///
/// **Matches by category id, not name** - an earlier revision compared
/// titles instead, which broke whenever two distinct category ids share a
/// title (common for regional adult sub-genres): one populated sibling
/// permanently masked every other category with that name from recovery.
/// Ids can't collide the same way, so presence is tracked by id.
///
/// The wildcard pseudo-genre is excluded - see `WILDCARD_CATEGORY_ID`.
pub async fn find_missing_itv_categories(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    existing_category_ids: &std::collections::HashSet<String>,
) -> CommandResult<MissingItvCategories> {
    let categories = get_categories(http, creds, token, StalkerContentType::Itv).await.unwrap_or_default();
    let category_names: HashMap<String, String> = categories.into_iter().map(|c| (c.id, c.title)).collect();

    let mut missing_ids: Vec<String> = category_names
        .keys()
        .filter(|id| id.as_str() != WILDCARD_CATEGORY_ID)
        .filter(|id| !existing_category_ids.contains(id.as_str()))
        .cloned()
        .collect();
    // `category_names` is a `HashMap`, so its key order is randomized per
    // process. Recovery writes each category to the DB as it finishes, and a
    // user watching the list fill in should see the same order twice in a
    // row; sorting also makes the log line below reproducible.
    missing_ids.sort();

    Ok(MissingItvCategories { category_names, missing_ids })
}

/// Result of `find_missing_itv_categories`: the id->title map for EVERY ITV
/// genre (needed to fill `Channel.group.title`, since channel rows only carry
/// a numeric `tv_genre_id`) plus just the ids that need recovering.
pub struct MissingItvCategories {
    pub category_names: HashMap<String, String>,
    pub missing_ids: Vec<String>,
}

/// Crawls one ITV category to completion (`category=<id>&genre=<id>`, both
/// set to the same value - never `genre: "*"` paired with one category), so
/// adult/"censored" genres excluded from `get_all_channels` are retrieved.
///
/// Deliberately ONE category per call, caller owns the loop over
/// `find_missing_itv_categories`'s ids: an earlier revision crawled every
/// missing category internally and returned one combined `Vec`, so quitting
/// mid-run (a real portal takes ~30s+ here) discarded every channel already
/// fetched. Per-category calls let the caller commit each genre as it completes.
pub async fn crawl_itv_category(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    category_id: &str,
    category_names: &HashMap<String, String>,
) -> CommandResult<Vec<Channel>> {
    let (channels, _) = crawl_itv_pages(http, creds, token, category_id, category_names).await?;
    Ok(channels)
}

/// VOD/series sibling of `find_missing_itv_categories` - same excluded-
/// adult-genre gap, but movie/series sync had no per-category recovery at
/// all until this. Matches by category id, not name, for the same reason
/// (distinct ids can share a title). `existing_category_ids` comes from the
/// caller's own just-completed wildcard crawl, not a DB reload, so recovery
/// only targets genres that crawl returned zero items for. Wildcard
/// pseudo-category excluded - see `WILDCARD_CATEGORY_ID`.
pub async fn find_missing_vod_or_series_categories(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    content_type: StalkerContentType,
    existing_category_ids: &std::collections::HashSet<String>,
) -> CommandResult<Vec<String>> {
    let categories = get_categories(http, creds, token, content_type).await.unwrap_or_default();

    let mut missing_category_ids: Vec<String> = categories
        .into_iter()
        .map(|c| c.id)
        .filter(|id| id.as_str() != WILDCARD_CATEGORY_ID)
        .filter(|id| !existing_category_ids.contains(id.as_str()))
        .collect();
    missing_category_ids.sort();

    Ok(missing_category_ids)
}

async fn try_get_all_channels_action(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    category_names: &HashMap<String, String>,
) -> CommandResult<(Vec<Channel>, std::collections::HashSet<String>)> {
    let headers = identity::build_api_headers(creds.mac_address, creds.serial_number, Some(token));
    let url = identity::build_request_url(creds.portal_url, &[("type", "itv"), ("action", "get_all_channels")]);
    let body = stalker_get(http, &url, &headers, 30).await?;
    let empty = Value::Null;
    let data = body.pointer("/js/data").unwrap_or(&empty).as_array().cloned().unwrap_or_default();
    let rows = dedup_raw_rows_by_id(data);
    let ids = extract_category_ids(&rows);
    let channels = rows.iter().map(|row| parse_channel_row(row, category_names, creds.portal_url)).collect();
    Ok((channels, ids))
}

/// Drops rows whose portal-assigned `id` repeats - `get_all_channels`/
/// overlapping page crawls can hand back the same channel twice, colliding
/// with the frontend's `{#each ... (channel.id)}` keying. Must dedup on the
/// RAW rows (portal id) before mapping assigns each a fresh uuid.
fn dedup_raw_rows_by_id(rows: Vec<Value>) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    rows.into_iter()
        .filter(|row| match row.get("id").and_then(value_to_string) {
            Some(id) => seen.insert(id),
            None => true,
        })
        .collect()
}

/// Crawls `get_ordered_list(type=itv)` pages using `parse_content_page`'s
/// corrected `total_pages`, retrying each page once on failure, and stopping
/// early on zero new ids (else a portal ignoring `p` loops to the 500-page cap).
async fn crawl_itv_pages(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    category: &str,
    category_names: &HashMap<String, String>,
) -> CommandResult<(Vec<Channel>, std::collections::HashSet<String>)> {
    let mut all_raw: Vec<Value> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    let mut page = 1i64;

    loop {
        let Some(content_page) = fetch_itv_page_with_retry(http, creds, token, category, page).await? else {
            break;
        };
        let mut added = 0;
        for row in &content_page.0 {
            if let Some(id) = row.get("id").and_then(value_to_string) {
                if !seen_ids.insert(id) {
                    continue;
                }
            }
            all_raw.push(row.clone());
            added += 1;
        }
        if added == 0 || content_page.1.cur_page >= content_page.1.total_pages || page > 500 {
            break;
        }
        page += 1;
    }

    let ids = extract_category_ids(&all_raw);
    let channels = all_raw.iter().map(|row| parse_channel_row(row, category_names, creds.portal_url)).collect();
    Ok((channels, ids))
}

async fn fetch_itv_page_with_retry(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    category: &str,
    page: i64,
) -> CommandResult<Option<(Vec<Value>, StalkerContentPage)>> {
    for attempt in 0..2 {
        match fetch_itv_page(http, creds, token, category, page).await {
            Ok(result) => return Ok(result),
            Err(e) if attempt == 0 => {
                tracing::warn!("Stalker ITV page {page} fetch failed, retrying once: {e}");
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

async fn fetch_itv_page(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    category: &str,
    page: i64,
) -> CommandResult<Option<(Vec<Value>, StalkerContentPage)>> {
    let headers = identity::build_api_headers(creds.mac_address, creds.serial_number, Some(token));
    let page_str = page.to_string();
    // Both `category` and `genre` are set to the same value (wildcard for
    // the full crawl, one id for per-category recovery) - never `genre: "*"`
    // paired with a specific `category`.
    let url = identity::build_request_url(
        creds.portal_url,
        &[
            ("type", "itv"),
            ("action", "get_ordered_list"),
            ("sortby", "number"),
            ("p", &page_str),
            ("category", category),
            ("genre", category),
        ],
    );
    let body = stalker_get(http, &url, &headers, 30).await?;
    let empty = Value::Null;
    let raw_data = body.pointer("/js/data").unwrap_or(&empty).as_array().cloned().unwrap_or_default();
    if raw_data.is_empty() {
        return Ok(None);
    }
    let page_info = parse_content_page(&body, creds.portal_url)?;
    Ok(Some((raw_data, page_info)))
}


fn parse_channel_row(v: &Value, category_names: &HashMap<String, String>, portal_url: &str) -> Channel {
    let use_http_tmp_link = v.get("use_http_tmp_link").and_then(value_to_string);
    let use_load_balancing = v.get("use_load_balancing").and_then(value_to_string);
    // The portal's own numeric channel id doesn't survive elsewhere in the
    // unified `Channel` shape - stashed here so `stalker_sync_epg` can call
    // `get_short_epg(ch_id=..)` for a specific stored channel later.
    let stalker_channel_id = v.get("id").and_then(value_to_string);
    // The RAW portal category id, not the resolved name - lets category-
    // aware sync/recovery scope DB ops to an exact id rather than a name
    // (two ids can share a title, see `find_missing_itv_categories`).
    let raw_category_id = v.get("tv_genre_id").or_else(|| v.get("category_id")).and_then(value_to_string);
    let raw = Some(
        serde_json::json!({
            "stalker_channel_id": stalker_channel_id,
            "use_http_tmp_link": use_http_tmp_link,
            "use_load_balancing": use_load_balancing,
            "category_id": raw_category_id.clone(),
        })
        .to_string(),
    );

    Channel {
        id: uuid::Uuid::new_v4().to_string(),
        url: v.get("cmd").and_then(value_to_string).unwrap_or_default(),
        name: v.get("name").and_then(value_to_string).unwrap_or_default(),
        group: ChannelGroup {
            title: raw_category_id
                .map(|id| category_names.get(&id).cloned().unwrap_or(id))
                .unwrap_or_default(),
        },
        tvg: ChannelTvg {
            id: v.get("xmltv_id").and_then(value_to_string),
            name: v.get("name").and_then(value_to_string),
            url: None,
            logo: v
                .get("logo")
                .or_else(|| v.get("screenshot_uri"))
                .and_then(value_to_string)
                .map(|s| make_absolute_url(portal_url, &s)),
            rec: None,
        },
        epg_params: None,
        timeshift: None,
        catchup: None,
        http: ChannelHttp::default(),
        radio: "0".to_string(),
        drm: None,
        raw,
        channel_number: v.get("number").and_then(value_to_i64),
    }
}

pub fn item_to_vod_details(item: &StalkerContentItem, content_type: StalkerContentType) -> VodDetails {
    VodDetails {
        id: item.id.clone(),
        name: item.name.clone(),
        stream_type: StreamType::Movie,
        container_extension: None,
        direct_source: None,
        series_id: None,
        season_number: None,
        episode_number: None,
        cover: item.cover.clone().or_else(|| item.screenshot_uri.clone()),
        // Left unresolved - `create_link` results are temporary and must be
        // re-resolved fresh right before play, not cached from fetch time.
        stream_url: None,
        plot: item.description.clone(),
        cast: item.actors.clone(),
        rating: item.rating_imdb.clone(),
        genre: item.genres_str.clone(),
        release_date: item.year.clone(),
        tmdb_id: None,
        seasons: None,
        episodes: None,
        cmd: item.cmd.clone(),
        use_http_tmp_link: item.use_http_tmp_link.clone(),
        use_load_balancing: item.use_load_balancing.clone(),
        stalker_content_type: Some(content_type.as_str().to_string()),
    }
}

/// Best-effort reconstruction of season/episode structure from `get_ordered_
/// list(type=series, movie_id=<id>)` - each row is treated as one season with
/// its own `cmd` and a `series` array of episode indices. Least-verified
/// piece of the Stalker implementation - real portals vary here.
pub async fn get_series_details(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    content_type: StalkerContentType,
    item: &StalkerContentItem,
) -> CommandResult<SeriesDetails> {
    let headers = identity::build_api_headers(creds.mac_address, creds.serial_number, Some(token));
    // Only `{action: get_ordered_list, type: series, movie_id}` - earlier
    // revisions also sent `season_id`/`episode_id`/`category`, but listing
    // here is unconditional; scoping params could only narrow a response
    // some portals expect unscoped.
    let url = identity::build_request_url(
        creds.portal_url,
        &[("type", "series"), ("action", "get_ordered_list"), ("movie_id", item.id.as_str())],
    );
    let body = stalker_get(http, &url, &headers, 20).await?;
    let empty = Value::Null;
    let js = body.pointer("/js").unwrap_or(&empty);
    let data = js.get("data").and_then(|v| v.as_array()).cloned().unwrap_or_default();

    let mut seasons = Vec::new();
    let mut episodes: HashMap<String, Vec<SeasonEpisode>> = HashMap::new();

    for (index, row) in data.iter().enumerate() {
        let season_number = (index as i64) + 1;
        let season_name = row
            .get("name")
            .and_then(value_to_string)
            .unwrap_or_else(|| format!("Season {season_number}"));
        let row_cmd = row.get("cmd").and_then(value_to_string);
        let episode_indices: Vec<String> = row
            .get("series")
            .and_then(|s| s.as_array())
            .map(|arr| arr.iter().filter_map(value_to_string).collect())
            .unwrap_or_default();

        seasons.push(SeasonInfo {
            id: row.get("id").and_then(value_to_i64),
            name: season_name,
            season_number,
            episode_count: Some(episode_indices.len() as i64),
            air_date: row.get("year").and_then(value_to_string),
            cover: row.get("screenshot_uri").and_then(value_to_string),
        });

        let season_episodes: Vec<SeasonEpisode> = episode_indices
            .iter()
            .enumerate()
            .map(|(ep_idx, series_param)| SeasonEpisode {
                id: format!("{}:{}", item.id, series_param),
                episode_num: Some((ep_idx as i64) + 1),
                title: format!("Episode {}", ep_idx + 1),
                season: season_number,
                container_extension: None,
                info: None,
                cover: None,
                plot: None,
                stream_url: None,
                direct_source: None,
                cmd: row_cmd.clone(),
                series_param: Some(series_param.clone()),
            })
            .collect();

        episodes.insert(season_number.to_string(), season_episodes);
    }

    Ok(SeriesDetails {
        info: item_to_vod_details(item, content_type),
        seasons,
        episodes,
    })
}

// ---------------------------------------------------------------------
// Native EPG (get_short_epg / get_epg_info)
// ---------------------------------------------------------------------

/// Matches `getProgramTimestampSeconds`: prefer an already-unix-seconds
/// value, but only when strictly positive; else parse `raw_value` as
/// RFC3339 or a plain `YYYY-MM-DD HH:MM:SS` string.
fn stalker_epg_timestamp_seconds(raw_value: &str, timestamp_value: Option<&Value>) -> Option<i64> {
    if let Some(ts) = timestamp_value.and_then(value_to_i64) {
        if ts > 0 {
            return Some(ts);
        }
    }
    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = if trimmed.contains('T') { trimmed.to_string() } else { trimmed.replacen(' ', "T", 1) };
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&candidate) {
        return Some(dt.timestamp());
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&candidate, "%Y-%m-%dT%H:%M:%S") {
        return Some(dt.and_utc().timestamp());
    }
    None
}

/// Matches `toIsoString` — note the deliberate asymmetry vs. the Xtream
/// port: on a date string that's neither a valid unix timestamp NOR
/// parseable as a real date, this returns EMPTY (not a raw passthrough),
/// which then drops the whole program via the `start.is_empty()` check in
/// `map_stalker_epg_item` — matches the reference exactly, since real
/// Stalker EPG payloads are plain, un-encoded text with no analogous
/// "maybe it's not actually formatted the way we expect but keep it
/// anyway" case the way Xtream's base64 fields have.
fn stalker_epg_iso_string(raw_value: &str, timestamp_seconds: Option<i64>) -> String {
    if let Some(ts) = timestamp_seconds {
        return chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.to_rfc3339()).unwrap_or_default();
    }
    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let candidate = if trimmed.contains('T') { trimmed.to_string() } else { trimmed.replacen(' ', "T", 1) };
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&candidate) {
        return dt.to_rfc3339();
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&candidate, "%Y-%m-%dT%H:%M:%S") {
        return dt.and_utc().to_rfc3339();
    }
    String::new()
}

/// Maps one raw Stalker EPG entry (`get_short_epg`/`get_epg_info` row) into
/// StreamFlow's unified EPG program shape, keyed by `channel_key` (the
/// unified `Channel`'s own id/tvg-id — see `commands::stalker::
/// stalker_sync_epg`, mirroring the Xtream port's same design). Titles/
/// descriptions are plain text — never base64, unlike Xtream.
fn map_stalker_epg_item(item: &Value, channel_key: &str) -> Option<crate::parsers::xmltv::ParsedEpgProgram> {
    let start_raw = item
        .get("time")
        .and_then(value_to_string)
        .or_else(|| item.get("start").and_then(value_to_string))
        .unwrap_or_default();
    let stop_raw = item
        .get("time_to")
        .and_then(value_to_string)
        .or_else(|| item.get("stop").and_then(value_to_string))
        .unwrap_or_default();
    let start_ts = stalker_epg_timestamp_seconds(&start_raw, item.get("start_timestamp"));
    let stop_ts = stalker_epg_timestamp_seconds(&stop_raw, item.get("stop_timestamp"));
    let start = stalker_epg_iso_string(&start_raw, start_ts);
    let stop = stalker_epg_iso_string(&stop_raw, stop_ts);
    if start.is_empty() || stop.is_empty() {
        return None;
    }

    Some(crate::parsers::xmltv::ParsedEpgProgram {
        channel_id: channel_key.to_string(),
        start,
        stop,
        title: item.get("name").and_then(value_to_string).unwrap_or_default(),
        description: item.get("descr").and_then(value_to_string),
        category: None,
        icon_url: None,
    })
}

/// `get_short_epg` — a rolling window of `size` upcoming programs for one
/// ITV channel, identified by the PORTAL's own numeric `ch_id` (not the
/// unified `Channel.id`) — matches `stalker-epg.md`'s documented request
/// shape exactly: `type=itv&action=get_short_epg&ch_id=<id>&size=<n>`.
pub async fn get_short_epg(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    stalker_channel_id: &str,
    size: i64,
    channel_key: &str,
) -> CommandResult<Vec<crate::parsers::xmltv::ParsedEpgProgram>> {
    let headers = identity::build_api_headers(creds.mac_address, creds.serial_number, Some(token));
    let size_str = size.to_string();
    let url = identity::build_request_url(
        creds.portal_url,
        &[("type", "itv"), ("action", "get_short_epg"), ("ch_id", stalker_channel_id), ("size", &size_str)],
    );
    let body = stalker_get(http, &url, &headers, 15).await?;
    let empty = Value::Null;
    let data = body.pointer("/js/data").unwrap_or(&empty).as_array().cloned().unwrap_or_default();
    let mut programs: Vec<_> = data.iter().filter_map(|v| map_stalker_epg_item(v, channel_key)).collect();
    programs.sort_by(|a, b| a.start.cmp(&b.start));
    Ok(programs)
}

/// `get_epg_info` - the bulk, whole-portal EPG for a `period`-hour window
/// (iptvnator always uses 168 = 7 days), keyed by the portal's numeric
/// channel id. Accepts all three shapes real/mock portals send: `js.data` as
/// a channel-id-keyed map, a bare array with each entry's own `ch_id`, or an
/// array nested under a `data`/`epg`/`items` key.
///
/// **Integration note**: unlike `get_short_epg` (keyed by the unified
/// `Channel`'s id), this bulk result is keyed by the PORTAL's raw channel id
/// - correlating it to a stored channel would need a new column. Exposed as
/// a faithful protocol implementation; not yet wired into persistent storage.
pub async fn get_epg_info(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    period_hours: i64,
) -> CommandResult<HashMap<String, Vec<crate::parsers::xmltv::ParsedEpgProgram>>> {
    let headers = identity::build_api_headers(creds.mac_address, creds.serial_number, Some(token));
    let period_str = period_hours.to_string();
    let url = identity::build_request_url(creds.portal_url, &[("type", "itv"), ("action", "get_epg_info"), ("period", &period_str)]);
    let body = stalker_get(http, &url, &headers, 30).await?;
    let empty = Value::Null;
    let data = body.pointer("/js/data").unwrap_or(&empty);

    let mut by_channel: HashMap<String, Vec<crate::parsers::xmltv::ParsedEpgProgram>> = HashMap::new();

    let push_entry = |by_channel: &mut HashMap<String, Vec<crate::parsers::xmltv::ParsedEpgProgram>>, entry: &Value, fallback_channel_id: &str| {
        let channel_id = entry.get("ch_id").and_then(value_to_string).unwrap_or_else(|| fallback_channel_id.to_string());
        if let Some(program) = map_stalker_epg_item(entry, &channel_id) {
            by_channel.entry(channel_id).or_default().push(program);
        }
    };

    match data {
        Value::Array(entries) => {
            for entry in entries {
                push_entry(&mut by_channel, entry, "");
            }
        }
        Value::Object(map) => {
            for (channel_id, value) in map {
                let entries = value
                    .as_array()
                    .cloned()
                    .or_else(|| value.get("data").and_then(|v| v.as_array()).cloned())
                    .or_else(|| value.get("epg").and_then(|v| v.as_array()).cloned())
                    .or_else(|| value.get("items").and_then(|v| v.as_array()).cloned())
                    .unwrap_or_default();
                for entry in &entries {
                    push_entry(&mut by_channel, entry, channel_id);
                }
            }
        }
        _ => {}
    }

    for programs in by_channel.values_mut() {
        programs.sort_by(|a, b| a.start.cmp(&b.start));
    }
    Ok(by_channel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_create_link_when_flags_set() {
        assert!(needs_create_link(Some("http://portal.example.com/x"), Some("1"), None));
        assert!(needs_create_link(Some("http://portal.example.com/x"), None, Some("1")));
    }

    #[test]
    fn needs_create_link_when_no_evidence_either_way() {
        assert!(needs_create_link(None, None, None));
    }

    #[test]
    fn skips_create_link_for_fully_qualified_remote_url() {
        assert!(!needs_create_link(Some("http://cdn.example.com/stream/1.ts"), None, None));
    }

    #[test]
    fn needs_create_link_for_portal_local_or_relative_cmd() {
        assert!(needs_create_link(Some("/media/live.php?x=1"), None, None));
        assert!(needs_create_link(Some("http://127.0.0.1/live.php"), None, None));
        assert!(needs_create_link(Some("ffrt4://some-token"), None, None));
    }

    #[test]
    fn normalizes_cmd_with_leading_solution_token() {
        assert_eq!(
            normalize_stalker_cmd("ffmpeg http://host/play/live.php?x=1"),
            "http://host/play/live.php?x=1"
        );
    }

    #[test]
    fn make_absolute_url_resolves_relative_screenshot_uri_against_origin() {
        assert_eq!(
            make_absolute_url("http://host.example.com:8080/stalker_portal/server/load.php", "misc/logo.png"),
            "http://host.example.com:8080/misc/logo.png"
        );
        assert_eq!(make_absolute_url("http://host.example.com", "http://cdn.example.com/x.png"), "http://cdn.example.com/x.png");
        assert_eq!(make_absolute_url("http://host.example.com", ""), "");
        // Unparseable base fails open to the original relative path unchanged.
        assert_eq!(make_absolute_url("not a url", "misc/logo.png"), "misc/logo.png");
    }

    #[test]
    fn maps_stalker_epg_item_using_plain_text_name_and_descr() {
        let item = serde_json::json!({
            "name": "Evening News",
            "descr": "Today's headlines",
            "time": "2025-01-15 14:00:00",
            "time_to": "2025-01-15 14:30:00",
            "start_timestamp": "1736949600",
            "stop_timestamp": "1736951400",
        });
        let program = map_stalker_epg_item(&item, "chan-1").unwrap();
        assert_eq!(program.channel_id, "chan-1");
        assert_eq!(program.title, "Evening News");
        assert_eq!(program.description.as_deref(), Some("Today's headlines"));
        assert!(program.start.starts_with("2025-01-15"));
    }

    #[test]
    fn stalker_epg_item_dropped_on_unparseable_date_with_no_timestamp() {
        let item = serde_json::json!({"name": "X", "time": "not a date", "time_to": "also not a date"});
        assert!(map_stalker_epg_item(&item, "chan-1").is_none());
    }

    #[test]
    fn stalker_channel_id_is_always_stashed_in_raw_json() {
        let row = serde_json::json!({"id": "45", "cmd": "http://host/live.php", "name": "Chan"});
        let category_names = HashMap::new();
        let channel = parse_channel_row(&row, &category_names, "http://host/portal.php");
        let raw: serde_json::Value = serde_json::from_str(&channel.raw.unwrap()).unwrap();
        assert_eq!(raw["stalker_channel_id"], "45");
    }
}
