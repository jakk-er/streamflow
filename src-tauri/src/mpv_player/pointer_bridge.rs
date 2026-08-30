//! Shared Linux/macOS mouse-forwarding bridge, folded into `session.rs`'s
//! event thread (no thread of its own). Windows forwards pointer events by
//! owning the `wnd_proc` of mpv's own window; that doesn't generalize once
//! mpv is embedded via XEmbed/NSView, since mpv's own client then owns
//! input for that surface. Instead this observes libmpv's `mouse-pos/x`,
//! `mouse-pos/y`, `mouse-pos/hover` LEAF properties only - the parent
//! `mouse-pos` node hits `libmpv2`'s `unimplemented!()` on `Format::Node`,
//! which would abort the process (`panic = "abort"`) on the first mouse
//! move. Double-click reroutes mpv's real `MBTN_LEFT_DBL` binding to a
//! script-message, read back as `Event::ClientMessage`.
//!
//! Emits the same `mpv-pointer` event Windows does, so
//! `EmbeddedMpvPlayer.svelte` needs no platform-specific code.

use std::time::{Duration, Instant};

use libmpv2::events::PropertyData;
use libmpv2::Mpv;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Matches `window.rs`'s own `MOUSE_MOVE_THROTTLE`.
const MOUSE_MOVE_THROTTLE: Duration = Duration::from_millis(150);
const DBLCLICK_SCRIPT_MESSAGE: &str = "streamflow-dblclick";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PointerEvent<'a> {
    session_id: &'a str,
    kind: &'a str,
}

pub struct PointerBridge {
    app: AppHandle,
    session_id: String,
    hovering: bool,
    last_move_emit: Instant,
}

impl PointerBridge {
    pub fn new(app: AppHandle, session_id: String) -> Self {
        Self { app, session_id, hovering: false, last_move_emit: Instant::now() - MOUSE_MOVE_THROTTLE }
    }

    /// Called once, alongside `session.rs`'s core property observations.
    pub fn observe(&self, mpv: &Mpv) {
        let _ = mpv.observe_property("mouse-pos/x", libmpv2::Format::Int64, 100);
        let _ = mpv.observe_property("mouse-pos/y", libmpv2::Format::Int64, 101);
        let _ = mpv.observe_property("mouse-pos/hover", libmpv2::Format::Flag, 102);
        let _ = mpv.command("keybind", &["MBTN_LEFT_DBL", &format!("script-message {DBLCLICK_SCRIPT_MESSAGE}")]);
    }

    fn emit(&self, kind: &str) {
        let _ = self.app.emit("mpv-pointer", PointerEvent { session_id: &self.session_id, kind });
    }

    /// Called from the event loop's `PropertyChange` arm.
    pub fn handle_property_change(&mut self, name: &str, change: &PropertyData) {
        match (name, change) {
            ("mouse-pos/hover", PropertyData::Flag(hover)) => {
                if *hover && !self.hovering {
                    self.hovering = true;
                } else if !*hover && self.hovering {
                    self.hovering = false;
                    self.emit("leave");
                }
            }
            ("mouse-pos/x", PropertyData::Int64(_)) | ("mouse-pos/y", PropertyData::Int64(_)) => {
                if !self.hovering {
                    return;
                }
                let now = Instant::now();
                if now.duration_since(self.last_move_emit) >= MOUSE_MOVE_THROTTLE {
                    self.last_move_emit = now;
                    self.emit("move");
                }
            }
            _ => {}
        }
    }

    /// Called from the event loop's `Event::ClientMessage(args)` arm.
    pub fn handle_client_message(&self, args: &[&str]) {
        if args.first() == Some(&DBLCLICK_SCRIPT_MESSAGE) {
            self.emit("dblclick");
        }
    }
}
