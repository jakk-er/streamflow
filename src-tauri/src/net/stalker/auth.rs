use super::identity::{self, STB_PROFILE_PARAMS};
use crate::error::{CommandError, CommandResult};
use crate::types::{StalkerAuthOutcome, StalkerSessionInfo};
use regex::Regex;
use reqwest::{Client, Url};
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Clone, Copy)]
pub struct StalkerCredentials<'a> {
    pub portal_url: &'a str,
    pub mac_address: &'a str,
    pub serial_number: Option<&'a str>,
    pub device_id: Option<&'a str>,
    pub device_id2: Option<&'a str>,
    pub signature1: Option<&'a str>,
    pub signature2: Option<&'a str>,
}

/// Identity fields are read back through `identity`'s normalizers, never off
/// the struct directly - the portal pins `device_id`/`device_id2` to the MAC
/// on first sight and refuses a later mismatch as a device conflict, so a
/// stray space or the legacy placeholder serial must never reach the wire as
/// a real value. Mirrors the reference client's `normalizeStalkerPortalIdentity()`.
impl<'a> StalkerCredentials<'a> {
    pub fn serial(&self) -> Option<&'a str> {
        identity::normalize_serial_number(self.serial_number)
    }

    pub fn device_id(&self) -> Option<&'a str> {
        identity::normalize_identity_value(self.device_id)
    }

    pub fn device_id2(&self) -> Option<&'a str> {
        identity::normalize_identity_value(self.device_id2)
    }

    pub fn signature1(&self) -> Option<&'a str> {
        identity::normalize_identity_value(self.signature1)
    }

    pub fn signature2(&self) -> Option<&'a str> {
        identity::normalize_identity_value(self.signature2)
    }

    /// The same identity pointed at a different endpoint — how discovery
    /// carries the user's full identity across each candidate it probes.
    pub fn with_portal_url(&self, portal_url: &'a str) -> Self {
        Self { portal_url, ..*self }
    }
}

/// Retries a fresh-connection transport failure with a short backoff. Some
/// portals sit behind a CDN that rate-limits new-connection RATE per IP (not
/// request shape) - bursting several fresh connections within a few hundred
/// ms, as `discover_portal_endpoint`'s probing plus the handshake chain can,
/// triggers a `ConnectionReset`. ~1.5s spacing avoids it reliably, so delays
/// widen across three attempts to clear that threshold by the third (~2.7s).
async fn send_with_retry(
    http: &Client,
    url: &str,
    headers: &[(String, String)],
    timeout_secs: u64,
) -> CommandResult<reqwest::Response> {
    const RETRY_DELAYS_MS: [u64; 3] = [500, 900, 1300];
    let mut attempt = 0usize;
    loop {
        let mut request = http.get(url).timeout(Duration::from_secs(timeout_secs));
        for (name, value) in headers {
            request = request.header(name.as_str(), value.as_str());
        }
        match request.send().await {
            Ok(response) => return Ok(response),
            Err(e) if attempt < RETRY_DELAYS_MS.len() => {
                // `{e}` (Display) hides the actual cause (DNS/reset/TLS/
                // timeout) - only `{:?}` (Debug) includes the source chain.
                tracing::warn!(
                    "Stalker request to {url} failed (attempt {}/{}), retrying: {e:?}",
                    attempt + 1,
                    RETRY_DELAYS_MS.len() + 1
                );
                tokio::time::sleep(Duration::from_millis(RETRY_DELAYS_MS[attempt])).await;
                attempt += 1;
            }
            Err(e) => {
                tracing::warn!("Stalker request to {url} failed (final attempt): {e:?}");
                return Err(CommandError::Api(
                    "Couldn't reach the Stalker portal. Check the server address and your internet connection.".into(),
                ));
            }
        }
    }
}

/// Low-level GET with the standard identity headers. Auth failures on real
/// portals arrive as HTTP 200 with a plain-text body ("Authorization
/// failed.", "Access denied.", "Unauthorized request."), not a 4xx/error
/// status, so both shapes are handled here (4xx/5xx handled defensively too).
pub(crate) async fn stalker_get(http: &Client, url: &str, headers: &[(String, String)], timeout_secs: u64) -> CommandResult<Value> {
    stalker_get_inner(http, url, headers, timeout_secs, true).await
}

/// `stalker_get` without the unparseable-body warning. Endpoint discovery
/// probes candidate paths *expected* not to exist (e.g. a 404 HTML page),
/// and logging each at WARN made healthy discovery look like a failure.
/// Classification still sees the body; only the tracing is suppressed.
async fn stalker_get_quiet(http: &Client, url: &str, headers: &[(String, String)], timeout_secs: u64) -> CommandResult<Value> {
    stalker_get_inner(http, url, headers, timeout_secs, false).await
}

async fn stalker_get_inner(
    http: &Client,
    url: &str,
    headers: &[(String, String)],
    timeout_secs: u64,
    log_unparseable: bool,
) -> CommandResult<Value> {
    let response = send_with_retry(http, url, headers, timeout_secs).await?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| CommandError::Api(format!("Failed to read the portal's response: {e}")))?;

    if status.as_u16() >= 500 {
        tracing::warn!("Stalker portal at {url} returned status {}", status.as_u16());
        return Err(CommandError::Api(format!(
            "The Stalker portal returned an error (code {}). It may be temporarily down.",
            status.as_u16()
        )));
    }

    match serde_json::from_str::<Value>(&text) {
        Ok(json) => Ok(json),
        Err(parse_err) => Err(classify_plaintext_failure(url, &text, &parse_err, log_unparseable)),
    }
}

/// Whole-body-anchored classification (never substring match — a WAF/proxy
/// error page happening to contain one of these phrases mid-sentence must
/// not be misclassified as a real portal auth failure).
fn classify_plaintext_failure(url: &str, body: &str, parse_err: &serde_json::Error, log_unparseable: bool) -> CommandError {
    let trimmed = body.trim();
    if trimmed.eq_ignore_ascii_case("Authorization failed.") || trimmed.starts_with("Authorization failed.") {
        CommandError::Auth("The portal session expired or was rejected — try logging in again.".into())
    } else if trimmed.eq_ignore_ascii_case("Access denied.") {
        CommandError::Auth("This account has been blocked by the IPTV provider.".into())
    } else if trimmed.eq_ignore_ascii_case("Unauthorized request.") {
        CommandError::Auth("The portal rejected this request (missing device identity).".into())
    } else {
        // The body is the most useful diagnostic (HTML often means the URL
        // is missing its endpoint path). Logged, not shown to the user -
        // it can be raw HTML/binary.
        if log_unparseable {
            let preview: String = trimmed.chars().take(500).collect();
            tracing::warn!("Stalker portal at {url} returned an unparseable body (json error: {parse_err}): {preview:?}");
        }
        CommandError::InvalidResponse("The Stalker portal sent back a response we couldn't understand.".into())
    }
}

/// Wider phrase set than `classify_plaintext_failure`, which only sees a
/// body AFTER JSON parsing has failed - this recognizes a *successfully
/// parsed* auth-failure envelope like `{"js":{"error":"Authorization
/// failed"}}` or bare `{"js":"Unauthorized"}`, both sent with a clean 200.
/// Used by probe classification to tell "needs a real handshake" from
/// "not a Stalker endpoint at all."
fn is_json_auth_failure(body: &Value) -> bool {
    fn matches_failure_phrase(s: &str) -> bool {
        let re = {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| {
                Regex::new(r"(?i)authorization\s+failed|access\s+denied|unauthorized\s+request|auth\s+failed|invalid\s+token|\bunauthorized\b|authorization").unwrap()
            })
        };
        let trimmed = s.trim();
        !trimmed.is_empty() && trimmed.len() <= 200 && re.is_match(trimmed)
    }

    let Some(js) = body.get("js") else { return false };
    if let Some(s) = js.as_str() {
        return matches_failure_phrase(s);
    }
    if let Some(obj) = js.as_object() {
        for key in ["error", "msg"] {
            if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
                if matches_failure_phrase(s) {
                    return true;
                }
            }
        }
    }
    false
}

enum ProbeClassification {
    /// A real, playable content response — this candidate is a working,
    /// token-free ("simple") Stalker portal.
    Data,
    /// The candidate is a real Stalker endpoint but rejected the token-less
    /// probe — needs the full handshake flow confirmed before trusting it.
    AuthRequired,
    /// Doesn't look like a Stalker API response at all (wrong path, a 404
    /// page, an unrelated JSON API, ...) — try the next candidate.
    NotAPortal,
}

/// A bare `js` key isn't enough - some portals answer 200 with
/// `{js:{error:"Unknown action"}}` or `{js:false}` for an unrecognized path.
/// Only `js` being an array, or `js.data` an array with no `js.error`,
/// counts as real data.
fn classify_probe_response(body: &Value) -> ProbeClassification {
    if is_json_auth_failure(body) {
        return ProbeClassification::AuthRequired;
    }
    if let Some(js) = body.get("js") {
        if js.is_array() {
            return ProbeClassification::Data;
        }
        if let Some(obj) = js.as_object() {
            if matches!(obj.get("data"), Some(Value::Array(_))) && !obj.contains_key("error") {
                return ProbeClassification::Data;
            }
        }
    }
    ProbeClassification::NotAPortal
}

/// Builds candidate API endpoints in probe order: an explicit `.php`
/// endpoint the user pasted wins outright; otherwise `portal.php` ->
/// `server/load.php` -> `stalker_portal/server/load.php`. The `/c` landing
/// page path (the web-player shell, not the API) is stripped first.
fn build_endpoint_candidates(raw_url: &str) -> Vec<String> {
    let trimmed = raw_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Vec::new();
    }
    let Ok(mut parsed) = Url::parse(trimmed) else {
        return Vec::new();
    };
    parsed.set_query(None);
    parsed.set_fragment(None);

    let path = parsed.path().trim_end_matches('/').to_string();
    let ends_with_php = path.to_lowercase().ends_with(".php");

    let mut candidates = Vec::new();
    if ends_with_php {
        candidates.push(with_path(&parsed, &path));
    }

    let base = if ends_with_php {
        path.rsplit_once('/').map(|(p, _)| p.to_string()).unwrap_or_default()
    } else if path.to_lowercase().ends_with("/c") {
        path[..path.len() - 2].to_string()
    } else {
        path.clone()
    };

    candidates.push(with_path(&parsed, &format!("{base}/portal.php")));
    candidates.push(with_path(&parsed, &format!("{base}/server/load.php")));
    if !base.to_lowercase().contains("/stalker_portal") {
        candidates.push(with_path(&parsed, &format!("{base}/stalker_portal/server/load.php")));
    }

    let mut seen = std::collections::HashSet::new();
    candidates.retain(|c| seen.insert(c.clone()));
    candidates
}

fn with_path(base: &Url, path: &str) -> String {
    let mut url = base.clone();
    url.set_path(path);
    url.to_string()
}

/// A token-less, serial-less `itv/get_genres` probe - the "bare" shape a
/// real token-free portal accepts. No `Authorization`/`SN`/`__cfduid`; a
/// full portal's identity requirements are confirmed separately via a real
/// handshake, only once this probe proves the candidate needs one.
async fn probe_candidate(http: &Client, candidate: &str, mac_address: &str) -> CommandResult<Value> {
    let headers = identity::build_api_headers(mac_address, None, None);
    let url = identity::build_request_url(candidate, &[("type", "itv"), ("action", "get_genres")]);
    stalker_get_quiet(http, &url, &headers, 10).await
}

/// Runs the real handshake + `get_profile` against one candidate to confirm
/// it's a genuine full-portal endpoint (only after the token-less probe came
/// back auth-required - never speculatively, since it's a heavier round trip).
///
/// **Must use the user's REAL identity in full, not a partial one.**
/// `get_profile` permanently binds `device_id` to the MAC (first non-empty
/// value wins, a later mismatch is a device conflict, a later empty value is
/// a lockout) - an earlier revision confirmed with MAC+serial only then
/// re-authenticated with the full identity, and the device-id-less first
/// call poisoned the binding for anyone with advanced identity fields set.
///
/// Returns the outcome as-is, including refusals - the caller keeps it as a
/// fallback so the portal's own words survive when no candidate resolves.
async fn confirm_full_portal(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    stored: StoredStalkerSession<'_>,
    login: Option<&str>,
    password: Option<&str>,
) -> Option<StalkerAuthOutcome> {
    authenticate(http, creds, stored, login, password).await.ok()
}

/// What `discover_portal_endpoint` resolved: the working API endpoint, the
/// portal's observed mode, and — for a full portal — the session its own
/// confirming handshake established.
pub struct DiscoveredPortal {
    pub endpoint: String,
    pub full_portal: bool,
    /// The session discovery already negotiated. `None` only for a simple
    /// (token-free) portal. Reuse this rather than calling `authenticate()`
    /// again - a second handshake invalidates the token, and a second
    /// `get_profile` re-runs device-id binding (see `confirm_full_portal`).
    pub outcome: Option<StalkerAuthOutcome>,
}

/// Finds the actual API script (not the bare portal URL, which serves an
/// HTML web-player shell). Probes `creds.portal_url`'s candidates in order;
/// for each, a token-less `itv/get_genres` request decides the mode: real
/// data means a working token-free ("simple") portal, resolved immediately
/// with no handshake. An auth-failure shape means it's real but needs the
/// full handshake, run with the user's complete identity - the resulting
/// session is returned, not discarded. Anything else moves to the next
/// candidate.
///
/// `stored` lets an edit re-present the existing token: if identity is
/// unchanged, the handshake echoes it back and `get_profile` is skipped, so
/// renaming a playlist can't disturb a pinned device binding.
pub async fn discover_portal_endpoint(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    stored: StoredStalkerSession<'_>,
    login: Option<&str>,
    password: Option<&str>,
) -> CommandResult<DiscoveredPortal> {
    let candidates = build_endpoint_candidates(creds.portal_url);
    if candidates.is_empty() {
        return Err(CommandError::Api("Invalid portal URL".into()));
    }

    // First endpoint that proved real but refused this identity, kept as a
    // fallback - without it, a device conflict/blocked account on the only
    // working candidate surfaced as a generic "no endpoint found" instead of
    // the actionable reason. Mirrors the reference client's `authRejection`.
    let mut refusal: Option<(String, StalkerAuthOutcome)> = None;

    for candidate in &candidates {
        let probe_result = probe_candidate(http, candidate, creds.mac_address).await;
        let classification = match &probe_result {
            Ok(json) => classify_probe_response(json),
            Err(CommandError::Auth(_)) => ProbeClassification::AuthRequired,
            Err(_) => ProbeClassification::NotAPortal,
        };

        match classification {
            ProbeClassification::Data => {
                return Ok(DiscoveredPortal {
                    endpoint: candidate.clone(),
                    full_portal: false,
                    outcome: None,
                })
            }
            ProbeClassification::AuthRequired => {
                let candidate_creds = creds.with_portal_url(candidate);
                let Some(outcome) = confirm_full_portal(http, &candidate_creds, stored, login, password).await else {
                    continue;
                };
                // `LoginRequired` resolves the endpoint too - the frontend
                // re-prompts and calls `stalker_do_auth` against it.
                if matches!(outcome, StalkerAuthOutcome::Success { .. } | StalkerAuthOutcome::LoginRequired) {
                    return Ok(DiscoveredPortal {
                        endpoint: candidate.clone(),
                        full_portal: true,
                        outcome: Some(outcome),
                    });
                }
                if refusal.is_none() {
                    refusal = Some((candidate.clone(), outcome));
                }
            }
            ProbeClassification::NotAPortal => {}
        }
    }

    if let Some((endpoint, outcome)) = refusal {
        return Ok(DiscoveredPortal { endpoint, full_portal: true, outcome: Some(outcome) });
    }

    Err(CommandError::Api(format!(
        "Couldn't find a working Stalker API endpoint at {}. Try entering the exact portal URL your provider gave you, including the /server/load.php or /portal.php path.",
        creds.portal_url
    )))
}

struct HandshakeResult {
    token: String,
    random: String,
    not_valid: bool,
}

async fn handshake(http: &Client, creds: &StalkerCredentials<'_>, stored_token: Option<&str>) -> CommandResult<HandshakeResult> {
    // No `Authorization` header here - a candidate token travels only in the
    // `token=` query param below, since there's no session yet to bear.
    let headers = identity::build_api_headers(creds.mac_address, creds.serial(), None);
    let prehash = identity::prehash(creds.mac_address);
    let url = identity::build_request_url(
        creds.portal_url,
        &[
            ("type", "stb"),
            ("action", "handshake"),
            ("token", stored_token.unwrap_or("")),
            ("prehash", &prehash),
        ],
    );
    let body = stalker_get(http, &url, &headers, 15).await?;
    let token = body
        .pointer("/js/token")
        .and_then(|v| v.as_str())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .ok_or_else(|| CommandError::Auth("The portal's handshake didn't return a session token.".into()))?;
    // Real STB clients echo the handshake's own `random` back in
    // `get_profile`'s `metrics` JSON — some portals validate its presence,
    // so a client-generated fallback is used only when the portal omitted it.
    let random = body
        .pointer("/js/random")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
    let not_valid = to_finite_i64(body.pointer("/js/not_valid")).unwrap_or(0) == 1;
    Ok(HandshakeResult { token, random, not_valid })
}

enum ProfileStatus {
    Ok { watchdog_timeout: i64, timeslot: i64 },
    LoginRequired,
    DeviceConflict(String),
    Blocked(String),
}

/// Reads a field that's semantically a number but may arrive as a JSON
/// number OR a string (`"status": "2"`). `Value::as_i64()` returns `None`
/// for a string, which made `classify_profile` treat a stringified refusal
/// status as a healthy profile. Mirrors the reference's `toFiniteNumber()`.
fn to_finite_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().filter(|f| f.is_finite()).map(|f| f as i64)),
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return None;
            }
            trimmed
                .parse::<i64>()
                .ok()
                .or_else(|| trimmed.parse::<f64>().ok().filter(|f| f.is_finite()).map(|f| f as i64))
        }
        _ => None,
    }
}

/// `block_msg` routinely carries markup ("Your STB is damaged.<br/> Call the
/// provider."), and it reaches the user verbatim — strip tags and collapse
/// whitespace first. Mirrors `stripStalkerPortalMarkup`.
fn strip_portal_markup(text: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"<[^>]*>").unwrap());
    re.replace_all(text, " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Combines `msg`/`block_msg` into one plain-text line - both are carried
/// since panels split the reason across them; duplicates are de-duplicated.
fn combine_portal_messages(msg: Option<&str>, block_msg: Option<&str>) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for raw in [msg, block_msg].into_iter().flatten() {
        let cleaned = strip_portal_markup(raw);
        if !cleaned.is_empty() && !parts.contains(&cleaned) {
            parts.push(cleaned);
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" \u{2014} "))
    }
}

fn device_conflict_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        vec![
            Regex::new(r"(?i)device\s*conflict").unwrap(),
            Regex::new(r"(?i)device[\s_-]?id.{0,40}?(mismatch|conflict|does\s*not\s*match|not\s*match)").unwrap(),
        ]
    })
}

/// True when a `status: 1` refusal reports this MAC bound to a different
/// device ID - the one refusal with a concrete remedy, worth splitting from
/// a generic "blocked". Deliberately narrow: looser keywords like "already"
/// also match unrelated refusals (e.g. "device limit reached").
fn is_device_conflict_message(msg: &str) -> bool {
    !msg.is_empty() && device_conflict_regexes().iter().any(|re| re.is_match(msg))
}

/// Decodes `js.status` into the four outcomes auth branches on: `status: 2`
/// asks for `do_auth`; a refusal is `status: 1` OR any portal-written
/// `msg`/`block_msg` (a panel that explains itself has refused, whatever
/// `status` says). Read through `to_finite_i64` so a stringified `"1"`/`"2"`
/// isn't mistaken for healthy.
fn classify_profile(body: &Value) -> ProfileStatus {
    let js = body.pointer("/js").unwrap_or(body);
    let status = to_finite_i64(js.get("status"));
    let portal_text = combine_portal_messages(
        js.get("msg").and_then(|v| v.as_str()),
        js.get("block_msg").and_then(|v| v.as_str()),
    );

    if status == Some(2) {
        return ProfileStatus::LoginRequired;
    }

    if status == Some(1) || portal_text.is_some() {
        let message = portal_text.unwrap_or_else(|| "The portal rejected this device.".into());
        return if is_device_conflict_message(&message) {
            ProfileStatus::DeviceConflict(message)
        } else {
            ProfileStatus::Blocked(message)
        };
    }

    match status {
        // A healthy profile answers `status: 0`, and plenty of panels omit
        // the field entirely on success.
        None | Some(0) => {
            let watchdog_timeout = to_finite_i64(js.get("watchdog_timeout")).unwrap_or(120).clamp(30, 3600);
            let timeslot = to_finite_i64(js.get("timeslot")).unwrap_or(0);
            ProfileStatus::Ok { watchdog_timeout, timeslot }
        }
        // An unrecognized non-zero status with nothing to explain it.
        Some(_) => ProfileStatus::Blocked("The portal rejected this device.".into()),
    }
}

#[allow(clippy::too_many_arguments)]
async fn get_profile(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    token: &str,
    random: &str,
    not_valid_token: bool,
    auth_second_step: bool,
) -> CommandResult<Value> {
    let headers = identity::build_api_headers(creds.mac_address, creds.serial(), Some(token));
    let prehash = identity::prehash(creds.mac_address);
    // Real STB clients include `sn` inside `metrics` alongside mac/model/type
    // (some portals validate its presence) — `get_profile` is the only
    // action that carries `sn` at all, in the query string AND `metrics`.
    let metrics = if let Some(serial) = creds.serial() {
        serde_json::json!({
            "mac": creds.mac_address,
            "model": "MAG250",
            "type": "STB",
            "random": random,
            "sn": serial,
        })
    } else {
        serde_json::json!({
            "mac": creds.mac_address,
            "model": "MAG250",
            "type": "STB",
            "random": random,
        })
    }
    .to_string();

    let mut params: Vec<(&str, &str)> = vec![("type", "stb"), ("action", "get_profile")];
    params.extend_from_slice(STB_PROFILE_PARAMS);
    params.push(("metrics", &metrics));
    params.push(("not_valid_token", if not_valid_token { "1" } else { "0" }));
    let auth_second_step_val = if auth_second_step { "1" } else { "0" };
    params.push(("auth_second_step", auth_second_step_val));
    if let Some(serial) = creds.serial() {
        params.push(("sn", serial));
    }
    if let Some(device_id) = creds.device_id() {
        params.push(("device_id", device_id));
    }
    if let Some(device_id2) = creds.device_id2() {
        params.push(("device_id2", device_id2));
    }
    if let Some(signature1) = creds.signature1() {
        params.push(("signature", signature1));
    }
    if let Some(signature2) = creds.signature2() {
        params.push(("signature2", signature2));
    }
    params.push(("prehash", &prehash));

    let url = identity::build_request_url(creds.portal_url, &params);
    stalker_get(http, &url, &headers, 15).await
}

async fn do_auth(http: &Client, creds: &StalkerCredentials<'_>, token: &str, login: &str, password: &str) -> CommandResult<bool> {
    let headers = identity::build_api_headers(creds.mac_address, creds.serial(), Some(token));
    let mut params: Vec<(&str, &str)> = vec![("type", "stb"), ("action", "do_auth"), ("login", login), ("password", password)];
    if let Some(device_id) = creds.device_id() {
        params.push(("device_id", device_id));
    }
    if let Some(device_id2) = creds.device_id2() {
        params.push(("device_id2", device_id2));
    }
    let url = identity::build_request_url(creds.portal_url, &params);
    let body = stalker_get(http, &url, &headers, 15).await?;
    Ok(body.pointer("/js").and_then(|v| v.as_bool()).unwrap_or(false))
}

#[allow(clippy::too_many_arguments)]
fn build_session_info(
    portal_url: &str,
    token: &str,
    watchdog_timeout: i64,
    timeslot: i64,
    not_valid: bool,
    fingerprint: &str,
) -> StalkerSessionInfo {
    StalkerSessionInfo {
        token: token.to_string(),
        endpoint: portal_url.to_string(),
        full_portal: true,
        watchdog_timeout,
        timeslot,
        not_valid,
        login_completed: true,
        session_fingerprint: fingerprint.to_string(),
    }
}

/// A stable, opaque, local-only fingerprint of everything that identifies
/// *which* session a stored token belongs to: portal endpoint, full device
/// identity, and credentials. Only ever compared against a fingerprint this
/// same function previously produced, never against iptvnator's own value,
/// so exact serialization doesn't need to match theirs.
fn session_fingerprint(creds: &StalkerCredentials<'_>, username: Option<&str>, password: Option<&str>) -> String {
    fn norm(v: Option<&str>) -> &str {
        v.map(str::trim).filter(|s| !s.is_empty()).unwrap_or("")
    }
    // Same normalizers the request builders use, so two configs that put
    // identical bytes on the wire never fingerprint differently (e.g. blank
    // vs. legacy-placeholder serial) and needlessly discard a live token.
    let parts = [
        creds.portal_url,
        creds.mac_address,
        creds.serial().unwrap_or(""),
        creds.device_id().unwrap_or(""),
        creds.device_id2().unwrap_or(""),
        creds.signature1().unwrap_or(""),
        creds.signature2().unwrap_or(""),
        norm(username),
        norm(password),
    ];
    serde_json::to_string(&parts).unwrap_or_default()
}

/// The previously-persisted session state a caller has on hand, if any —
/// bundled together since the token-reuse shortcut in `authenticate()` needs
/// all four fields to agree before it can skip `get_profile`.
#[derive(Default, Clone, Copy)]
pub struct StoredStalkerSession<'a> {
    pub token: Option<&'a str>,
    pub fingerprint: Option<&'a str>,
    pub watchdog_timeout: Option<i64>,
    pub timeslot: Option<i64>,
}

/// Full handshake -> get_profile -> (do_auth if needed) orchestration.
/// `login`/`password` are only used if the portal answers `status: 2` (most
/// portals are MAC-only) - matches the frontend's `stalker_auth` vs.
/// `stalker_do_auth` split.
///
/// **Token-reuse shortcut**: if `stored`'s fingerprint matches the current
/// credentials, its token is offered to the handshake; if the portal echoes
/// it back valid and the caller already has a known watchdog cadence,
/// `get_profile` is skipped entirely and that cadence is reused as-is. A
/// fingerprint mismatch withholds the candidate token, forcing a fresh
/// session.
pub async fn authenticate(
    http: &Client,
    creds: &StalkerCredentials<'_>,
    stored: StoredStalkerSession<'_>,
    login: Option<&str>,
    password: Option<&str>,
) -> CommandResult<StalkerAuthOutcome> {
    let fingerprint = session_fingerprint(creds, login, password);
    let fingerprint_matches = stored.fingerprint.is_some_and(|f| f == fingerprint);
    let candidate_token = if fingerprint_matches { stored.token } else { None };

    let handshake_result = handshake(http, creds, candidate_token).await?;
    let token = handshake_result.token;

    if let (Some(candidate), Some(watchdog_timeout)) = (candidate_token, stored.watchdog_timeout) {
        if !handshake_result.not_valid && token == candidate {
            return Ok(StalkerAuthOutcome::Success {
                session: build_session_info(
                    creds.portal_url,
                    &token,
                    watchdog_timeout,
                    stored.timeslot.unwrap_or(0),
                    false,
                    &fingerprint,
                ),
            });
        }
    }

    let profile = get_profile(http, creds, &token, &handshake_result.random, handshake_result.not_valid, false).await?;

    match classify_profile(&profile) {
        ProfileStatus::Ok { watchdog_timeout, timeslot } => Ok(StalkerAuthOutcome::Success {
            session: build_session_info(creds.portal_url, &token, watchdog_timeout, timeslot, handshake_result.not_valid, &fingerprint),
        }),
        ProfileStatus::LoginRequired => {
            let (Some(login), Some(password)) = (login, password) else {
                return Ok(StalkerAuthOutcome::LoginRequired);
            };
            if !do_auth(http, creds, &token, login, password).await? {
                return Ok(StalkerAuthOutcome::LoginRejected { message: "Invalid username or password.".into() });
            }
            let profile2 = get_profile(http, creds, &token, &handshake_result.random, handshake_result.not_valid, true).await?;
            match classify_profile(&profile2) {
                ProfileStatus::Ok { watchdog_timeout, timeslot } => Ok(StalkerAuthOutcome::Success {
                    session: build_session_info(
                        creds.portal_url,
                        &token,
                        watchdog_timeout,
                        timeslot,
                        handshake_result.not_valid,
                        &fingerprint,
                    ),
                }),
                ProfileStatus::LoginRequired => Ok(StalkerAuthOutcome::LoginRejected {
                    message: "The portal still requires login after authenticating.".into(),
                }),
                ProfileStatus::DeviceConflict(message) => Ok(StalkerAuthOutcome::DeviceConflict { message }),
                ProfileStatus::Blocked(message) => Ok(StalkerAuthOutcome::Blocked { message }),
            }
        }
        ProfileStatus::DeviceConflict(message) => Ok(StalkerAuthOutcome::DeviceConflict { message }),
        ProfileStatus::Blocked(message) => Ok(StalkerAuthOutcome::Blocked { message }),
    }
}

pub async fn watchdog_ping(http: &Client, creds: &StalkerCredentials<'_>, token: &str, init: bool) -> CommandResult<()> {
    let headers = identity::build_api_headers(creds.mac_address, creds.serial(), Some(token));
    let init_val = if init { "1" } else { "0" };
    let url = identity::build_request_url(
        creds.portal_url,
        &[
            ("type", "watchdog"),
            ("action", "get_events"),
            ("event_active_id", "0"),
            ("cur_play_type", "0"),
            ("init", init_val),
        ],
    );
    // A missed ping only affects the portal's own "online" admin display,
    // never the session itself — failures are logged, not surfaced.
    if let Err(e) = stalker_get(http, &url, &headers, 10).await {
        tracing::warn!("Stalker watchdog ping failed (non-fatal): {e}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_strip_trailing_c_landing_page_and_try_portal_php_first() {
        let candidates = build_endpoint_candidates("http://vod.supremetv.live:80/c/");
        assert_eq!(
            candidates,
            vec![
                "http://vod.supremetv.live/portal.php",
                "http://vod.supremetv.live/server/load.php",
                "http://vod.supremetv.live/stalker_portal/server/load.php",
            ]
        );
    }

    #[test]
    fn candidates_keep_pasted_php_endpoint_first_and_probe_siblings() {
        let candidates = build_endpoint_candidates("http://panel.example.com/cp/api.php");
        assert_eq!(candidates[0], "http://panel.example.com/cp/api.php");
        assert!(candidates.contains(&"http://panel.example.com/cp/portal.php".to_string()));
    }

    #[test]
    fn candidates_skip_nested_stalker_portal_when_base_already_has_it() {
        let candidates = build_endpoint_candidates("http://host.example.com/stalker_portal");
        assert!(!candidates.iter().any(|c| c.contains("stalker_portal/stalker_portal")));
    }

    #[test]
    fn device_conflict_regex_matches_reference_phrasing_only() {
        assert!(is_device_conflict_message("Device ID conflict detected"));
        assert!(is_device_conflict_message("device_id does not match"));
        assert!(!is_device_conflict_message("Your account has been blocked"));
        assert!(!is_device_conflict_message("device limit reached"));
    }

    #[test]
    fn classifies_array_js_as_data() {
        let body = serde_json::json!({"js": [{"id": "1", "title": "News"}]});
        assert!(matches!(classify_probe_response(&body), ProbeClassification::Data));
    }

    #[test]
    fn classifies_data_array_with_no_error_as_data() {
        let body = serde_json::json!({"js": {"data": [{"id": "1"}], "total_items": 1}});
        assert!(matches!(classify_probe_response(&body), ProbeClassification::Data));
    }

    #[test]
    fn classifies_error_envelope_as_auth_required_not_data() {
        // A portal answering 200 with an error envelope for an unrecognized
        // action must not be mistaken for real genre data.
        let body = serde_json::json!({"js": {"error": "Authorization failed", "data": []}});
        assert!(matches!(classify_probe_response(&body), ProbeClassification::AuthRequired));
    }

    #[test]
    fn classifies_unrecognized_shape_as_not_a_portal() {
        let body = serde_json::json!({"status": "ok"});
        assert!(matches!(classify_probe_response(&body), ProbeClassification::NotAPortal));
        let body2 = serde_json::json!({"js": false});
        assert!(matches!(classify_probe_response(&body2), ProbeClassification::NotAPortal));
    }

    #[test]
    fn json_auth_failure_matches_bare_js_string_and_structured_msg() {
        assert!(is_json_auth_failure(&serde_json::json!({"js": "Unauthorized"})));
        assert!(is_json_auth_failure(&serde_json::json!({"js": {"msg": "Invalid token"}})));
        assert!(!is_json_auth_failure(&serde_json::json!({"js": {"data": []}})));
    }

    #[test]
    fn session_fingerprint_changes_with_identity_or_credentials() {
        let creds_a = StalkerCredentials {
            portal_url: "http://host/portal.php",
            mac_address: "00:1A:79:31:66:30",
            serial_number: None,
            device_id: None,
            device_id2: None,
            signature1: None,
            signature2: None,
        };
        let mut creds_b = StalkerCredentials { mac_address: "00:1A:79:31:66:31", ..creds_a };
        let fp_a = session_fingerprint(&creds_a, Some("user"), Some("pass"));
        let fp_b = session_fingerprint(&creds_b, Some("user"), Some("pass"));
        assert_ne!(fp_a, fp_b);

        creds_b.mac_address = creds_a.mac_address;
        let fp_a_again = session_fingerprint(&creds_a, Some("user"), Some("pass"));
        assert_eq!(fp_a, fp_a_again);
        let fp_diff_password = session_fingerprint(&creds_a, Some("user"), Some("different"));
        assert_ne!(fp_a, fp_diff_password);
    }
}
