use crate::db::DbPool;
use crate::state::ProxyState;
use axum::extract::{Query, State};
use axum::http::header::{ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, ETAG, LAST_MODIFIED, RANGE};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::body::Bytes;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures_util::stream::{self, Stream};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;

/// How long to wait for the next chunk before giving up on a silently-stalled
/// upstream. Live TS/HLS CDNs occasionally stall mid-stream without erroring
/// or closing the socket - `bytes_stream()` then never resolves again and
/// the player just freezes with no error to trigger a reconnect. This turns
/// a silent stall into a definite stream end so the player's retry logic fires.
const UPSTREAM_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// Binds `127.0.0.1:0` (OS-assigned free port) and spawns the proxy server,
/// returning the resolved port. Called synchronously via `block_on` in
/// `setup` so the server is already listening before the frontend can call
/// `get_stream_proxy_port` - no startup race.
pub async fn start(db: DbPool, http: Client) -> std::io::Result<u16> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let state = ProxyState { db, http, port, headers_cache: std::sync::Mutex::new(std::collections::HashMap::new()) };

    let app = Router::new()
        .route("/stream", get(stream_handler).head(stream_handler).options(stream_options))
        .layer(tower_http::catch_panic::CatchPanicLayer::new())
        .with_state(Arc::new(state));

    tauri::async_runtime::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("stream proxy server exited unexpectedly: {e}");
        }
    });

    Ok(port)
}

/// Raw HTTP query params — deliberately snake_case, not the camelCase Tauri
/// command DTOs use, since this is a plain loopback HTTP endpoint hit
/// directly via fetch/hls.js/mpegts.js, not IPC.
#[derive(Debug, Deserialize)]
struct StreamQuery {
    #[allow(dead_code)]
    playlist_id: String,
    url: String,
    /// Set by VOD/episode/catch-up callers only - opts a bounded, on-demand
    /// request back into `Content-Length` forwarding on a plain `200` below.
    #[serde(default)]
    vod: bool,
}

/// Every error exit must carry CORS headers too, not just success - a plain
/// response with no `Access-Control-Allow-Origin` makes the browser report a
/// confusing "CORS policy" error for what's actually a clean upstream
/// failure (connection refused, DNS failure, dead-end redirect), since
/// `fetch()` can't distinguish a CORS-less response from no response.
fn error_response(status: StatusCode, message: &'static str) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert("Access-Control-Allow-Origin", HeaderValue::from_static("*"));
    (status, headers, message).into_response()
}

async fn stream_options() -> impl IntoResponse {
    // `Range` is not a CORS-safelisted request header, so hls.js/mpegts.js
    // adding it on a seek triggers a real preflight `OPTIONS` — without this
    // handler, playback works until the first seek and then silently breaks.
    let mut headers = HeaderMap::new();
    headers.insert("Access-Control-Allow-Origin", HeaderValue::from_static("*"));
    headers.insert("Access-Control-Allow-Methods", HeaderValue::from_static("GET, HEAD, OPTIONS"));
    headers.insert("Access-Control-Allow-Headers", HeaderValue::from_static("Range"));
    (StatusCode::NO_CONTENT, headers)
}

async fn stream_handler(
    State(state): State<Arc<ProxyState>>,
    method: Method,
    Query(query): Query<StreamQuery>,
    headers: HeaderMap,
) -> Response {
    // Some providers embed a leading "solution token" (e.g. `ffmpeg
    // http://host/...`) in a stored URL - seen from Stalker's `create_link`
    // and some M3U exports too. Left in place, `reqwest` fails to parse it
    // as an absolute URL. Stripped here defensively too (also stripped at
    // M3U parse / `create_link` time) so rows imported before that fix still play.
    let target_url = crate::net::url_utils::strip_solution_token(&query.url);

    let range_header = headers.get(RANGE).cloned();
    tracing::debug!(
        "stream proxy: {} {} range={:?}",
        method,
        target_url,
        range_header.as_ref().and_then(|v| v.to_str().ok())
    );

    let auth_headers = resolve_auth_headers(&state, &query.playlist_id, &target_url).await;

    let mut upstream_req = state.http.request(method.clone(), &target_url);
    if let Some(range) = &range_header {
        upstream_req = upstream_req.header(RANGE, range.clone());
    }
    for (name, value) in &auth_headers {
        upstream_req = upstream_req.header(name, value.as_str());
    }

    let upstream_resp = match upstream_req.send().await {
        Ok(resp) => resp,
        Err(e) => {
            // `{e}` (Display) hides the real cause (DNS/TCP/TLS/redirect
            // failure) which only surfaces via `{:?}` (Debug)'s source chain.
            tracing::warn!("stream proxy: upstream request to {} failed: {e:?}", target_url);
            return error_response(StatusCode::BAD_GATEWAY, "Failed to reach the stream source");
        }
    };
    let status = StatusCode::from_u16(upstream_resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    tracing::debug!("stream proxy: upstream responded with status {}", upstream_resp.status());

    // reqwest and axum share the same `http` crate types, so headers copy
    // across directly.
    //
    // `Content-Length` is deliberately excluded except for genuine `206`
    // partial responses. A `200` from a live/segment endpoint can carry a
    // stale or partial `Content-Length` (a live feed has no real total) -
    // forwarding it makes `fetch()` (mpegts.js/hls.js's loader) treat the
    // body as bounded: once that many bytes arrive the Streams API reports
    // "done" even though the origin keeps sending more, which mpegts.js
    // reads as a false EOF rather than a real disconnect. `206` responses
    // keep an accurate length, so VOD seeking is unaffected.
    let mut response_headers = HeaderMap::new();
    for name in [CONTENT_TYPE, CONTENT_RANGE, ACCEPT_RANGES, LAST_MODIFIED, ETAG] {
        if let Some(value) = upstream_resp.headers().get(&name) {
            response_headers.insert(name, value.clone());
        }
    }
    // `vod=1` requests are never a live/segment feed, so a `200`'s length is
    // trustworthy there - forwarding it lets `<video>` learn the file has a
    // finite duration even when the provider never answers with a real `206`.
    let forward_200_length = query.vod && status == StatusCode::OK;
    if status == StatusCode::PARTIAL_CONTENT || forward_200_length {
        if let Some(value) = upstream_resp.headers().get(&CONTENT_LENGTH) {
            response_headers.insert(CONTENT_LENGTH, value.clone());
        }
    }
    response_headers.insert("Access-Control-Allow-Origin", HeaderValue::from_static("*"));
    response_headers.insert(
        "Access-Control-Expose-Headers",
        HeaderValue::from_static("Content-Range, Content-Length, Accept-Ranges"),
    );

    if method == Method::HEAD {
        return (status, response_headers).into_response();
    }

    // An HLS manifest's segment/sub-playlist URIs are almost always relative,
    // and hls.js resolves those against wherever it fetched the manifest
    // FROM - this proxy's own URL, not the real upstream host. Left
    // un-rewritten, every relative URI 404s (and a 404 has no CORS headers,
    // so the browser reports it as a CORS failure too). Only the manifest
    // text needs rewriting, and only via plain GET (never range-fetched).
    let content_type = upstream_resp.headers().get(&CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(str::to_string);
    if range_header.is_none() && looks_like_manifest(&target_url, content_type.as_deref()) {
        return handle_manifest_response(upstream_resp, status, response_headers, &target_url, &query.playlist_id, state.port).await;
    }

    // Stream the body through rather than buffering - required for live
    // streams and gives free backpressure on large VOD files. Idle-guarded
    // (see `UPSTREAM_IDLE_TIMEOUT`) so a silently-stalled upstream can't
    // hang the response forever.
    let body = axum::body::Body::from_stream(idle_guarded_stream(upstream_resp));

    (status, response_headers, body).into_response()
}

enum ProxyStreamState {
    Active(reqwest::Response),
    Done,
}

/// Wraps an upstream body so a chunk that never arrives ends the stream
/// instead of hanging - see [`UPSTREAM_IDLE_TIMEOUT`]. A timeout or read
/// error ends with `Err`, which axum surfaces as an aborted body that
/// mpegts.js/hls.js treat as a normal failure to retry; a clean `None`
/// (upstream closed normally) ends without error, a legitimate VOD EOF.
fn idle_guarded_stream(upstream: reqwest::Response) -> impl Stream<Item = Result<Bytes, std::io::Error>> {
    stream::unfold(ProxyStreamState::Active(upstream), |state| async move {
        let mut resp = match state {
            ProxyStreamState::Active(resp) => resp,
            ProxyStreamState::Done => return None,
        };

        match tokio::time::timeout(UPSTREAM_IDLE_TIMEOUT, resp.chunk()).await {
            Ok(Ok(Some(bytes))) => Some((Ok(bytes), ProxyStreamState::Active(resp))),
            Ok(Ok(None)) => None,
            Ok(Err(e)) => {
                tracing::warn!("stream proxy: upstream stream error: {e}");
                Some((Err(std::io::Error::other(e.to_string())), ProxyStreamState::Done))
            }
            Err(_) => {
                tracing::warn!("stream proxy: upstream idle for {UPSTREAM_IDLE_TIMEOUT:?}, closing stream");
                Some((
                    Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "upstream idle timeout")),
                    ProxyStreamState::Done,
                ))
            }
        }
    })
}

/// One substring check covers every real `.../mpegurl` content-type variant;
/// `.m3u8` extension is the fallback for a mislabeled header. Neither check
/// is trusted alone - `handle_manifest_response` verifies the body starts
/// with `#EXTM3U`, so a false positive here just costs one buffered response.
fn looks_like_manifest(url: &str, content_type: Option<&str>) -> bool {
    let ct_says_manifest = content_type.map(|ct| ct.to_ascii_lowercase().contains("mpegurl")).unwrap_or(false);
    let url_says_manifest = url.to_ascii_lowercase().contains(".m3u8");
    ct_says_manifest || url_says_manifest
}

async fn handle_manifest_response(
    upstream_resp: reqwest::Response,
    status: StatusCode,
    mut response_headers: HeaderMap,
    target_url: &str,
    playlist_id: &str,
    proxy_port: u16,
) -> Response {
    let bytes = match upstream_resp.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!("stream proxy: failed to read manifest body for {}: {e:?}", target_url);
            return error_response(StatusCode::BAD_GATEWAY, "Failed to read the stream source");
        }
    };

    let (Ok(text), Ok(base)) = (std::str::from_utf8(&bytes), reqwest::Url::parse(target_url)) else {
        return (status, response_headers, bytes).into_response();
    };
    if !text.trim_start().starts_with("#EXTM3U") {
        // The content-type or `.m3u8` extension said manifest, but the body
        // disagrees - serve what was actually fetched rather than guessing.
        return (status, response_headers, bytes).into_response();
    }

    let rewritten = rewrite_manifest(text, &base, proxy_port, playlist_id);
    response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/vnd.apple.mpegurl"));
    response_headers.remove(CONTENT_LENGTH); // stale: refers to the pre-rewrite byte length
    (status, response_headers, rewritten).into_response()
}

/// Rewrites every fetchable URI in an HLS playlist - segments, nested/variant
/// playlists, and `URI="..."` on `#EXT-X-KEY`/`#EXT-X-MAP` - into an
/// absolute `/stream?url=...` link, resolved against the manifest's own url
/// (`base`) so a relative path lands on the real upstream host.
fn rewrite_manifest(text: &str, base: &reqwest::Url, proxy_port: u16, playlist_id: &str) -> String {
    let mut out = String::with_capacity(text.len() + 256);
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(rewritten_tag) = rewrite_uri_attribute(line, base, proxy_port, playlist_id) {
            out.push_str(&rewritten_tag);
        } else if !line.is_empty() && !line.starts_with('#') {
            match base.join(line) {
                Ok(absolute) => out.push_str(&build_proxy_stream_url(proxy_port, playlist_id, absolute.as_str())),
                Err(_) => out.push_str(line),
            }
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Rewrites a `URI="..."` attribute in place on one `#EXT-X-KEY`/`#EXT-X-MAP`
/// line. Returns `None` for any other line, so the caller falls through to
/// its plain-URI-line handling.
fn rewrite_uri_attribute(line: &str, base: &reqwest::Url, proxy_port: u16, playlist_id: &str) -> Option<String> {
    if !line.starts_with("#EXT-X-KEY") && !line.starts_with("#EXT-X-MAP") {
        return None;
    }
    let value_start = line.find("URI=\"")? + 5;
    let value_end = value_start + line[value_start..].find('"')?;
    let absolute = base.join(&line[value_start..value_end]).ok()?;
    let proxied = build_proxy_stream_url(proxy_port, playlist_id, absolute.as_str());
    Some(format!("{}{proxied}{}", &line[..value_start], &line[value_end..]))
}

fn build_proxy_stream_url(proxy_port: u16, playlist_id: &str, absolute_url: &str) -> String {
    format!(
        "http://127.0.0.1:{proxy_port}/stream?playlist_id={}&url={}",
        utf8_percent_encode(playlist_id, NON_ALPHANUMERIC),
        utf8_percent_encode(absolute_url, NON_ALPHANUMERIC),
    )
}

/// Cache TTL for the DB-backed lookup below. Long enough to absorb a live
/// channel's manifest-refresh cadence and mpegts.js reconnects, short enough
/// that edited playlist/channel headers take effect within one window.
const HEADERS_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);

async fn resolve_auth_headers(state: &ProxyState, playlist_id: &str, target_url: &str) -> Vec<(String, String)> {
    let cache_key = (playlist_id.to_string(), target_url.to_string());
    if let Ok(cache) = state.headers_cache.lock() {
        if let Some((headers, cached_at)) = cache.get(&cache_key) {
            if cached_at.elapsed() < HEADERS_CACHE_TTL {
                return headers.clone();
            }
        }
    }

    let headers = resolve_auth_headers_uncached(state, playlist_id, target_url).await;

    if let Ok(mut cache) = state.headers_cache.lock() {
        // Opportunistic cleanup piggybacked on a write already holding the
        // lock, rather than a separate sweep task - many distinct segment
        // URLs (each a cache miss) would otherwise grow this map unboundedly.
        cache.retain(|_, (_, cached_at)| cached_at.elapsed() < HEADERS_CACHE_TTL);
        cache.insert(cache_key, (headers.clone(), std::time::Instant::now()));
    }

    headers
}

/// Stalker needs portal auth (MAC cookie/Bearer token) attached server-side;
/// M3U/Xtream need `User-Agent`/`Referer`/`Origin` attached server-side - the
/// browser's `fetch` can't set either on a cross-origin request.
///
/// `target_url` is compared against the portal's own host: same-origin gets
/// the full identity (cookie/SN/Bearer, MAG UA, portal Origin/Referer);
/// cross-origin (a third-party CDN redirect) gets a generic, credential-free
/// set instead, so portal session tokens never leak off-portal.
async fn resolve_auth_headers_uncached(state: &ProxyState, playlist_id: &str, target_url: &str) -> Vec<(String, String)> {
    let pool = state.db.clone();
    let playlist_id_owned = playlist_id.to_string();
    let target_url_owned = target_url.to_string();
    let (playlist, channel) = tauri::async_runtime::spawn_blocking(move || {
        let conn = pool.get().ok()?;
        let playlist = crate::db::playlists::get(&conn, &playlist_id_owned).ok().flatten()?;
        let channel = crate::db::channels::find_by_playlist_and_url(&conn, &playlist_id_owned, &target_url_owned)
            .ok()
            .flatten();
        Some((playlist, channel))
    })
    .await
    .ok()
    .flatten()
    .map_or((None, None), |(p, c)| (Some(p), c));

    let Some(playlist) = playlist else { return Vec::new() };

    if !matches!(playlist.playlist_type, crate::types::PlaylistType::Stalker) {
        // Matches iptvnator's `resolveExternalPlayerHttpHeaders()`: per-channel
        // `#EXTVLCOPT`/`#KODIPROP` wins, playlist-level fields fill the gap,
        // and a field empty on both sides is omitted rather than defaulted -
        // many providers 403 synthetic-looking headers, so no UA is safer
        // than a wrong one.
        fn first_non_empty(values: [Option<&str>; 2]) -> Option<String> {
            values.into_iter().flatten().map(str::trim).find(|s| !s.is_empty()).map(str::to_string)
        }
        let mut user_agent = first_non_empty([channel.as_ref().and_then(|c| c.http.user_agent.as_deref()), playlist.user_agent.as_deref()]);
        let mut referer = first_non_empty([channel.as_ref().and_then(|c| c.http.referrer.as_deref()), playlist.referrer.as_deref()]);
        let mut origin = first_non_empty([channel.as_ref().and_then(|c| c.http.origin.as_deref()), playlist.origin.as_deref()]);

        // A stale row from before `apply_xtream_default_headers` existed can
        // have empty user_agent/referrer/origin in the DB (that fix only
        // writes defaults at add/update/refresh time) - computed here too,
        // at read time, as the real single source of truth; the write-time
        // defaults remain a harmless, redundant convenience for Settings.
        //
        // M3U/M3U-Plus previously got no fallback at all (gated on Xtream
        // only, since M3U has no single "panel host"). Real gap: many
        // "M3U Plus" exports are Xtream panels under the hood and hit the
        // same hotlink-protection 403s. Xtream still prefers its configured
        // `server_url` first; everything else falls back to the stream's own
        // host, the only "origin" an M3U channel has.
        let origin_default = match playlist.playlist_type {
            crate::types::PlaylistType::Xtream => {
                playlist.server_url.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| s.trim_end_matches('/').to_string())
            }
            _ => None,
        }
        .or_else(|| {
            reqwest::Url::parse(target_url)
                .ok()
                .and_then(|u| u.host_str().map(|host| format!("{}://{}", u.scheme(), host)))
        });
        if let Some(origin_default) = origin_default {
            user_agent.get_or_insert_with(|| crate::net::xtream::XTREAM_USER_AGENT.to_string());
            referer.get_or_insert_with(|| origin_default.clone());
            origin.get_or_insert(origin_default);
        }

        let mut headers = Vec::new();
        if let Some(user_agent) = user_agent {
            headers.push(("User-Agent".to_string(), user_agent));
        }
        if let Some(referer) = referer {
            headers.push(("Referer".to_string(), referer));
        }
        if let Some(origin) = origin {
            headers.push(("Origin".to_string(), origin));
        }
        return headers;
    }
    let Some(mac) = playlist.mac_address.as_deref() else {
        return Vec::new();
    };
    // NOT `let Some(token) = ... else { return Vec::new() }` - a "not full"
    // Stalker portal (`is_full_stalker_portal() == false`) never gets a
    // `stalker_token` by design (expected, not an error - see
    // `commands/stalker.rs`'s `require_token`/`reauthenticate`). Requiring
    // both mac and token used to send stream requests completely bare, even
    // though `build_api_headers` already accepts `token: None` and still
    // sends the MAC-based `Cookie` many Stalker/Ministra CDNs authorize
    // streaming with. Was a real bug: such a playlist had every live request
    // rejected with a non-standard 458 (no identification sent at all).
    let token = playlist.stalker_token.as_deref();

    let portal_url = playlist.stalker_endpoint.as_deref().or(playlist.portal_url.as_deref());
    let portal_host = portal_url.and_then(|u| reqwest::Url::parse(u).ok()).and_then(|u| u.host_str().map(str::to_string));
    let target_host = reqwest::Url::parse(target_url).ok().and_then(|u| u.host_str().map(str::to_string));
    let same_origin = matches!((&portal_host, &target_host), (Some(a), Some(b)) if a.eq_ignore_ascii_case(b));

    if same_origin {
        let mut headers = crate::net::stalker::identity::build_api_headers(mac, playlist.stalker_serial_number.as_deref(), token);
        if let Some(parsed) = portal_url.and_then(|u| reqwest::Url::parse(u).ok()) {
            let origin = format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or(""));
            headers.push(("Origin".to_string(), origin.clone()));
            headers.push(("Referer".to_string(), origin));
        }
        headers
    } else {
        // Matches iptvnator's `buildStalkerExternalPlaybackHeaders()`
        // cross-origin branch: credential-free KSPlayer profile, no
        // `X-User-Agent` (would identify this as a MAG STB to a third-party
        // host) - see `net/stalker/identity.rs::build_api_headers` for why
        // `Connection: keep-alive` was restored after being wrongly removed.
        vec![
            ("User-Agent".to_string(), "KSPlayer".to_string()),
            ("Accept".to_string(), "*/*".to_string()),
            ("Connection".to_string(), "keep-alive".to_string()),
            ("Icy-MetaData".to_string(), "1".to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> reqwest::Url {
        reqwest::Url::parse("http://provider.example.com/live/user/pass/1943572.m3u8").unwrap()
    }

    #[test]
    fn rewrites_absolute_path_segment_uris_against_the_manifest_host() {
        let manifest = "#EXTM3U\n#EXTINF:10.0,\n/hls/abc123/1943572_0.ts\n";
        let out = rewrite_manifest(manifest, &base(), 5173, "pl1");
        let absolute = "http://provider.example.com/hls/abc123/1943572_0.ts";
        assert!(out.contains(&format!(
            "http://127.0.0.1:5173/stream?playlist_id=pl1&url={}",
            utf8_percent_encode(absolute, NON_ALPHANUMERIC)
        )));
        assert!(out.contains("#EXTINF:10.0,"));
    }

    #[test]
    fn rewrites_relative_segment_uris_against_the_manifest_directory() {
        let manifest = "#EXTM3U\n#EXTINF:10.0,\nseg_0.ts\n";
        let out = rewrite_manifest(manifest, &base(), 5173, "pl1");
        let absolute = "http://provider.example.com/live/user/pass/seg_0.ts";
        assert!(out.contains(&format!("url={}", utf8_percent_encode(absolute, NON_ALPHANUMERIC))));
    }

    #[test]
    fn leaves_already_absolute_uris_pointing_at_a_different_host_alone_but_still_proxied() {
        let manifest = "#EXTM3U\n#EXTINF:10.0,\nhttp://cdn.other.example.com/x/seg.ts\n";
        let out = rewrite_manifest(manifest, &base(), 5173, "pl1");
        let absolute = "http://cdn.other.example.com/x/seg.ts";
        assert!(out.contains(&format!("url={}", utf8_percent_encode(absolute, NON_ALPHANUMERIC))));
    }

    #[test]
    fn rewrites_variant_playlist_references() {
        let manifest = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=800000\nchunklist.m3u8\n";
        let out = rewrite_manifest(manifest, &base(), 5173, "pl1");
        let absolute = "http://provider.example.com/live/user/pass/chunklist.m3u8";
        assert!(out.contains(&format!("url={}", utf8_percent_encode(absolute, NON_ALPHANUMERIC))));
    }

    #[test]
    fn rewrites_key_uri_attribute_in_place_without_disturbing_the_rest_of_the_tag() {
        let manifest = "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\",IV=0x1234\n";
        let out = rewrite_manifest(manifest, &base(), 5173, "pl1");
        let absolute = "http://provider.example.com/live/user/pass/key.bin";
        assert!(out.starts_with(&format!(
            "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"http://127.0.0.1:5173/stream?playlist_id=pl1&url={}\",IV=0x1234\n",
            utf8_percent_encode(absolute, NON_ALPHANUMERIC)
        )));
    }

    #[test]
    fn leaves_comment_only_lines_and_blank_lines_untouched() {
        let manifest = "#EXTM3U\n#EXT-X-VERSION:3\n\n#EXT-X-ENDLIST\n";
        let out = rewrite_manifest(manifest, &base(), 5173, "pl1");
        assert_eq!(out, manifest);
    }

    // Regression: `vod=1` looked like a reasonable query value but silently
    // failed axum's `Query` extraction (`bool`'s `FromStr` only accepts
    // "true"/"false"), sending back a CORS-header-less 400 that broke every
    // VOD/episode/catch-up request. `wrapUrlThroughStreamProxy` sends
    // "true"/omits the param - never "1".
    #[test]
    fn vod_query_param_only_parses_as_true_or_false_not_one_or_zero() {
        let ok = serde_urlencoded::from_str::<StreamQuery>("playlist_id=p&url=u&vod=true");
        assert!(ok.is_ok());
        assert!(ok.unwrap().vod);

        let missing = serde_urlencoded::from_str::<StreamQuery>("playlist_id=p&url=u");
        assert!(missing.is_ok());
        assert!(!missing.unwrap().vod);

        let bad = serde_urlencoded::from_str::<StreamQuery>("playlist_id=p&url=u&vod=1");
        assert!(bad.is_err());
    }

    #[test]
    fn looks_like_manifest_matches_on_content_type_or_extension() {
        assert!(looks_like_manifest("http://x.com/a.m3u8", None));
        assert!(looks_like_manifest("http://x.com/a", Some("application/vnd.apple.mpegurl")));
        assert!(looks_like_manifest("http://x.com/a", Some("audio/x-mpegURL")));
        assert!(!looks_like_manifest("http://x.com/a.ts", Some("video/mp2t")));
    }
}
