//! Small URL-sanitization helpers shared across every stream source (M3U,
//! Xtream, Stalker) and the stream proxy itself, so a fix here can't drift
//! out of sync between them the way it would if each module kept its own copy.

/// Strips a leading "solution token" some providers prefix onto a playable
/// URL (e.g. `ffmpeg http://host/live.php?...`, `ffrt4://...`). Written for
/// Stalker's `create_link`, but the same convention also shows up in M3U
/// stream lines from panels sharing Stalker backend infra - left unstripped,
/// URL parsing fails with a confusing `RelativeUrlWithoutBase` far from the
/// actual cause.
pub fn strip_solution_token(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(idx) = trimmed.find("http://").or_else(|| trimmed.find("https://")) {
        if idx > 0 {
            return trimmed[idx..].to_string();
        }
    }
    trimmed.to_string()
}

/// Splits the legacy `url|User-Agent=...&Referer=...` M3U convention (pre-
/// dating `#EXTVLCOPT:`/attribute forms) into a clean URL plus any
/// `User-Agent`/`Referer` values. Returns the URL unchanged when there's no
/// `|`, covering most real playlists.
pub fn split_pipe_http_headers(url: &str) -> (String, Option<String>, Option<String>) {
    let Some((base, params)) = url.split_once('|') else {
        return (url.to_string(), None, None);
    };

    let mut user_agent = None;
    let mut referrer = None;
    for pair in params.split('&') {
        let Some((key, value)) = pair.split_once('=') else { continue };
        let decoded = percent_encoding::percent_decode_str(value).decode_utf8_lossy().into_owned();
        match key.trim().to_ascii_lowercase().as_str() {
            "user-agent" => user_agent = Some(decoded),
            "referer" | "referrer" => referrer = Some(decoded),
            _ => {}
        }
    }
    (base.to_string(), user_agent, referrer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_leading_solution_token() {
        assert_eq!(
            strip_solution_token("ffmpeg http://host/play/live.php?mac=1&stream=2"),
            "http://host/play/live.php?mac=1&stream=2"
        );
    }

    #[test]
    fn leaves_plain_urls_untouched() {
        assert_eq!(strip_solution_token("http://host/one.m3u8"), "http://host/one.m3u8");
    }

    #[test]
    fn splits_pipe_separated_headers() {
        let (url, ua, referer) = split_pipe_http_headers("http://host/one.ts|User-Agent=VLC%2F3.0&Referer=http://ref.example.com");
        assert_eq!(url, "http://host/one.ts");
        assert_eq!(ua.as_deref(), Some("VLC/3.0"));
        assert_eq!(referer.as_deref(), Some("http://ref.example.com"));
    }

    #[test]
    fn no_pipe_leaves_url_and_headers_untouched() {
        let (url, ua, referer) = split_pipe_http_headers("http://host/one.ts");
        assert_eq!(url, "http://host/one.ts");
        assert!(ua.is_none());
        assert!(referer.is_none());
    }
}
