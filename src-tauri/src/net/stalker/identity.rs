use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use sha1::{Digest as _, Sha1};
use sha2::Sha256;

/// The exact set `encodeURIComponent` leaves unescaped (`A-Za-z0-9` plus
/// `-_.!~*'()`), applied to every non-`cmd` param. Plain `NON_ALPHANUMERIC`
/// also escapes those nine marks (`get_genres` -> `get%5Fgenres`) - decodes
/// the same server-side, but not the bytes a real MAG box puts on the wire.
const URI_COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

/// The placeholder serial older builds (and other Stalker clients) wrote
/// into every playlist. Identifies no real device, so it's treated as
/// *absent* everywhere a serial is read.
pub const LEGACY_DEFAULT_STALKER_SERIAL: &str = "BEDACD4569BAF";

/// Trim-normalizes an optional identity value, collapsing blank/whitespace
/// to `None`. The portal pins `device_id`/`device_id2` to the MAC on first
/// sight and refuses a later mismatch - a stray space would otherwise be a
/// permanent lockout. Mirrors `normalizeStalkerIdentityValue`.
pub fn normalize_identity_value(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

/// `normalize_identity_value` plus the legacy-placeholder rule — see
/// `LEGACY_DEFAULT_STALKER_SERIAL`. Mirrors `normalizeStalkerSerialNumber`.
pub fn normalize_serial_number(value: Option<&str>) -> Option<&str> {
    normalize_identity_value(value).filter(|s| !s.eq_ignore_ascii_case(LEGACY_DEFAULT_STALKER_SERIAL))
}

pub const STALKER_MAG_USER_AGENT: &str =
    "Mozilla/5.0 (QtEmbedded; U; Linux; C) AppleWebKit/533.3 (KHTML, like Gecko) MAG250";

/// Fixed MAG250 firmware fingerprint block sent on every `get_profile` call
/// — real portals key device recognition off this looking like a real box,
/// not just the MAC.
pub const STB_PROFILE_PARAMS: &[(&str, &str)] = &[
    (
        "ver",
        "ImageDescription: 0.2.18-r14-pub-250; ImageDate: Fri Jan 15 15:20:44 EET 2016; PORTAL version: 5.6.0; API Version: JS API version: 328; STB API version: 134; Player Engine version: 0x566",
    ),
    ("stb_type", "MAG250"),
    ("hw_version", "1.7-BD-00"),
    ("image_version", "218"),
    ("client_type", "STB"),
    ("num_banks", "2"),
    ("video_out", "hdmi"),
    ("hd", "1"),
];

fn hex_upper(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// Canonicalizes a user-typed MAC to the uppercase colon-separated form a
/// real STB sends (`00:1A:79:31:66:30`) - portals key MAC-bound records on
/// this exact form, so a differently-formatted MAC would produce a different
/// `Cookie`/SHA-1 `prehash` and fail auth or bind the wrong device record.
/// Falls back to plain trim+uppercase for anything not exactly 12 hex digits.
pub fn normalize_mac(raw: &str) -> String {
    let hex_digits: String = raw.trim().chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex_digits.len() == 12 {
        let upper = hex_digits.to_ascii_uppercase();
        upper
            .as_bytes()
            .chunks(2)
            .map(|c| std::str::from_utf8(c).unwrap())
            .collect::<Vec<_>>()
            .join(":")
    } else {
        raw.trim().to_ascii_uppercase()
    }
}

pub fn prehash(mac_address: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(mac_address.to_uppercase().as_bytes());
    hex_upper(&hasher.finalize())
}

/// Salt appended to the MAC before hashing `device_id2` - matches the
/// convention other Stalker clients (StbEmu, `stalker-to-m3u`) use, so
/// switching from one of those doesn't trigger a device conflict.
const DEVICE_ID2_SALT: &str = "stalker";

/// Derives `(device_id1, device_id2)` from a MAC the way StbEmu/
/// `stalker-to-m3u`-compatible clients do: uppercase hex SHA-256 of the
/// canonical MAC for `device_id1`, and of the MAC + literal suffix
/// `"stalker"` for `device_id2`. Returns `None` for an invalid MAC (never
/// hash a typo).
///
/// **Deliberately NOT wired into any request path** - only an explicit,
/// opt-in, import-time "derive from MAC" frontend action
/// (`commands::stalker::stalker_derive_device_ids`) calls this; never a
/// silent runtime fallback.
pub fn derive_device_ids(mac_address: &str) -> Option<(String, String)> {
    // `normalize_mac` falls back to plain uppercase trim for non-12-hex-digit
    // input rather than failing - fine for its own purpose, but wrong here:
    // hashing an unrecognizable string would silently bind a device id to a typo.
    let hex_digit_count = mac_address.trim().chars().filter(|c| c.is_ascii_hexdigit()).count();
    if hex_digit_count != 12 {
        return None;
    }
    let mac = normalize_mac(mac_address);
    let mut hasher1 = Sha256::new();
    hasher1.update(mac.as_bytes());
    let device_id1 = hex_upper(&hasher1.finalize());

    let mut hasher2 = Sha256::new();
    hasher2.update(format!("{mac}{DEVICE_ID2_SALT}").as_bytes());
    let device_id2 = hex_upper(&hasher2.finalize());

    Some((device_id1, device_id2))
}

/// Lowercased hex chars of the serial + a fixed suffix, truncated/padded to
/// exactly 32 chars — matches the reference client's derived `__cfduid`
/// cookie for portals that pin sessions to it.
pub fn serial_cfduid(serial: &str) -> String {
    let hex_chars: String = serial
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let mut combined = format!("{hex_chars}e030245495acd6ebfc1");
    combined.truncate(32);
    while combined.len() < 32 {
        combined.push('0');
    }
    combined
}

pub fn build_cookie(mac_address: &str, serial_number: Option<&str>) -> String {
    let mut cookie = format!("mac={mac_address}; stb_lang=en_US@rg=dezzzz; timezone=Europe/Berlin");
    if let Some(serial) = normalize_serial_number(serial_number) {
        cookie.push_str(&format!("; __cfduid={}", serial_cfduid(serial)));
    }
    cookie
}

/// Full identity header set for talking to the portal's `load.php`/
/// `portal.php` API itself. Playback-stream headers are a separate,
/// narrower set — see `stream_proxy.rs`.
pub fn build_api_headers(mac_address: &str, serial_number: Option<&str>, token: Option<&str>) -> Vec<(String, String)> {
    // Includes `Connection: keep-alive` - an earlier revision removed it on
    // the theory it conflicted with reqwest/hyper's connection management,
    // but removing it alone didn't fix the observed connection resets, so
    // it's restored to match the reference client's wire format.
    let mut headers = vec![
        ("User-Agent".to_string(), STALKER_MAG_USER_AGENT.to_string()),
        ("X-User-Agent".to_string(), STALKER_MAG_USER_AGENT.to_string()),
        ("Cookie".to_string(), build_cookie(mac_address, serial_number)),
        ("Accept".to_string(), "*/*".to_string()),
        ("Connection".to_string(), "keep-alive".to_string()),
        ("Accept-Language".to_string(), "en-US,en;q=0.9".to_string()),
    ];
    // Normalized here as well as in `build_cookie` so the `SN` header and the
    // serial-derived `__cfduid` can never disagree about what the serial is.
    if let Some(serial) = normalize_serial_number(serial_number) {
        headers.push(("SN".to_string(), serial.to_string()));
    }
    if let Some(token) = token.filter(|t| !t.is_empty()) {
        headers.push(("Authorization".to_string(), format!("Bearer {token}")));
    }
    headers
}

/// Encodes a `cmd` query VALUE with the portal's own minimal scheme, not
/// standard percent-encoding: a fixed safe set
/// (`A-Za-z0-9-_.~!*()/:?@$,+=[]%`) passes through raw (survives a
/// `Url::parse` round trip unchanged), everything else is percent-encoded
/// uppercase hex. `%` is in the safe set, so an already-encoded `cmd` is
/// never double-encoded - matches how a real MAG box's PHP `$_GET` decodes
/// `cmd` exactly once.
pub fn encode_cmd_value(cmd: &str) -> String {
    let mut out = String::with_capacity(cmd.len());
    for ch in cmd.chars() {
        if is_safe_cmd_char(ch) {
            out.push(ch);
        } else {
            let mut buf = [0u8; 4];
            for byte in ch.encode_utf8(&mut buf).as_bytes() {
                out.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    out
}

fn is_safe_cmd_char(ch: char) -> bool {
    matches!(ch,
        'A'..='Z' | 'a'..='z' | '0'..='9'
        | '-' | '_' | '.' | '~' | '!' | '*' | '(' | ')'
        | '/' | ':' | '?' | '@' | '$' | ',' | '+' | '=' | '[' | ']' | '%'
    )
}

/// Builds `{base-without-query}?key=val&...` - `cmd` uses the portal's
/// minimal encoding (`encode_cmd_value`), every other param uses
/// `encodeURIComponent`'s unreserved set (`URI_COMPONENT`).
/// `JsHttpRequest=1-xml` is appended if missing - required by every portal
/// action.
pub fn build_request_url(base: &str, params: &[(&str, &str)]) -> String {
    let base_without_query = base.split('?').next().unwrap_or(base);
    let mut has_js_http_request = false;
    let mut parts = Vec::with_capacity(params.len() + 1);
    for (key, value) in params {
        if *key == "JsHttpRequest" {
            has_js_http_request = true;
        }
        if *key == "cmd" {
            parts.push(format!("{key}={}", encode_cmd_value(value)));
        } else {
            parts.push(format!("{key}={}", utf8_percent_encode(value, URI_COMPONENT)));
        }
    }
    if !has_js_http_request {
        parts.push("JsHttpRequest=1-xml".to_string());
    }
    format!("{base_without_query}?{}", parts.join("&"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_encoding_leaves_url_safe_chars_raw_and_escapes_metacharacters() {
        let encoded = encode_cmd_value("ffmpeg http://host/live.php?tok=abc&x=1#frag;y");
        assert!(encoded.contains("http://host/live.php?tok=abc"));
        assert!(encoded.contains("%20")); // space between "ffmpeg" and the URL
        assert!(encoded.contains("%26")); // &
        assert!(encoded.contains("%23")); // #
        assert!(encoded.contains("%3B")); // ;
    }

    #[test]
    fn cmd_encoding_never_double_encodes_existing_percent_sequences() {
        let encoded = encode_cmd_value("ffrt3 http://host/live.php?tok=abc%3Adef");
        assert!(encoded.contains("abc%3Adef"));
        assert!(!encoded.contains("%253A"));
    }

    #[test]
    fn cmd_encoding_percent_encodes_non_ascii_as_utf8_bytes() {
        let encoded = encode_cmd_value("café");
        assert_eq!(encoded, "caf%C3%A9");
    }

    #[test]
    fn prehash_is_uppercase_sha1_of_uppercased_mac() {
        let hash = prehash("00:1a:79:aa:bb:cc");
        assert_eq!(hash.len(), 40);
        assert_eq!(hash, hash.to_uppercase());
    }

    #[test]
    fn derives_device_ids_as_64_char_uppercase_hex_and_is_deterministic() {
        let (id1, id2) = derive_device_ids("00:1a:79:31:66:30").expect("valid MAC");
        assert_eq!(id1.len(), 64);
        assert_eq!(id2.len(), 64);
        assert_eq!(id1, id1.to_uppercase());
        assert_ne!(id1, id2);
        let (id1_again, id2_again) = derive_device_ids("00-1A-79-31-66-30").expect("valid MAC, different formatting");
        assert_eq!(id1, id1_again);
        assert_eq!(id2, id2_again);
    }

    #[test]
    fn derive_device_ids_rejects_invalid_mac() {
        assert!(derive_device_ids("not-a-mac").is_none());
        assert!(derive_device_ids("").is_none());
    }

    #[test]
    fn normalizes_mac_separators_and_case() {
        assert_eq!(normalize_mac("00:1a:79:31:66:30"), "00:1A:79:31:66:30");
        assert_eq!(normalize_mac("00-1a-79-31-66-30"), "00:1A:79:31:66:30");
        assert_eq!(normalize_mac("00.1a.79.31.66.30"), "00:1A:79:31:66:30");
        assert_eq!(normalize_mac("001a79316630"), "00:1A:79:31:66:30");
        assert_eq!(normalize_mac("  00:1A:79:31:66:30  "), "00:1A:79:31:66:30");
    }

    #[test]
    fn serial_cfduid_is_always_32_chars() {
        assert_eq!(serial_cfduid("ABC123").len(), 32);
        assert_eq!(serial_cfduid("").len(), 32);
    }

    #[test]
    fn params_use_encode_uri_component_set_not_full_percent_encoding() {
        let url = build_request_url("http://portal.example.com/server/load.php", &[("type", "itv"), ("action", "get_genres")]);
        // `_` is unreserved for `encodeURIComponent`; the previous
        // `NON_ALPHANUMERIC` set escaped it to `get%5Fgenres`.
        assert!(url.contains("action=get_genres"), "{url}");
        assert!(!url.contains("%5F"), "{url}");
    }

    #[test]
    fn params_still_escape_characters_that_would_break_the_query() {
        let metrics = r#"{"mac":"00:1A:79:31:66:30","model":"MAG250"}"#;
        let url = build_request_url("http://portal.example.com/server/load.php", &[("metrics", metrics)]);
        assert!(!url.contains('{'), "{url}");
        assert!(url.contains("%22"), "{url}"); // "
        assert!(url.contains("%3A"), "{url}"); // :
        assert!(url.contains("%2C"), "{url}"); // ,
    }

    #[test]
    fn identity_values_normalize_blank_and_legacy_placeholder_to_absent() {
        assert_eq!(normalize_identity_value(Some("  ABC  ")), Some("ABC"));
        assert_eq!(normalize_identity_value(Some("   ")), None);
        assert_eq!(normalize_identity_value(None), None);
        assert_eq!(normalize_serial_number(Some(" bedacd4569baf ")), None);
        assert_eq!(normalize_serial_number(Some(LEGACY_DEFAULT_STALKER_SERIAL)), None);
        assert_eq!(normalize_serial_number(Some(" REAL123 ")), Some("REAL123"));
    }

    #[test]
    fn legacy_placeholder_serial_produces_no_sn_header_or_cfduid() {
        let headers = build_api_headers("00:1A:79:31:66:30", Some(LEGACY_DEFAULT_STALKER_SERIAL), None);
        assert!(!headers.iter().any(|(name, _)| name == "SN"));
        let cookie = &headers.iter().find(|(name, _)| name == "Cookie").unwrap().1;
        assert!(!cookie.contains("__cfduid"), "{cookie}");
    }

    #[test]
    fn whitespace_padded_serial_reaches_the_wire_trimmed() {
        let headers = build_api_headers("00:1A:79:31:66:30", Some("  REAL123  "), None);
        let sn = &headers.iter().find(|(name, _)| name == "SN").unwrap().1;
        assert_eq!(sn, "REAL123");
        let cookie = &headers.iter().find(|(name, _)| name == "Cookie").unwrap().1;
        assert!(cookie.contains(&format!("__cfduid={}", serial_cfduid("REAL123"))), "{cookie}");
    }

    #[test]
    fn build_request_url_appends_js_http_request_once() {
        let url = build_request_url(
            "http://portal.example.com/server/load.php?stale=1",
            &[("type", "stb"), ("action", "handshake")],
        );
        assert!(url.starts_with("http://portal.example.com/server/load.php?"));
        assert!(!url.contains("stale=1"));
        assert_eq!(url.matches("JsHttpRequest").count(), 1);
    }
}
