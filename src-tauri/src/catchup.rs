//! Catch-up/timeshift (archive/replay) playback URL construction, ported
//! from iptvnator's `catchup.utils.ts` (M3U) and `xtream-url.service.ts`
//! (Xtream). Doesn't port Xtream's live-probe-and-cache variant detection
//! (needs a stateful per-session probe this backend has no equivalent for);
//! defaults to the same `rest:ts` fallback iptvnator itself uses when no
//! probe is available.

use crate::types::Channel;
use regex::Regex;
use std::sync::OnceLock;

fn first_non_blank(values: [Option<&str>; 3]) -> Option<&str> {
    values.into_iter().flatten().find(|s| !s.trim().is_empty())
}

fn is_http_url(s: &str) -> bool {
    reqwest::Url::parse(s.trim())
        .map(|u| u.scheme() == "http" || u.scheme() == "https")
        .unwrap_or(false)
}

/// First non-blank of `catchup.days`, `timeshift`, `tvg.rec`, clamped to
/// `>= 0`. Matches `getM3uArchiveDays()`: an empty-string value at a
/// higher-precedence key is skipped, not treated as present.
pub fn m3u_archive_days(channel: &Channel) -> i64 {
    let catchup_days = channel.catchup.as_ref().and_then(|c| c.days.as_deref());
    let timeshift = channel.timeshift.as_deref();
    let tvg_rec = channel.tvg.rec.as_deref();
    let value = first_non_blank([catchup_days, timeshift, tvg_rec]).unwrap_or("0");
    value.trim().parse::<i64>().unwrap_or(0).max(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum M3uCatchupMode {
    None,
    Source,
    Shift,
}

/// Matches `getM3uCatchupSupportMode()`'s precedence: no support unless
/// `m3u_archive_days > 0`; an absolute-URL `catchup-source` wins outright;
/// then `catchup="shift"` with an http(s) stream URL; then, only when
/// `catchup` is blank/absent (not for other values like `"append"`), an
/// implicit shift fallback for providers that only set `tvg-rec`/`timeshift`.
fn m3u_catchup_support_mode(channel: &Channel) -> M3uCatchupMode {
    if m3u_archive_days(channel) <= 0 {
        return M3uCatchupMode::None;
    }
    let catchup_source = channel.catchup.as_ref().and_then(|c| c.source.as_deref()).unwrap_or("");
    if is_http_url(catchup_source) {
        return M3uCatchupMode::Source;
    }
    let catchup_type = channel
        .catchup
        .as_ref()
        .and_then(|c| c.r#type.as_deref())
        .map(|s| s.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let stream_is_http = is_http_url(&channel.url);
    if catchup_type == "shift" && stream_is_http {
        return M3uCatchupMode::Shift;
    }
    if catchup_type.is_empty() && stream_is_http {
        return M3uCatchupMode::Shift;
    }
    M3uCatchupMode::None
}

/// Whether this channel supports catch-up playback at all — a pure
/// predicate for gating a "replay" affordance in the UI.
pub fn is_m3u_catchup_supported(channel: &Channel) -> bool {
    m3u_catchup_support_mode(channel) != M3uCatchupMode::None
}

fn xmltv_short_timestamp_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d{4})(\d{2})(\d{2})(\d{2})(\d{2})\s*([+-]\d{2})(\d{2})$").unwrap())
}

/// Matches `getEpgProgramTimestampSeconds()`: prefer a positive unix-seconds
/// value; else parse `date_value` as RFC3339 or `YYYY-MM-DD HH:MM:SS`; else
/// the raw XMLTV `YYYYMMDDHHmm ±HHMM` form, using the literal sign character
/// (not the parsed number's sign, which loses it on values like `-0030`).
fn epg_program_timestamp_seconds(date_value: &str, unix_timestamp_value: Option<i64>) -> Option<i64> {
    if let Some(ts) = unix_timestamp_value {
        if ts > 0 {
            return Some(ts);
        }
    }
    let trimmed = date_value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return Some(dt.timestamp());
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_utc().timestamp());
    }
    let caps = xmltv_short_timestamp_regex().captures(trimmed)?;
    let year: i32 = caps[1].parse().ok()?;
    let month: u32 = caps[2].parse().ok()?;
    let day: u32 = caps[3].parse().ok()?;
    let hour: u32 = caps[4].parse().ok()?;
    let minute: u32 = caps[5].parse().ok()?;
    let offset_hours: i64 = caps[6].trim_start_matches(['+', '-']).parse().ok()?;
    let offset_minutes: i64 = caps[7].parse().ok()?;
    let offset_sign: i64 = if caps[6].starts_with('-') { -1 } else { 1 };
    let offset_total_minutes = offset_sign * (offset_hours * 60 + offset_minutes);
    let utc_dt = chrono::NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(hour, minute, 0)?;
    Some(utc_dt.and_utc().timestamp() - offset_total_minutes * 60)
}

/// Sets `utc`/`lutc` query params, matching JS `URLSearchParams.set()`:
/// an existing key is overwritten in place, a missing one appended, other
/// params preserved unchanged.
fn set_catchup_query_params(raw_url: &str, utc: i64, lutc: i64) -> Option<String> {
    let mut url = reqwest::Url::parse(raw_url.trim()).ok()?;
    let existing: Vec<(String, String)> = url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();

    let mut rebuilt: Vec<(String, String)> = Vec::with_capacity(existing.len() + 2);
    let mut has_utc = false;
    let mut has_lutc = false;
    for (k, v) in existing {
        if k == "utc" {
            if has_utc {
                continue;
            }
            rebuilt.push((k, utc.to_string()));
            has_utc = true;
        } else if k == "lutc" {
            if has_lutc {
                continue;
            }
            rebuilt.push((k, lutc.to_string()));
            has_lutc = true;
        } else {
            rebuilt.push((k, v));
        }
    }
    if !has_utc {
        rebuilt.push(("utc".to_string(), utc.to_string()));
    }
    if !has_lutc {
        rebuilt.push(("lutc".to_string(), lutc.to_string()));
    }

    {
        let mut pairs = url.query_pairs_mut();
        pairs.clear();
        for (k, v) in &rebuilt {
            pairs.append_pair(k, v);
        }
    }
    Some(url.to_string())
}

/// Builds a playable M3U catch-up URL for `channel` at `program_start` (an
/// ISO/XMLTV date string; `program_start_timestamp`, if positive, wins over
/// parsing it). `now_timestamp` defaults to now. Matches `resolveM3uCatchupUrl()`:
/// catch-up support is only ever a boolean on/off gate — neither this nor
/// iptvnator enforces "don't rewind past N days" here.
pub fn resolve_m3u_catchup_url(
    channel: &Channel,
    program_start: &str,
    program_start_timestamp: Option<i64>,
    now_timestamp: Option<i64>,
) -> Option<String> {
    let mode = m3u_catchup_support_mode(channel);
    if mode == M3uCatchupMode::None {
        return None;
    }
    let start_ts = epg_program_timestamp_seconds(program_start, program_start_timestamp)?;
    let base_url = match mode {
        M3uCatchupMode::Source => channel.catchup.as_ref()?.source.as_deref()?,
        M3uCatchupMode::Shift => channel.url.as_str(),
        M3uCatchupMode::None => return None,
    };
    if base_url.trim().is_empty() {
        return None;
    }
    let now = now_timestamp.filter(|&n| n > 0).unwrap_or_else(|| chrono::Utc::now().timestamp());
    set_catchup_query_params(base_url, start_ts, now)
}

// ---------------------------------------------------------------------
// Xtream catch-up
// ---------------------------------------------------------------------

/// Matches `isXtreamCatchupAvailable()`: requires BOTH `tv_archive == 1`
/// (not just truthy) AND a positive day count.
pub fn xtream_catchup_available(tv_archive: Option<i64>, tv_archive_duration: Option<i64>) -> bool {
    tv_archive == Some(1) && tv_archive_duration.unwrap_or(0).max(0) > 0
}

/// Formats a catch-up start time as `YYYY-MM-DD:HH-MM` (not ISO8601), in
/// `timezone` (IANA name) when valid, else local machine time. Matches
/// `formatCatchupStartTime()`; seconds are always dropped.
fn format_catchup_start_time(timestamp: i64, timezone: Option<&str>) -> String {
    use chrono::TimeZone;
    if let Some(tz_name) = timezone {
        if let Ok(tz) = tz_name.parse::<chrono_tz::Tz>() {
            if let chrono::LocalResult::Single(dt) = tz.timestamp_opt(timestamp, 0) {
                return format!(
                    "{:04}-{:02}-{:02}:{:02}-{:02}",
                    dt.format("%Y").to_string().parse::<i32>().unwrap_or(0),
                    dt.format("%m").to_string().parse::<u32>().unwrap_or(1),
                    dt.format("%d").to_string().parse::<u32>().unwrap_or(1),
                    dt.format("%H").to_string().parse::<u32>().unwrap_or(0),
                    dt.format("%M").to_string().parse::<u32>().unwrap_or(0),
                );
            }
        }
    }
    let local = chrono::Local.timestamp_opt(timestamp, 0).single().unwrap_or_else(chrono::Local::now);
    format!("{}", local.format("%Y-%m-%d:%H-%M"))
}

/// The five wire-level catch-up URL "flavors", matching `XTREAM_CATCHUP_VARIANT`.
/// Only `RestTs` is ever constructed today (see `resolve_xtream_catchup_url`),
/// but the full enum is kept so `construct_xtream_catchup_url` stays a
/// complete port of the reference's URL-building logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum XtreamCatchupVariant {
    Legacy,
    LegacyM3u8,
    LegacyTs,
    RestM3u8,
    RestTs,
}

impl XtreamCatchupVariant {
    fn extension(self) -> Option<&'static str> {
        match self {
            XtreamCatchupVariant::LegacyM3u8 | XtreamCatchupVariant::RestM3u8 => Some("m3u8"),
            XtreamCatchupVariant::LegacyTs | XtreamCatchupVariant::RestTs => Some("ts"),
            XtreamCatchupVariant::Legacy => None,
        }
    }

    fn is_legacy(self) -> bool {
        matches!(self, XtreamCatchupVariant::Legacy | XtreamCatchupVariant::LegacyM3u8 | XtreamCatchupVariant::LegacyTs)
    }
}

/// Builds a catch-up URL for one Xtream stream. `start_timestamp`/
/// `stop_timestamp` are Unix seconds (a real EPG program's start/stop).
/// Matches `constructCatchupUrl()`'s two schemes exactly:
/// - REST: `{server}/timeshift/{user}/{pass}/{durationMinutes}/{timeString}/{streamId}.{ext}`
///   (credentials URL-path-encoded, same as live/VOD stream URLs)
/// - Legacy: `{server}/streaming/timeshift.php?username=..&password=..&stream=..&start=..&duration=..[&extension=..]`
///   (credentials only trimmed, then percent-encoded by normal query encoding)
///
/// `duration_minutes` is `max(1, round((stop - start) / 60))`.
#[allow(clippy::too_many_arguments)]
pub fn construct_xtream_catchup_url(
    server_url: &str,
    username: &str,
    password: &str,
    stream_id: i64,
    start_timestamp: i64,
    stop_timestamp: i64,
    variant: XtreamCatchupVariant,
    server_timezone: Option<&str>,
) -> String {
    let username = username.trim();
    let password = password.trim();
    let duration_minutes = ((stop_timestamp - start_timestamp) as f64 / 60.0).round().max(1.0) as i64;
    let time_string = format_catchup_start_time(start_timestamp, server_timezone);
    let extension = variant.extension();

    if variant.is_legacy() {
        let mut params = vec![
            ("username".to_string(), username.to_string()),
            ("password".to_string(), password.to_string()),
            ("stream".to_string(), stream_id.to_string()),
            ("start".to_string(), time_string),
            ("duration".to_string(), duration_minutes.to_string()),
        ];
        if let Some(ext) = extension {
            params.push(("extension".to_string(), ext.to_string()));
        }
        let query: String = params
            .iter()
            .map(|(k, v)| format!("{k}={}", urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        format!("{}/streaming/timeshift.php?{query}", server_url.trim_end_matches('/'))
    } else {
        let user_enc = urlencoding::encode(username);
        let pass_enc = urlencoding::encode(password);
        let ext = extension.unwrap_or("ts");
        format!(
            "{}/timeshift/{user_enc}/{pass_enc}/{duration_minutes}/{time_string}/{stream_id}.{ext}",
            server_url.trim_end_matches('/')
        )
    }
}

/// Resolves a catch-up URL for one Xtream live stream at the given EPG
/// program window, defaulting to the `rest:ts` variant — matches iptvnator's
/// fallback for a context with no live-probe capability (this backend has
/// none). MPEG-TS-first also matches the reference client's preference when
/// both formats are viable (some portals' HLS manifest is valid but the
/// first media segment fails in Chromium/video.js).
pub fn resolve_xtream_catchup_url(
    server_url: &str,
    username: &str,
    password: &str,
    stream_id: i64,
    start_timestamp: i64,
    stop_timestamp: i64,
    server_timezone: Option<&str>,
) -> String {
    construct_xtream_catchup_url(
        server_url,
        username,
        password,
        stream_id,
        start_timestamp,
        stop_timestamp,
        XtreamCatchupVariant::RestTs,
        server_timezone,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ChannelCatchup, ChannelGroup, ChannelHttp, ChannelTvg};

    fn base_channel() -> Channel {
        Channel {
            id: "1".into(),
            url: "https://streams.example.com/live/channel-1.m3u8".into(),
            name: "Test".into(),
            group: ChannelGroup::default(),
            tvg: ChannelTvg::default(),
            epg_params: None,
            timeshift: None,
            catchup: None,
            http: ChannelHttp::default(),
            radio: "0".into(),
            drm: None,
            raw: None,
            channel_number: None,
        }
    }

    #[test]
    fn m3u_shift_mode_builds_url_from_stream_url() {
        let mut channel = base_channel();
        channel.catchup = Some(ChannelCatchup { r#type: Some("shift".into()), source: None, days: Some("7".into()) });
        let url = resolve_m3u_catchup_url(&channel, "", Some(1775808800), Some(1775820000)).unwrap();
        assert_eq!(url, "https://streams.example.com/live/channel-1.m3u8?utc=1775808800&lutc=1775820000");
    }

    #[test]
    fn m3u_source_mode_preserves_existing_params_in_place() {
        let mut channel = base_channel();
        channel.catchup = Some(ChannelCatchup {
            r#type: None,
            source: Some("https://archive.example.com/catchup.m3u8?utc=1&lutc=2&token=abc".into()),
            days: Some("7".into()),
        });
        let url = resolve_m3u_catchup_url(&channel, "", Some(1775808800), Some(1775820000)).unwrap();
        assert_eq!(url, "https://archive.example.com/catchup.m3u8?utc=1775808800&lutc=1775820000&token=abc");
    }

    #[test]
    fn m3u_implicit_shift_fallback_when_catchup_type_blank() {
        let mut channel = base_channel();
        channel.timeshift = Some("2".into());
        assert!(is_m3u_catchup_supported(&channel));
        let url = resolve_m3u_catchup_url(&channel, "", Some(1775808800), Some(1775820000));
        assert!(url.is_some());
    }

    #[test]
    fn m3u_unsupported_catchup_type_blocks_implicit_fallback() {
        let mut channel = base_channel();
        channel.timeshift = Some("2".into());
        channel.catchup = Some(ChannelCatchup { r#type: Some("append".into()), source: None, days: None });
        assert!(!is_m3u_catchup_supported(&channel));
    }

    #[test]
    fn m3u_zero_archive_days_disables_catchup() {
        let mut channel = base_channel();
        channel.catchup = Some(ChannelCatchup { r#type: Some("shift".into()), source: None, days: Some("0".into()) });
        assert!(!is_m3u_catchup_supported(&channel));
    }

    #[test]
    fn m3u_archive_days_skips_blank_higher_precedence_value() {
        let mut channel = base_channel();
        channel.catchup = Some(ChannelCatchup { r#type: None, source: None, days: Some("".into()) });
        channel.timeshift = Some("".into());
        channel.tvg.rec = Some("3".into());
        assert_eq!(m3u_archive_days(&channel), 3);
    }

    #[test]
    fn xmltv_short_timestamp_offsets_match_reference_vectors() {
        assert_eq!(epg_program_timestamp_seconds("202604100800 +0000", None), Some(1775808000));
        assert_eq!(epg_program_timestamp_seconds("202604100800 -0030", None), Some(1775809800));
        assert_eq!(epg_program_timestamp_seconds("202604100800 +0530", None), Some(1775788200));
        assert_eq!(epg_program_timestamp_seconds("202604100800 +0030", None), Some(1775806200));
        assert_eq!(epg_program_timestamp_seconds("202604100800 -0000", None), Some(1775808000));
    }

    #[test]
    fn unix_timestamp_wins_over_date_string() {
        assert_eq!(epg_program_timestamp_seconds("garbage", Some(1700000000)), Some(1700000000));
    }

    #[test]
    fn xtream_catchup_availability_requires_both_flag_and_days() {
        assert!(xtream_catchup_available(Some(1), Some(3)));
        assert!(!xtream_catchup_available(Some(1), Some(0)));
        assert!(!xtream_catchup_available(Some(0), Some(3)));
        assert!(!xtream_catchup_available(None, Some(3)));
    }

    #[test]
    fn xtream_rest_url_matches_reference_template() {
        let url = construct_xtream_catchup_url(
            "http://panel.example.com:8080",
            "user",
            "pass",
            42,
            1775808800,
            1775810600, // 1800s later = 30 minutes
            XtreamCatchupVariant::RestTs,
            None,
        );
        assert!(url.starts_with("http://panel.example.com:8080/timeshift/user/pass/30/"));
        assert!(url.ends_with("/42.ts"));
    }

    #[test]
    fn xtream_legacy_url_matches_reference_template() {
        let url = construct_xtream_catchup_url(
            "http://panel.example.com:8080",
            "user",
            "pass",
            42,
            1775808800,
            1775810600,
            XtreamCatchupVariant::LegacyM3u8,
            None,
        );
        assert!(url.starts_with("http://panel.example.com:8080/streaming/timeshift.php?"));
        assert!(url.contains("username=user"));
        assert!(url.contains("stream=42"));
        assert!(url.contains("duration=30"));
        assert!(url.contains("extension=m3u8"));
    }

    #[test]
    fn xtream_duration_minutes_floors_at_one() {
        let url = construct_xtream_catchup_url(
            "http://p.example.com",
            "u",
            "p",
            1,
            1775808800,
            1775808810, // 10 seconds later - rounds to 0, floored to 1
            XtreamCatchupVariant::RestTs,
            None,
        );
        assert!(url.contains("/1/"));
    }

    #[test]
    fn format_catchup_start_time_uses_named_timezone_when_valid() {
        // 2025-03-01 02:00:00 UTC
        let formatted = format_catchup_start_time(1740794400, Some("America/New_York"));
        assert_eq!(formatted, "2025-02-28:21-00");
    }
}
