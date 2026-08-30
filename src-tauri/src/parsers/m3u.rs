use crate::types::ChannelDrm;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, Default)]
pub struct ParsedM3uItem {
    pub name: String,
    pub url: String,
    pub tvg_id: Option<String>,
    pub tvg_name: Option<String>,
    pub tvg_url: Option<String>,
    pub tvg_logo: Option<String>,
    pub tvg_rec: Option<String>,
    pub group_title: Option<String>,
    pub http_referrer: Option<String>,
    pub http_user_agent: Option<String>,
    pub catchup_type: Option<String>,
    pub catchup_source: Option<String>,
    pub catchup_days: Option<String>,
    pub timeshift: Option<String>,
    pub radio: String,
    pub drm: Option<ChannelDrm>,
    /// The raw `#EXTINF:` line plus any `#KODIPROP:`/`#EXTVLCOPT:` lines that
    /// preceded the URL — kept so nothing the attribute parser doesn't
    /// recognize is silently lost.
    pub raw: String,
}

fn attr_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"([A-Za-z0-9_-]+)="([^"]*)""#).unwrap())
}

fn kodiprop_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^#KODIPROP:([^=]+)=(.*)$").unwrap())
}

fn extvlcopt_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^#EXTVLCOPT:\s*([^=]+)=(.*)$").unwrap())
}

/// Extracts EPG guide URLs some providers declare in the `#EXTM3U` header
/// itself (`x-tvg-url`/`url-tvg`/`tvg-url`), matching iptvnator's detection.
/// A single attribute can carry multiple comma-or-semicolon-separated URLs;
/// order is preserved and duplicates dropped.
fn extract_header_epg_urls(header_line: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut urls = Vec::new();
    for caps in attr_regex().captures_iter(header_line) {
        let key = caps[1].to_lowercase();
        if !matches!(key.as_str(), "x-tvg-url" | "url-tvg" | "tvg-url") {
            continue;
        }
        for candidate in caps[2].split(&[',', ';'][..]) {
            let candidate = candidate.trim();
            if !candidate.is_empty() && seen.insert(candidate.to_string()) {
                urls.push(candidate.to_string());
            }
        }
    }
    urls
}

/// `parse()`'s full result: the channel entries plus any EPG guide URLs
/// declared in the `#EXTM3U` header line.
#[derive(Debug, Clone, Default)]
pub struct ParsedM3uPlaylist {
    pub items: Vec<ParsedM3uItem>,
    pub detected_epg_urls: Vec<String>,
}

/// `#EXTM3U`/`#EXTINF:` grammar. Attributes are parsed via `key="value"`
/// matching; the display name is recovered from whatever's left after
/// stripping recognized attributes, avoiding full comma-in-quotes tracking.
pub fn parse(content: &str) -> ParsedM3uPlaylist {
    let mut items = Vec::new();
    let mut detected_epg_urls = Vec::new();
    let mut pending: Option<ParsedM3uItem> = None;
    let mut pending_raw_lines: Vec<String> = Vec::new();
    let mut pending_kodiprops: Vec<(String, String)> = Vec::new();
    let mut pending_vlc_user_agent: Option<String> = None;
    let mut pending_vlc_referrer: Option<String> = None;

    for line in content.lines() {
        let line = line.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("#EXTINF:") {
            // Only an *orphaned* previous #EXTINF (no URL line before this
            // one) invalidates what was buffered since it started. With no
            // pending item, buffered KODIPROP/EXTVLCOPT/raw lines were
            // collected BEFORE this #EXTINF and belong to the entry about to
            // start (Kodi/TiviMate can declare DRM props ahead of the entry
            // line) - must carry forward, not be wiped.
            if pending.is_some() {
                pending_kodiprops.clear();
                pending_raw_lines.clear();
                pending_vlc_user_agent = None;
                pending_vlc_referrer = None;
            }
            pending = Some(parse_extinf(rest));
            pending_raw_lines.push(line.to_string());
            continue;
        }

        if trimmed.eq_ignore_ascii_case("#EXTM3U") || trimmed.starts_with("#EXTM3U") {
            detected_epg_urls.extend(extract_header_epg_urls(trimmed));
            continue;
        }

        if let Some(caps) = kodiprop_regex().captures(trimmed) {
            pending_raw_lines.push(line.to_string());
            pending_kodiprops.push((caps[1].trim().to_lowercase(), caps[2].trim().to_string()));
            continue;
        }

        if let Some(caps) = extvlcopt_regex().captures(trimmed) {
            pending_raw_lines.push(line.to_string());
            let value = caps[2].trim().trim_matches('"').to_string();
            match caps[1].trim().to_ascii_lowercase().as_str() {
                "http-user-agent" => pending_vlc_user_agent = Some(value),
                "http-referrer" | "http-referer" => pending_vlc_referrer = Some(value),
                _ => {}
            }
            continue;
        }

        if trimmed.starts_with('#') {
            // Unrecognized comment/directive (#EXTGRP, ...) — keep in raw only.
            pending_raw_lines.push(line.to_string());
            continue;
        }

        // Non-comment line: the stream URL completing the pending item.
        if let Some(mut item) = pending.take() {
            let (clean_url, pipe_user_agent, pipe_referrer) = crate::net::url_utils::split_pipe_http_headers(trimmed);
            item.url = crate::net::url_utils::strip_solution_token(&clean_url);
            item.http_user_agent = item.http_user_agent.or(pending_vlc_user_agent.take()).or(pipe_user_agent);
            item.http_referrer = item.http_referrer.or(pending_vlc_referrer.take()).or(pipe_referrer);
            item.drm = extract_drm(&pending_kodiprops);
            item.raw = pending_raw_lines.join("\n");
            items.push(item);
        }
        pending_raw_lines.clear();
        pending_kodiprops.clear();
        pending_vlc_user_agent = None;
        pending_vlc_referrer = None;
    }

    ParsedM3uPlaylist { items, detected_epg_urls }
}

fn parse_extinf(rest: &str) -> ParsedM3uItem {
    let mut attrs = std::collections::HashMap::new();
    for caps in attr_regex().captures_iter(rest) {
        attrs.insert(caps[1].to_lowercase(), caps[2].to_string());
    }

    let without_attrs = attr_regex().replace_all(rest, "");
    let name = without_attrs
        .splitn(2, ',')
        .nth(1)
        .unwrap_or("")
        .trim()
        .to_string();

    ParsedM3uItem {
        name: if name.is_empty() {
            "Unnamed Channel".to_string()
        } else {
            name
        },
        tvg_id: attrs.get("tvg-id").cloned(),
        tvg_name: attrs.get("tvg-name").cloned(),
        tvg_url: attrs.get("tvg-url").cloned(),
        tvg_logo: attrs.get("tvg-logo").cloned(),
        tvg_rec: attrs.get("tvg-rec").cloned(),
        group_title: attrs.get("group-title").cloned(),
        http_referrer: attrs.get("http-referrer").cloned(),
        http_user_agent: attrs.get("http-user-agent").cloned(),
        catchup_type: attrs.get("catchup").cloned(),
        catchup_source: attrs.get("catchup-source").cloned(),
        catchup_days: attrs.get("catchup-days").cloned(),
        timeshift: attrs.get("timeshift").cloned(),
        radio: normalize_radio_flag(attrs.get("radio").map(String::as_str)),
        ..Default::default()
    }
}

/// The `radio` attribute is boolean-ish text (`"true"`/`"yes"`/`"1"`, etc);
/// the frontend only checks for literal `"1"`, so anything else must be
/// normalized here or a spec-compliant `radio="true"` entry silently shows
/// up as a regular TV channel.
fn normalize_radio_flag(value: Option<&str>) -> String {
    match value.map(|s| s.trim().to_ascii_lowercase()) {
        Some(v) if v == "1" || v == "true" || v == "yes" => "1".to_string(),
        _ => "0".to_string(),
    }
}

/// Best-effort `#KODIPROP:` DRM extraction. Captures `license_type` and raw
/// `license_key` as-is rather than fully decoding every ClearKey encoding
/// iptvnator supports - `VideoPlayer.svelte` doesn't wire EME license
/// handling yet, so there's nothing to consume a decoded key today. Extend
/// once DRM playback is implemented.
fn extract_drm(kodiprops: &[(String, String)]) -> Option<ChannelDrm> {
    if kodiprops.is_empty() {
        return None;
    }

    let mut drm_type = None;
    let mut license_key = None;
    for (key, value) in kodiprops {
        match key.as_str() {
            "inputstream.adaptive.license_type" => drm_type = Some(value.clone()),
            "inputstream.adaptive.license_key" => license_key = Some(value.clone()),
            "inputstream.adaptive.drm_legacy" => {
                if let Some((t, k)) = value.split_once('|') {
                    drm_type.get_or_insert_with(|| t.to_string());
                    license_key.get_or_insert_with(|| k.to_string());
                }
            }
            _ => {}
        }
    }

    if drm_type.is_none() && license_key.is_none() {
        return None;
    }

    Some(ChannelDrm {
        r#type: drm_type,
        license_url: None,
        headers: None,
        data: license_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_entry() {
        let m3u = "#EXTM3U\n#EXTINF:-1,Channel One\nhttp://example.com/one.m3u8\n";
        let items = parse(m3u).items;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Channel One");
        assert_eq!(items[0].url, "http://example.com/one.m3u8");
    }

    #[test]
    fn parses_attributes_and_group() {
        let m3u = concat!(
            "#EXTM3U\n",
            "#EXTINF:-1 tvg-id=\"bbc1\" tvg-logo=\"http://logo\" group-title=\"News\",BBC One\n",
            "http://example.com/bbc1.m3u8\n"
        );
        let items = parse(m3u).items;
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.name, "BBC One");
        assert_eq!(item.tvg_id.as_deref(), Some("bbc1"));
        assert_eq!(item.tvg_logo.as_deref(), Some("http://logo"));
        assert_eq!(item.group_title.as_deref(), Some("News"));
    }

    #[test]
    fn drops_extinf_with_no_following_url() {
        let m3u = "#EXTM3U\n#EXTINF:-1,Orphan Entry\n#EXTINF:-1,Real Entry\nhttp://example.com/real.m3u8\n";
        let items = parse(m3u).items;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Real Entry");
    }

    #[test]
    fn extracts_kodiprop_clearkey_drm() {
        let m3u = concat!(
            "#EXTM3U\n",
            "#EXTINF:-1,DRM Channel\n",
            "#KODIPROP:inputstream.adaptive.license_type=clearkey\n",
            "#KODIPROP:inputstream.adaptive.license_key=deadbeefdeadbeefdeadbeefdeadbeef:00112233445566778899aabbccddeeff\n",
            "http://example.com/drm.m3u8\n"
        );
        let items = parse(m3u).items;
        assert_eq!(items.len(), 1);
        let drm = items[0].drm.as_ref().expect("expected drm to be extracted");
        assert_eq!(drm.r#type.as_deref(), Some("clearkey"));
        assert!(drm.data.is_some());
    }

    #[test]
    fn extracts_kodiprop_declared_before_extinf() {
        // Kodi/TiviMate playlists sometimes declare DRM properties BEFORE
        // the #EXTINF line, not just after it. A prior bug cleared the
        // buffered KODIPROP lines the moment #EXTINF was seen, silently
        // dropping DRM for exactly this layout.
        let m3u = concat!(
            "#EXTM3U\n",
            "#KODIPROP:inputstream.adaptive.license_type=clearkey\n",
            "#KODIPROP:inputstream.adaptive.license_key=deadbeefdeadbeefdeadbeefdeadbeef:00112233445566778899aabbccddeeff\n",
            "#EXTINF:-1,DRM Channel\n",
            "http://example.com/drm.m3u8\n"
        );
        let items = parse(m3u).items;
        assert_eq!(items.len(), 1);
        let drm = items[0].drm.as_ref().expect("expected drm to be extracted");
        assert_eq!(drm.r#type.as_deref(), Some("clearkey"));
    }

    #[test]
    fn normalizes_radio_attribute_variants() {
        let m3u = concat!(
            "#EXTM3U\n",
            "#EXTINF:-1 radio=\"true\",Radio One\n",
            "http://example.com/radio1\n",
            "#EXTINF:-1 radio=\"1\",Radio Two\n",
            "http://example.com/radio2\n",
            "#EXTINF:-1,TV Channel\n",
            "http://example.com/tv\n"
        );
        let items = parse(m3u).items;
        assert_eq!(items[0].radio, "1");
        assert_eq!(items[1].radio, "1");
        assert_eq!(items[2].radio, "0");
    }

    #[test]
    fn parses_extvlcopt_headers() {
        let m3u = concat!(
            "#EXTM3U\n",
            "#EXTINF:-1,Header Channel\n",
            "#EXTVLCOPT:http-user-agent=CustomAgent/1.0\n",
            "#EXTVLCOPT:http-referrer=http://ref.example.com\n",
            "http://example.com/one.m3u8\n"
        );
        let items = parse(m3u).items;
        assert_eq!(items[0].http_user_agent.as_deref(), Some("CustomAgent/1.0"));
        assert_eq!(items[0].http_referrer.as_deref(), Some("http://ref.example.com"));
    }

    #[test]
    fn strips_solution_token_prefix_from_url() {
        let m3u = concat!(
            "#EXTM3U\n",
            "#EXTINF:-1,Panel Channel\n",
            "ffmpeg http://vod.example.com/play/live.php?mac=00:1A:79&stream=1&play_token=abc\n"
        );
        let items = parse(m3u).items;
        assert_eq!(items[0].url, "http://vod.example.com/play/live.php?mac=00:1A:79&stream=1&play_token=abc");
    }

    #[test]
    fn handles_multiple_entries_and_missing_group() {
        let m3u = concat!(
            "#EXTM3U\n",
            "#EXTINF:-1,First\nhttp://example.com/1.m3u8\n",
            "#EXTINF:-1,Second\nhttp://example.com/2.m3u8\n"
        );
        let items = parse(m3u).items;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "First");
        assert_eq!(items[1].name, "Second");
        assert!(items[0].group_title.is_none());
    }
}
