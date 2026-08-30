use crate::db::DbPool;
#[cfg(windows)]
use crate::mpv_player::MpvSession;
use reqwest::Client;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// In-process embedded mpv playback sessions, keyed by session id (a fresh
/// UUID per `mpv_start_session` call, never reused, so a stale session
/// mid-teardown can't be confused with a new one). Windows-only.
#[cfg(windows)]
pub type MpvSessionRegistry = Arc<Mutex<HashMap<String, Arc<MpvSession>>>>;

/// Cooperative control for one in-flight download's background task. The
/// task polls these between chunks rather than being killed outright, so a
/// pause/cancel always lands on a clean chunk boundary instead of leaving a
/// half-written partial file.
pub struct DownloadHandle {
    pub pause: Arc<AtomicBool>,
    pub cancel: Arc<AtomicBool>,
}

pub struct AppState {
    pub db: DbPool,
    pub http: Client,
    /// Handle to the Tauri app so background tasks that finish *after* their
    /// spawning command returned can notify the frontend (`app.emit(...)`).
    /// Motivating case: censored-ITV recovery writes channels minutes after
    /// the channel list rendered; without an event those rows stay invisible
    /// until next app start (which re-deletes them via fast sync first).
    /// `None` only for an `AppState::detached` built without one.
    pub app: Option<tauri::AppHandle>,
    /// Keys (`itv:<playlist>` / `vod:<playlist>:<movie|series>`) of the
    /// censored-category recovery crawls currently in flight. Multiple sync
    /// entry points can spawn one for the same playlist concurrently -
    /// without this dedup, two crawls hit a portal that tolerates ~1
    /// connection at a time and both insert the same rows. `Arc` because
    /// the guard is released by the detached task, which outlives `&AppState`.
    pub recovery_inflight: Arc<Mutex<HashSet<String>>>,
    pub stream_proxy_port: Mutex<Option<u16>>,
    /// Resolved once at startup (`app.path().download_dir()`, falling back
    /// to the app data dir) - `start_download` gets only a bare filename
    /// from the frontend, so this is what it's resolved against.
    pub downloads_dir: PathBuf,
    /// Keyed by download id. A running download's task removes its own entry
    /// on completion/failure/cancel - a missing entry on `pause_download`
    /// just means nothing is in-flight to pause.
    pub downloads: Mutex<HashMap<String, DownloadHandle>>,
    /// Keyed by the session id returned from `spawn_external_player`. Holding
    /// the live `Child` is what makes `kill_player`/`get_player_status`
    /// possible - there is no other handle to a spawned mpv/vlc process.
    pub players: Mutex<HashMap<String, tokio::process::Child>>,
    /// In-process embedded mpv playback sessions - see `MpvSessionRegistry`.
    #[cfg(windows)]
    pub mpv_sessions: MpvSessionRegistry,
    /// Serializes `mpv_start_session` end-to-end (whole command body, not
    /// just the registry check) - load-bearing, not defensive caution: near-
    /// simultaneous calls each found an empty registry (none had reached
    /// `insert` yet) and each spawned its own independent mpv session,
    /// causing a real resource-exhaustion crash. Must be an async
    /// `tokio::sync::Mutex` (held across `.await`), not a std one, so each
    /// call fully finishes (including registering itself) before the next
    /// begins its stale-session check.
    #[cfg(windows)]
    pub mpv_start_lock: tokio::sync::Mutex<()>,
}

impl AppState {
    /// Builds an `AppState` for code outside a Tauri command (the scheduler,
    /// a detached VOD sync spawn) with no real `State<'_, AppState>` to draw
    /// from. Only `db`/`http` are read by those code paths, so the
    /// download/player fields are harmless unused placeholders here.
    pub fn detached(db: DbPool, http: Client, app: Option<tauri::AppHandle>) -> Self {
        AppState {
            db,
            http,
            app,
            recovery_inflight: Arc::new(Mutex::new(HashSet::new())),
            stream_proxy_port: Mutex::new(None),
            downloads_dir: PathBuf::new(),
            downloads: Mutex::new(HashMap::new()),
            players: Mutex::new(HashMap::new()),
            #[cfg(windows)]
            mpv_sessions: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(windows)]
            mpv_start_lock: tokio::sync::Mutex::new(()),
        }
    }
}

/// Separate from `AppState`: the stream proxy is an axum router, not a Tauri
/// command, so it just shares the cheap `Arc`-backed clones it needs rather
/// than wiring Tauri's state manager into axum.
pub struct ProxyState {
    pub db: DbPool,
    pub http: Client,
    /// The proxy's own port, set once `start()` knows it (after binding).
    /// Needed to rewrite HLS manifest URIs back into `/stream?url=...` links
    /// that point at this same server — see `stream_proxy.rs`'s manifest
    /// rewriting for why the proxy has to know its own address.
    pub port: u16,
    /// Short-TTL memoization of `resolve_auth_headers`'s DB-backed lookup,
    /// keyed by `(playlist_id, target_url)`. A live HLS manifest refetch or
    /// mpegts.js reconnect reuses the same URL every few seconds, which was
    /// re-running a full DB round trip each time for unchanged headers.
    pub headers_cache: Mutex<HashMap<(String, String), (Vec<(String, String)>, Instant)>>,
}
