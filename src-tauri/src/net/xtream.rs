use crate::error::{CommandError, CommandResult};
use crate::parsers::xmltv::ParsedEpgProgram;
use crate::types::{
    Channel, ChannelGroup, ChannelHttp, ChannelTvg, EpisodeInfo, SeasonEpisode, SeasonInfo,
    SeriesDetails, StreamType, VodDetails, XtreamCategory, XtreamServerInfo, XtreamStream,
    XtreamStreamType, XtreamUserInfo,
};
use base64::Engine;
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// Sent on every Xtream request — some panels reject unrecognized/generic
/// HTTP client User-Agents outright.
pub const XTREAM_USER_AGENT: &str = "VLC/3.0.18 LibVLC/3.0.18";

/// Xtream credentials are interpolated into a URL *path* segment
/// (`/live/{user}/{pass}/{id}.{ext}`), not a query string - characters like
/// `/`, `?`, `#`, `@` would otherwise corrupt the path or throw off the
/// frontend's `new URL(...)`-based extension detection.
const PATH_SEGMENT_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'`')
    .add(b'#')
    .add(b'?')
    .add(b'{')
    .add(b'}')
    .add(b'/')
    .add(b'%')
    .add(b'\\')
    .add(b'^')
    .add(b'|')
    .add(b'@')
    .add(b':');

fn encode_path_segment(value: &str) -> String {
    utf8_percent_encode(value, PATH_SEGMENT_ENCODE_SET).to_string()
}

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

fn value_to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Strips a trailing `/player_api.php` or `/get.php` and validates scheme —
/// the frontend lets users paste either the bare panel URL or a full
/// `player_api.php` URL copied from another client's config.
pub fn normalize_server_url(raw: &str) -> CommandResult<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(CommandError::Api("Server URL is required".into()));
    }
    let mut url = trimmed.to_string();
    for suffix in ["/player_api.php", "/get.php"] {
        // Case-insensitive: a pasted `.../Player_API.PHP` (copied from
        // another client's config, or a URL a mobile browser autocapitalized)
        // must strip the same way a lowercase one does.
        if url.len() >= suffix.len() {
            let tail_start = url.len() - suffix.len();
            if url.is_char_boundary(tail_start) && url[tail_start..].eq_ignore_ascii_case(suffix) {
                url.truncate(tail_start);
            }
        }
    }
    let url = url.trim_end_matches('/').to_string();

    let parsed = reqwest::Url::parse(&url).map_err(|_| CommandError::Api("Invalid server URL".into()))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(CommandError::Api("Server URL must use http or https".into()));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(CommandError::Api(
            "Server URL must not contain embedded credentials".into(),
        ));
    }

    Ok(url)
}

async fn player_api_request(
    http: &Client,
    server_url: &str,
    username: &str,
    password: &str,
    action: Option<&str>,
    extra: &[(&str, &str)],
) -> CommandResult<Value> {
    // Real-world credentials are sometimes copy-pasted with stray
    // leading/trailing whitespace — trimmed here, once, so every Xtream
    // action (auth, categories, streams, vod/series info) benefits, not
    // just the initial login check.
    let username = username.trim();
    let password = password.trim();
    let mut params: Vec<(&str, &str)> = vec![("username", username), ("password", password)];
    if let Some(action) = action {
        params.push(("action", action));
    }
    params.extend_from_slice(extra);

    let url = format!("{server_url}/player_api.php");
    let response = http
        .get(&url)
        .query(&params)
        .header(reqwest::header::USER_AGENT, XTREAM_USER_AGENT)
        .header(reqwest::header::ACCEPT, "application/json")
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| {
            tracing::warn!("Xtream request to {url} failed: {e}");
            CommandError::Api(
                "Couldn't reach your IPTV provider's server. Check the server address and your internet connection, then try again.".into(),
            )
        })?;

    let status = response.status();
    // 4xx/5xx is treated as an error before the JSON parser sees it - some
    // panels' error bodies still parse as valid (but empty) JSON, which
    // would otherwise silently produce empty categories/streams instead of
    // a clear error.
    if status.as_u16() >= 400 {
        tracing::warn!("Xtream server at {url} returned status {}", status.as_u16());
        return Err(CommandError::Api(format!(
            "Your IPTV provider's server returned an error (code {}). This usually means it's temporarily down or overloaded — try again shortly, or contact your provider if it keeps happening.",
            status.as_u16()
        )));
    }

    response.json::<Value>().await.map_err(|e| {
        tracing::warn!("Xtream server at {url} returned unparseable JSON: {e}");
        CommandError::InvalidResponse(
            "Your IPTV provider's server sent back a response we couldn't understand. It may be temporarily misconfigured or under maintenance.".into(),
        )
    })
}

/// Counts as "account info" if it has a nested `user_info` object, OR (some
/// panels return fields flat) the body itself carries a field only an
/// account-info response would have.
fn has_account_info_payload(body: &Value) -> bool {
    body.get("user_info").is_some() || body.get("auth").is_some() || body.get("status").is_some()
}

/// Real panels differ in which request shape they expect for account status.
/// Tries `get_account_info`, then a blank action (the original sole
/// attempt), then `get_profile`, stopping at the first response that
/// actually looks like account info. An error on any but the last attempt
/// just moves to the next shape; the last attempt's error (or a generic auth
/// failure if nothing ever looked account-info-shaped) is what's surfaced.
pub async fn get_account_info(
    http: &Client,
    server_url: &str,
    username: &str,
    password: &str,
) -> CommandResult<XtreamUserInfo> {
    const ACTIONS: [Option<&str>; 3] = [Some("get_account_info"), None, Some("get_profile")];
    let mut last_err = None;
    for (i, action) in ACTIONS.iter().enumerate() {
        let is_last = i == ACTIONS.len() - 1;
        match player_api_request(http, server_url, username, password, *action, &[]).await {
            Ok(body) if has_account_info_payload(&body) => return parse_user_info(&body),
            Ok(_) if is_last => break,
            Ok(_) => continue,
            Err(e) if is_last => return Err(e),
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        CommandError::Auth("Invalid username/password, or the account is inactive".into())
    }))
}

fn parse_user_info(body: &Value) -> CommandResult<XtreamUserInfo> {
    let user_info = body.get("user_info").unwrap_or(body);
    // Some panels omit `auth` and rely on `status` text instead - only an
    // explicit `auth: 0` is a hard rejection; a missing field isn't assumed
    // failure.
    let auth_field = user_info.get("auth").and_then(value_to_i64);
    let auth = auth_field.unwrap_or(0);
    let status = user_info
        .get("status")
        .and_then(value_to_string)
        .unwrap_or_else(|| "unknown".to_string());

    if auth_field == Some(0) {
        return Err(CommandError::Auth(
            "Invalid username/password, or the account is inactive".into(),
        ));
    }
    // An expired subscription still authenticates on a real panel - the
    // user should still see the playlist and why it's not working, rather
    // than being locked out. Only an explicit ban/disable blocks it.
    if matches!(status.to_ascii_lowercase().as_str(), "banned" | "disabled") {
        return Err(CommandError::Auth(format!("Account is {status}")));
    }

    let server_info = body.get("server_info").map(|s| XtreamServerInfo {
        url: s.get("url").and_then(value_to_string),
        port: s.get("port").and_then(value_to_string),
        https_port: s.get("https_port").and_then(value_to_string),
        rtmp_port: s.get("rtmp_port").and_then(value_to_string),
        server_protocol: s.get("server_protocol").and_then(value_to_string),
        timezone: s.get("timezone").and_then(value_to_string),
        timestamp_now: s.get("timestamp_now").and_then(value_to_i64),
        time_now: s.get("time_now").and_then(value_to_string),
    });

    Ok(XtreamUserInfo {
        username: user_info.get("username").and_then(value_to_string).unwrap_or_default(),
        password: user_info.get("password").and_then(value_to_string).unwrap_or_default(),
        message: user_info.get("message").and_then(value_to_string),
        auth,
        status,
        exp_date: user_info.get("exp_date").and_then(value_to_string),
        max_connections: user_info.get("max_connections").and_then(value_to_string),
        allowed_output_formats: user_info
            .get("allowed_output_formats")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(value_to_string).collect()),
        is_trial: user_info.get("is_trial").and_then(value_to_i64).map(|n| n == 1),
        active_cons: user_info.get("active_cons").and_then(value_to_string),
        created_at: user_info.get("created_at").and_then(value_to_string),
        server_info,
    })
}

/// Prefers `ts` (plain MPEG-TS) over `m3u8`. `.m3u8` used to be preferred,
/// but real-world testing showed a stream's `.m3u8` manifest can load fine
/// while every referenced `.ts` segment 403s, while the same stream's plain
/// `.ts` URL plays fine - and `.ts` is the traditional Xtream Codes live
/// default anyway. Still respects what the provider actually advertises
/// (`allowed_output_formats`); a panel only listing `m3u8` still gets `m3u8`.
pub fn preferred_live_format(allowed: Option<&[String]>) -> String {
    if let Some(list) = allowed {
        if list.iter().any(|f| f.eq_ignore_ascii_case("ts")) {
            return "ts".to_string();
        }
        if list.iter().any(|f| f.eq_ignore_ascii_case("m3u8")) {
            return "m3u8".to_string();
        }
    }
    "ts".to_string()
}

fn category_action(stream_type: &str) -> &'static str {
    match stream_type {
        // Frontend sends "vod" (see `vod.svelte.ts`), not "movie" - "movie"
        // accepted defensively too, in case a future caller uses it.
        "vod" | "movie" => "get_vod_categories",
        "series" => "get_series_categories",
        _ => "get_live_categories",
    }
}

fn streams_action(stream_type: &str) -> &'static str {
    match stream_type {
        "vod" | "movie" => "get_vod_streams",
        "series" => "get_series",
        _ => "get_live_streams",
    }
}

pub async fn get_categories(
    http: &Client,
    server_url: &str,
    username: &str,
    password: &str,
    stream_type: &str,
) -> CommandResult<Vec<XtreamCategory>> {
    let action = category_action(stream_type);
    let body = player_api_request(http, server_url, username, password, Some(action), &[]).await?;
    let arr = body.as_array().cloned().unwrap_or_default();
    Ok(arr.iter().filter_map(parse_category).collect())
}

fn parse_category(v: &Value) -> Option<XtreamCategory> {
    Some(XtreamCategory {
        id: v.get("category_id").and_then(value_to_i64),
        category_id: v.get("category_id").and_then(value_to_string)?,
        category_name: v.get("category_name").and_then(value_to_string).unwrap_or_default(),
        parent_id: v.get("parent_id").and_then(value_to_i64).unwrap_or(0),
        count: None,
    })
}

pub async fn get_streams(
    http: &Client,
    server_url: &str,
    username: &str,
    password: &str,
    stream_type: &str,
    category_id: Option<&str>,
) -> CommandResult<Vec<XtreamStream>> {
    let action = streams_action(stream_type);
    let mut extra = Vec::new();
    if let Some(cat) = category_id {
        extra.push(("category_id", cat));
    }
    let body = player_api_request(http, server_url, username, password, Some(action), &extra).await?;
    let arr = body.as_array().cloned().unwrap_or_default();

    let xtream_stream_type = match stream_type {
        "vod" | "movie" => XtreamStreamType::Movie,
        "series" => XtreamStreamType::Series,
        "radio" => XtreamStreamType::Radio,
        _ => XtreamStreamType::Live,
    };
    Ok(arr.iter().filter_map(|v| parse_stream(v, xtream_stream_type)).collect())
}

fn parse_stream(v: &Value, stream_type: XtreamStreamType) -> Option<XtreamStream> {
    // `get_series` rows use `series_id` instead of `stream_id`.
    let stream_id = v
        .get("stream_id")
        .and_then(value_to_i64)
        .or_else(|| v.get("series_id").and_then(value_to_i64))?;

    Some(XtreamStream {
        num: v.get("num").and_then(value_to_i64).unwrap_or(0),
        name: v.get("name").and_then(value_to_string).unwrap_or_default(),
        stream_type,
        stream_id,
        stream_icon: v
            .get("stream_icon")
            .or_else(|| v.get("cover"))
            .and_then(value_to_string)
            .unwrap_or_default(),
        added: v.get("added").and_then(value_to_string).unwrap_or_default(),
        category_id: v.get("category_id").and_then(value_to_string).unwrap_or_default(),
        custom_sid: v.get("custom_sid").and_then(value_to_string).unwrap_or_default(),
        direct_source: v.get("direct_source").and_then(value_to_string).unwrap_or_default(),
        epg_channel_id: v.get("epg_channel_id").and_then(value_to_string),
        tv_archive: v.get("tv_archive").and_then(value_to_i64),
        tv_archive_duration: v.get("tv_archive_duration").and_then(value_to_i64),
        rating_imdb: v.get("rating_imdb").and_then(value_to_string),
        xtream_id: Some(stream_id),
        r#type: None,
        added_at: None,
        container_extension: v.get("container_extension").and_then(value_to_string),
        rating: v.get("rating").and_then(value_to_string),
        year: v.get("year").and_then(value_to_string),
        cover: v.get("cover").and_then(value_to_string),
        genre: v.get("genre").and_then(value_to_string),
        release_date: v
            .get("release_date")
            .or_else(|| v.get("releasedate"))
            .and_then(value_to_string),
        stream_url: None,
        series_id: v.get("series_id").and_then(value_to_i64),
        is_series: None,
    })
}

pub fn live_stream_url(server_url: &str, username: &str, password: &str, stream_id: i64, format: &str) -> String {
    let user = encode_path_segment(username);
    let pass = encode_path_segment(password);
    format!("{server_url}/live/{user}/{pass}/{stream_id}.{format}")
}

pub fn vod_stream_url(server_url: &str, username: &str, password: &str, stream_id: i64, extension: &str) -> String {
    let user = encode_path_segment(username);
    let pass = encode_path_segment(password);
    format!("{server_url}/movie/{user}/{pass}/{stream_id}.{extension}")
}

pub fn episode_stream_url(server_url: &str, username: &str, password: &str, episode_id: i64, extension: &str) -> String {
    let user = encode_path_segment(username);
    let pass = encode_path_segment(password);
    format!("{server_url}/series/{user}/{pass}/{episode_id}.{extension}")
}

/// Maps one Xtream live stream row into the unified `Channel` shape shared
/// across M3U/Xtream/Stalker — the frontend never distinguishes source for
/// Live TV, so every playlist type's live channels land in the same table.
pub fn stream_to_channel(
    stream: &XtreamStream,
    category_name: Option<&str>,
    server_url: &str,
    username: &str,
    password: &str,
    format: &str,
) -> Channel {
    Channel {
        id: uuid::Uuid::new_v4().to_string(),
        url: live_stream_url(server_url, username, password, stream.stream_id, format),
        name: stream.name.clone(),
        group: ChannelGroup {
            title: category_name.unwrap_or_default().to_string(),
        },
        tvg: ChannelTvg {
            id: stream.epg_channel_id.clone(),
            name: Some(stream.name.clone()),
            url: None,
            logo: if stream.stream_icon.is_empty() {
                None
            } else {
                Some(stream.stream_icon.clone())
            },
            rec: None,
        },
        epg_params: None,
        timeshift: None,
        // Xtream catch-up is a different URL scheme (resolved via
        // `xtream_resolve_catchup_url`) from M3U's attribute-driven one
        // (`catchup::resolve_m3u_catchup_url`), so `catchup`/`timeshift`
        // are deliberately left unset here - that would make the M3U
        // resolver build a wrong URL. `stream_id`/`tv_archive*` are stashed
        // in `raw` instead so the frontend can ask "does this support
        // catch-up" without a schema change.
        catchup: None,
        http: ChannelHttp::default(),
        radio: "0".to_string(),
        drm: None,
        raw: Some(
            serde_json::json!({
                "xtream_stream_id": stream.stream_id,
                "tv_archive": stream.tv_archive,
                "tv_archive_duration": stream.tv_archive_duration,
            })
            .to_string(),
        ),
        channel_number: Some(stream.num),
    }
}

pub async fn get_vod_info(
    http: &Client,
    server_url: &str,
    username: &str,
    password: &str,
    vod_id: &str,
) -> CommandResult<VodDetails> {
    let body = player_api_request(http, server_url, username, password, Some("get_vod_info"), &[("vod_id", vod_id)])
        .await?;
    parse_vod_info(&body, server_url, username, password, vod_id)
}

/// Diagnostic-only: logs whatever codec info the panel's `info.video`/
/// `info.audio` (ffprobe-shaped, present on many but not all panels) reports
/// for a title. Not parsed into `VodDetails`/`SeasonEpisode` - exists to
/// confirm empirically what MKV/AVI VOD titles are encoded as (suspected
/// HEVC + AC3/EAC3/DTS, which Chromium's native `<video>` can't play).
fn log_codec_info(kind: &str, id: &str, container_extension: Option<&str>, info: Option<&Value>) {
    let Some(info) = info else { return };
    let video_codec = info.get("video").and_then(|v| v.get("codec_name")).and_then(value_to_string);
    let audio_codec = info.get("audio").and_then(|v| v.get("codec_name")).and_then(value_to_string);
    if video_codec.is_none() && audio_codec.is_none() {
        return;
    }
    tracing::info!(
        "xtream {kind} {id}: container={:?} video_codec={:?} audio_codec={:?}",
        container_extension,
        video_codec,
        audio_codec
    );
}

fn parse_vod_info(
    body: &Value,
    server_url: &str,
    username: &str,
    password: &str,
    vod_id: &str,
) -> CommandResult<VodDetails> {
    let empty = Value::Null;
    let info = body.get("info").unwrap_or(&empty);
    let movie_data = body.get("movie_data").unwrap_or(&empty);

    // Missing `container_extension` means the provider hasn't told us
    // what's playable - guessing `.mp4` produced URLs that looked valid but
    // often failed. No extension means no stream URL, not a guess.
    let container_extension = movie_data.get("container_extension").and_then(value_to_string);
    log_codec_info("movie", vod_id, container_extension.as_deref(), Some(info));
    let stream_id = movie_data
        .get("stream_id")
        .and_then(value_to_i64)
        .unwrap_or_else(|| vod_id.parse().unwrap_or(0));
    let stream_url = container_extension
        .as_deref()
        .map(|ext| vod_stream_url(server_url, username, password, stream_id, ext));

    Ok(VodDetails {
        id: vod_id.to_string(),
        name: info
            .get("name")
            .or_else(|| info.get("o_name"))
            .and_then(value_to_string)
            .unwrap_or_default(),
        stream_type: StreamType::Movie,
        container_extension,
        direct_source: None,
        series_id: None,
        season_number: None,
        episode_number: None,
        cover: info
            .get("movie_image")
            .or_else(|| info.get("cover_big"))
            .and_then(value_to_string),
        stream_url,
        plot: info.get("plot").and_then(value_to_string),
        cast: info.get("cast").and_then(value_to_string),
        rating: info.get("rating").and_then(value_to_string),
        genre: info.get("genre").and_then(value_to_string),
        release_date: info
            .get("releasedate")
            .or_else(|| info.get("release_date"))
            .and_then(value_to_string),
        tmdb_id: info.get("tmdb_id").and_then(value_to_i64),
        seasons: None,
        episodes: None,
        cmd: None,
        use_http_tmp_link: None,
        use_load_balancing: None,
        stalker_content_type: None,
    })
}

pub async fn get_series_info(
    http: &Client,
    server_url: &str,
    username: &str,
    password: &str,
    series_id: &str,
) -> CommandResult<SeriesDetails> {
    let body = player_api_request(
        http,
        server_url,
        username,
        password,
        Some("get_series_info"),
        &[("series_id", series_id)],
    )
    .await?;
    parse_series_info(&body, server_url, username, password, series_id)
}

fn parse_series_info(
    body: &Value,
    server_url: &str,
    username: &str,
    password: &str,
    series_id: &str,
) -> CommandResult<SeriesDetails> {
    let empty = Value::Null;
    let info = body.get("info").unwrap_or(&empty);
    let seasons_val = body.get("seasons").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let episodes_val = body.get("episodes").unwrap_or(&empty);

    let seasons: Vec<SeasonInfo> = seasons_val
        .iter()
        .map(|s| SeasonInfo {
            id: s.get("id").and_then(value_to_i64),
            name: s.get("name").and_then(value_to_string).unwrap_or_default(),
            season_number: s.get("season_number").and_then(value_to_i64).unwrap_or(0),
            episode_count: s.get("episode_count").and_then(value_to_i64),
            air_date: s.get("air_date").and_then(value_to_string),
            cover: s.get("cover").or_else(|| s.get("cover_big")).and_then(value_to_string),
        })
        .collect();

    let mut episodes: HashMap<String, Vec<SeasonEpisode>> = HashMap::new();
    if let Value::Object(map) = episodes_val {
        for (season_key, list) in map {
            let Some(arr) = list.as_array() else { continue };
            let parsed: Vec<SeasonEpisode> = arr
                .iter()
                .map(|e| {
                    let episode_id = e.get("id").and_then(value_to_i64).unwrap_or(0);
                    let container_extension = e.get("container_extension").and_then(value_to_string);
                    let season_num = e
                        .get("season")
                        .and_then(value_to_i64)
                        .unwrap_or_else(|| season_key.parse().unwrap_or(0));
                    let stream_url = container_extension
                        .as_deref()
                        .map(|ext| episode_stream_url(server_url, username, password, episode_id, ext));
                    let info_obj = e.get("info");
                    log_codec_info("episode", &episode_id.to_string(), container_extension.as_deref(), info_obj);

                    SeasonEpisode {
                        id: episode_id.to_string(),
                        episode_num: e.get("episode_num").and_then(value_to_i64),
                        title: e.get("title").and_then(value_to_string).unwrap_or_default(),
                        season: season_num,
                        container_extension,
                        info: info_obj.map(|info| EpisodeInfo {
                            duration_secs: info.get("duration_secs").and_then(value_to_i64),
                            rating: info.get("rating").and_then(value_to_f64),
                        }),
                        cover: info_obj.and_then(|i| i.get("movie_image")).and_then(value_to_string),
                        plot: info_obj.and_then(|i| i.get("plot")).and_then(value_to_string),
                        stream_url,
                        direct_source: None,
                        cmd: None,
                        series_param: None,
                    }
                })
                .collect();
            episodes.insert(season_key.clone(), parsed);
        }
    }

    Ok(SeriesDetails {
        info: VodDetails {
            id: series_id.to_string(),
            name: info.get("name").and_then(value_to_string).unwrap_or_default(),
            stream_type: StreamType::Series,
            container_extension: None,
            direct_source: None,
            series_id: series_id.parse().ok(),
            season_number: None,
            episode_number: None,
            cover: info.get("cover").or_else(|| info.get("cover_big")).and_then(value_to_string),
            stream_url: None,
            plot: info.get("plot").and_then(value_to_string),
            cast: info.get("cast").and_then(value_to_string),
            rating: info.get("rating").and_then(value_to_string),
            genre: info.get("genre").and_then(value_to_string),
            release_date: info
                .get("releaseDate")
                .or_else(|| info.get("release_date"))
                .and_then(value_to_string),
            tmdb_id: None,
            seasons: Some(seasons.clone()),
            episodes: None,
            cmd: None,
            use_http_tmp_link: None,
            use_load_balancing: None,
            stalker_content_type: None,
        },
        seasons,
        episodes,
    })
}

// ---------------------------------------------------------------------
// Native EPG (get_short_epg / get_simple_data_table / get_simple_date_table)
// ---------------------------------------------------------------------

/// Real Xtream panels base64-encode `title`/`description` on EPG listings.
/// On decode failure (not actually base64), the raw string is returned
/// unchanged rather than dropped, so non-conformant panels degrade gracefully.
fn decode_base64_unicode(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(trimmed) {
        if let Ok(text) = String::from_utf8(bytes) {
            return text;
        }
    }
    if let Ok(bytes) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(trimmed) {
        if let Ok(text) = String::from_utf8(bytes) {
            return text;
        }
    }
    trimmed.to_string()
}

/// `response.epg_listings` can be an array OR a map keyed by an arbitrary
/// id — matches iptvnator's `getEpgListings` (`Object.values()` on the map
/// case).
fn epg_listings(body: &Value) -> Vec<Value> {
    match body.get("epg_listings") {
        Some(Value::Array(arr)) => arr.clone(),
        Some(Value::Object(map)) => map.values().cloned().collect(),
        _ => Vec::new(),
    }
}

/// A unix-seconds timestamp field, only if present and strictly positive —
/// matches `parseUnixTimestamp`.
fn epg_unix_timestamp(item: &Value, key: &str) -> Option<i64> {
    item.get(key).and_then(value_to_i64).filter(|&n| n > 0)
}

fn epg_iso_from_timestamp(ts: Option<i64>) -> Option<String> {
    ts.and_then(|t| chrono::DateTime::from_timestamp(t, 0)).map(|dt| dt.to_rfc3339())
}

/// Matches `normalizeDateString`: `"YYYY-MM-DD HH:MM:SS"` (space-separated,
/// treated as UTC) parses to a proper ISO string; anything unparseable
/// passes through unchanged rather than being dropped.
fn epg_normalize_date_string(raw: Option<&str>) -> String {
    let raw = raw.unwrap_or("").trim();
    if raw.is_empty() {
        return String::new();
    }
    let candidate = raw.replacen(' ', "T", 1);
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&candidate) {
        return dt.to_rfc3339();
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&candidate, "%Y-%m-%dT%H:%M:%S") {
        return dt.and_utc().to_rfc3339();
    }
    raw.to_string()
}

/// Maps one raw EPG listing into StreamFlow's unified EPG shape, keyed by
/// `channel_key` (the unified `Channel`'s id/tvg-id, not the provider's own
/// `channel_id`/`epg_id` - see `xtream_sync_epg`). Unix timestamp wins over
/// formatted date string for start/stop; an item missing either is dropped.
fn map_xtream_epg_item(item: &Value, channel_key: &str) -> Option<ParsedEpgProgram> {
    let start = epg_iso_from_timestamp(epg_unix_timestamp(item, "start_timestamp"))
        .unwrap_or_else(|| epg_normalize_date_string(item.get("start").and_then(|v| v.as_str())));
    let stop = epg_iso_from_timestamp(epg_unix_timestamp(item, "stop_timestamp")).unwrap_or_else(|| {
        let raw = item.get("stop").and_then(|v| v.as_str()).or_else(|| item.get("end").and_then(|v| v.as_str()));
        epg_normalize_date_string(raw)
    });
    if start.is_empty() || stop.is_empty() {
        return None;
    }

    let title = decode_base64_unicode(item.get("title").and_then(|v| v.as_str()).unwrap_or(""));
    let description = decode_base64_unicode(item.get("description").and_then(|v| v.as_str()).unwrap_or(""));

    Some(ParsedEpgProgram {
        channel_id: channel_key.to_string(),
        start,
        stop,
        title,
        description: if description.is_empty() { None } else { Some(description) },
        category: None,
        icon_url: None,
    })
}

fn sort_epg_programs(mut programs: Vec<ParsedEpgProgram>) -> Vec<ParsedEpgProgram> {
    // `start` is either a proper RFC3339 UTC string or (rarely) a raw
    // passthrough - lexicographic ordering of RFC3339 is chronologically
    // correct for the overwhelming majority of real listings.
    programs.sort_by(|a, b| a.start.cmp(&b.start));
    programs
}

/// `get_short_epg` - a short rolling window of upcoming programs for one
/// stream. `limit` defaults to 10 at the call site, independent of whatever
/// default the panel applies server-side.
pub async fn get_short_epg(
    http: &Client,
    server_url: &str,
    username: &str,
    password: &str,
    stream_id: i64,
    limit: i64,
    channel_key: &str,
) -> CommandResult<Vec<ParsedEpgProgram>> {
    let stream_id_str = stream_id.to_string();
    let limit_str = limit.to_string();
    let body = player_api_request(
        http,
        server_url,
        username,
        password,
        Some("get_short_epg"),
        &[("stream_id", &stream_id_str), ("limit", &limit_str)],
    )
    .await?;
    Ok(sort_epg_programs(epg_listings(&body).iter().filter_map(|v| map_xtream_epg_item(v, channel_key)).collect()))
}

/// The full multi-day EPG for one stream. Tries `get_simple_data_table`
/// first, falling back to the legacy-typo'd `get_simple_date_table` when the
/// first attempt fails or returns zero listings - some panels only
/// implement one spelling.
pub async fn get_full_epg(
    http: &Client,
    server_url: &str,
    username: &str,
    password: &str,
    stream_id: i64,
    channel_key: &str,
) -> CommandResult<Vec<ParsedEpgProgram>> {
    let stream_id_str = stream_id.to_string();
    if let Ok(body) = player_api_request(http, server_url, username, password, Some("get_simple_data_table"), &[("stream_id", &stream_id_str)]).await
    {
        let programs = sort_epg_programs(epg_listings(&body).iter().filter_map(|v| map_xtream_epg_item(v, channel_key)).collect());
        if !programs.is_empty() {
            return Ok(programs);
        }
    }
    let fallback = player_api_request(http, server_url, username, password, Some("get_simple_date_table"), &[("stream_id", &stream_id_str)]).await?;
    Ok(sort_epg_programs(epg_listings(&fallback).iter().filter_map(|v| map_xtream_epg_item(v, channel_key)).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_player_api_suffix_case_insensitively() {
        assert_eq!(
            normalize_server_url("http://panel.example.com:8080/Player_API.PHP").unwrap(),
            "http://panel.example.com:8080"
        );
        assert_eq!(
            normalize_server_url("http://panel.example.com:8080/GET.PHP").unwrap(),
            "http://panel.example.com:8080"
        );
    }

    #[test]
    fn decodes_base64_and_falls_back_to_raw_on_failure() {
        assert_eq!(decode_base64_unicode("SGVsbG8="), "Hello");
        assert_eq!(decode_base64_unicode("not base64 at all!!"), "not base64 at all!!");
    }

    #[test]
    fn epg_listings_handles_array_and_map_shapes() {
        let arr_body = serde_json::json!({"epg_listings": [{"id": "1"}, {"id": "2"}]});
        assert_eq!(epg_listings(&arr_body).len(), 2);
        let map_body = serde_json::json!({"epg_listings": {"a": {"id": "1"}, "b": {"id": "2"}}});
        assert_eq!(epg_listings(&map_body).len(), 2);
    }

    #[test]
    fn maps_epg_item_preferring_unix_timestamp_over_date_string() {
        let item = serde_json::json!({
            "title": "SGVsbG8=",
            "description": "V29ybGQ=",
            "start": "2000-01-01 00:00:00",
            "end": "2000-01-01 01:00:00",
            "start_timestamp": "1704110400",
            "stop_timestamp": "1704112200",
        });
        let program = map_xtream_epg_item(&item, "chan-1").unwrap();
        assert_eq!(program.channel_id, "chan-1");
        assert_eq!(program.title, "Hello");
        assert_eq!(program.description.as_deref(), Some("World"));
        assert!(program.start.starts_with("2024-01-01"));
        assert_ne!(program.start, "2000-01-01T00:00:00+00:00");
    }

    #[test]
    fn drops_epg_item_with_no_usable_start_or_stop() {
        let item = serde_json::json!({"title": "x"});
        assert!(map_xtream_epg_item(&item, "chan-1").is_none());
    }

    #[test]
    fn has_account_info_payload_detects_wrapped_and_flat_shapes() {
        assert!(has_account_info_payload(&serde_json::json!({"user_info": {"auth": 1}})));
        assert!(has_account_info_payload(&serde_json::json!({"auth": 1})));
        assert!(!has_account_info_payload(&serde_json::json!({"some_other_field": 1})));
    }

    #[test]
    fn normalizes_bare_and_player_api_urls() {
        assert_eq!(normalize_server_url("http://panel.example.com:8080").unwrap(), "http://panel.example.com:8080");
        assert_eq!(
            normalize_server_url("http://panel.example.com:8080/player_api.php").unwrap(),
            "http://panel.example.com:8080"
        );
        assert_eq!(
            normalize_server_url("http://panel.example.com:8080/get.php/").unwrap(),
            "http://panel.example.com:8080"
        );
    }

    #[test]
    fn rejects_embedded_credentials() {
        assert!(normalize_server_url("http://user:pass@panel.example.com").is_err());
    }

    #[test]
    fn rejects_non_http_scheme() {
        assert!(normalize_server_url("ftp://panel.example.com").is_err());
    }

    #[test]
    fn maps_vod_stream_type_to_movie_actions() {
        // Regression: frontend sends "vod" for movies, not "movie" - an
        // earlier match only recognized "movie" and fell through to live TV.
        assert_eq!(category_action("vod"), "get_vod_categories");
        assert_eq!(streams_action("vod"), "get_vod_streams");
        assert_eq!(category_action("series"), "get_series_categories");
        assert_eq!(streams_action("series"), "get_series");
    }

    #[test]
    fn encodes_special_characters_in_credentials_for_stream_urls() {
        // Credentials can contain URL-meaningful chars (`/`, `?`, `#`, `@`,
        // `:`) - an unescaped `#` alone would truncate the path as a fragment.
        let url = live_stream_url("http://panel.example.com:8080", "user@name", "p#ss/word", 42, "m3u8");
        assert_eq!(url, "http://panel.example.com:8080/live/user%40name/p%23ss%2Fword/42.m3u8");
    }

    #[test]
    fn expired_status_does_not_block_account_info() {
        let body = serde_json::json!({"user_info": {"auth": 1, "status": "Expired", "exp_date": "1000"}});
        assert!(parse_user_info(&body).is_ok());
    }

    #[test]
    fn missing_auth_field_does_not_block_account_info() {
        let body = serde_json::json!({"user_info": {"status": "Active"}});
        assert!(parse_user_info(&body).is_ok());
    }

    #[test]
    fn explicit_auth_zero_is_rejected() {
        let body = serde_json::json!({"user_info": {"auth": 0}});
        assert!(parse_user_info(&body).is_err());
    }

    #[test]
    fn banned_status_is_still_rejected() {
        let body = serde_json::json!({"user_info": {"auth": 1, "status": "Banned"}});
        assert!(parse_user_info(&body).is_err());
    }

    #[test]
    fn missing_container_extension_leaves_stream_url_unresolved() {
        let body = serde_json::json!({"info": {"name": "Movie"}, "movie_data": {"stream_id": 42}});
        let details = parse_vod_info(&body, "http://s.example.com", "u", "p", "42").unwrap();
        assert!(details.container_extension.is_none());
        assert!(details.stream_url.is_none());
    }

    #[test]
    fn prefers_ts_then_m3u8_then_default() {
        assert_eq!(preferred_live_format(Some(&["m3u8".to_string(), "ts".to_string()])), "ts");
        assert_eq!(preferred_live_format(Some(&["m3u8".to_string()])), "m3u8");
        assert_eq!(preferred_live_format(None), "ts");
    }
}
