//! One playback session: an `Mpv` core handle pointed at a native window via
//! `wid`, plus an event-observer thread reading back position/duration/
//! pause/eof for `PlayerControls.svelte`'s polling. No render thread or GL
//! context of our own - mpv draws into the window itself. Sessions are
//! self-contained so an overlapping teardown during a fast frontend
//! engine-cascade retry (fresh UUID per session) is safe, just wasteful.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use libmpv2::events::PropertyData;
use libmpv2::{Format, Mpv};
use serde::Serialize;

use super::window::{self, SendHwnd};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionStatus {
    Loading,
    Playing,
    Paused,
    Ended,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedMpvSession {
    pub id: String,
    pub title: String,
    pub stream_url: String,
    pub status: SessionStatus,
    pub position_seconds: f64,
    pub duration_seconds: Option<f64>,
    pub volume: f64,
    pub started_at: String,
    pub updated_at: String,
    pub error: Option<String>,
}

pub type SharedState = Arc<Mutex<EmbeddedMpvSession>>;

pub struct MpvSession {
    mpv: Arc<Mpv>,
    state: SharedState,
    hwnd: SendHwnd,
    event_stop: Arc<AtomicBool>,
    event_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn touch(state: &SharedState) {
    state.lock().unwrap().updated_at = now_rfc3339();
}

impl MpvSession {
    /// `hwnd` must already be a live window created via `window::create` on
    /// the main thread (see `commands/mpv.rs`'s `on_main_thread`). Only
    /// builds the `Mpv` core and points it at that window via `wid` - never
    /// touches Win32 itself, so this can run on any thread.
    pub fn start(id: String, url: String, title: String, hwnd: SendHwnd, start_position_seconds: Option<f64>) -> Result<Arc<Self>, String> {
        let hwnd_value = (hwnd.0).0 as i64;
        let mpv = Arc::new(
            Mpv::with_initializer(|init| {
                // The property that makes this "embedded" - mpv renders
                // into this window (vo=gpu) instead of its own top-level one.
                init.set_property("wid", hwnd_value).map_err(libmpv2::Error::from)?;
                init.set_property("keep-open", "yes")?; // don't auto-terminate the core at EOF - we read `eof-reached` ourselves
                init.set_property("idle", "yes")?;
                // Every URL handed to mpv is already a resolved, direct
                // media URL - never a page URL `ytdl_hook` needs to resolve.
                // Left on (mpv's default), every load spawned 3 doomed
                // youtube-dl/yt-dlp subprocess attempts as wasted latency.
                init.set_property("ytdl", false)?;
                Ok(())
            })
            .map_err(|e| format!("Mpv::with_initializer failed: {e}"))?,
        );

        let state: SharedState = Arc::new(Mutex::new(EmbeddedMpvSession {
            id: id.clone(),
            title,
            stream_url: url.clone(),
            status: SessionStatus::Loading,
            position_seconds: start_position_seconds.unwrap_or(0.0),
            duration_seconds: None,
            volume: 100.0,
            started_at: now_rfc3339(),
            updated_at: now_rfc3339(),
            error: None,
        }));

        let event_stop = Arc::new(AtomicBool::new(false));
        let event_thread = spawn_event_thread(mpv.clone(), state.clone(), event_stop.clone());

        mpv.command("loadfile", &[&url, "replace"]).map_err(|e| format!("loadfile failed: {e}"))?;
        if let Some(pos) = start_position_seconds.filter(|p| *p > 0.0) {
            let _ = mpv.set_property("start", pos.to_string());
        }

        Ok(Arc::new(Self { mpv, state, hwnd, event_stop, event_thread: Mutex::new(Some(event_thread)) }))
    }

    pub fn snapshot(&self) -> EmbeddedMpvSession {
        self.state.lock().unwrap().clone()
    }

    pub fn hwnd(&self) -> SendHwnd {
        self.hwnd
    }

    pub fn play_pause(&self, paused: bool) -> Result<(), String> {
        self.mpv.set_property("pause", paused).map_err(|e| e.to_string())?;
        touch(&self.state);
        Ok(())
    }

    pub fn seek(&self, position_seconds: f64) -> Result<(), String> {
        self.mpv.set_property("time-pos", position_seconds).map_err(|e| e.to_string())?;
        touch(&self.state);
        Ok(())
    }

    pub fn set_volume(&self, volume: f64) -> Result<(), String> {
        self.mpv.set_property("volume", volume.clamp(0.0, 100.0)).map_err(|e| e.to_string())?;
        touch(&self.state);
        Ok(())
    }

    /// mpv's native `brightness` property ranges -100..100, 0 = neutral.
    pub fn set_brightness(&self, brightness: f64) -> Result<(), String> {
        self.mpv.set_property("brightness", brightness.clamp(-100.0, 100.0)).map_err(|e| e.to_string())?;
        touch(&self.state);
        Ok(())
    }

    /// Stops the event thread and destroys the native window. The
    /// window-destroy step must run on the main thread - callers wrap this
    /// with `commands/mpv.rs`'s `on_main_thread` helper.
    pub fn stop(&self) {
        self.event_stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.event_thread.lock().unwrap().take() {
            let _ = join.join();
        }
        window::destroy(self.hwnd);
        // `mpv.command("quit", ...)` isn't needed - once the caller drops
        // this `Arc<MpvSession>`, `self.mpv`'s last reference goes with it
        // and `Mpv::drop` runs, which calls `mpv_destroy` itself.
    }
}

fn spawn_event_thread(event_client: Arc<Mpv>, state: SharedState, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("mpv-events".into())
        .spawn(move || {
            // Node-formatted properties (track-list, chapter-list, ...)
            // aren't handled by this crate's `PropertyData::from_raw` (it
            // panics on anything else) - scoped to core playback state only.
            let _ = event_client.disable_deprecated_events();
            // `libmpv2` doesn't wrap `mpv_request_log_messages` - called
            // directly via the raw `-sys` binding. mpv's internal log
            // explains WHY a demuxer rejected a file, better than an error
            // code alone. `"warn"` (not `"v"`) - verbose floods the console
            // with every internal init/decode step on every playback start,
            // none of it actionable.
            unsafe {
                let level = std::ffi::CString::new("warn").unwrap();
                libmpv2_sys::mpv_request_log_messages(event_client.ctx.as_ptr(), level.as_ptr());
            }
            let _ = event_client.observe_property("pause", Format::Flag, 1);
            let _ = event_client.observe_property("time-pos", Format::Double, 2);
            let _ = event_client.observe_property("duration", Format::Double, 3);
            let _ = event_client.observe_property("volume", Format::Double, 4);
            let _ = event_client.observe_property("eof-reached", Format::Flag, 5);

            loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                match event_client.wait_event(0.25) {
                    Some(Ok(libmpv2::events::Event::Shutdown)) => break,
                    Some(Ok(libmpv2::events::Event::LogMessage { prefix, level, text, .. })) => {
                        tracing::info!("[mpv:{level}] {prefix}: {}", text.trim_end());
                    }
                    Some(Ok(libmpv2::events::Event::EndFile(_))) => {
                        let mut s = state.lock().unwrap();
                        s.status = SessionStatus::Ended;
                        s.updated_at = now_rfc3339();
                    }
                    Some(Ok(libmpv2::events::Event::PropertyChange { name, change, .. })) => {
                        let mut s = state.lock().unwrap();
                        match (name, change) {
                            ("pause", PropertyData::Flag(paused)) => {
                                if s.status != SessionStatus::Ended && s.status != SessionStatus::Error {
                                    s.status = if paused { SessionStatus::Paused } else { SessionStatus::Playing };
                                }
                            }
                            ("time-pos", PropertyData::Double(pos)) => s.position_seconds = pos,
                            ("duration", PropertyData::Double(dur)) => s.duration_seconds = Some(dur),
                            ("volume", PropertyData::Double(vol)) => s.volume = vol,
                            ("eof-reached", PropertyData::Flag(true)) => s.status = SessionStatus::Ended,
                            _ => {}
                        }
                        s.updated_at = now_rfc3339();
                    }
                    Some(Err(e)) => {
                        let mut s = state.lock().unwrap();
                        s.status = SessionStatus::Error;
                        s.error = Some(e.to_string());
                        s.updated_at = now_rfc3339();
                    }
                    _ => {}
                }
            }
        })
        .expect("failed to spawn mpv event thread")
}
