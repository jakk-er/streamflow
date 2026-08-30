//! In-process embedded mpv playback for content the native `<video>` element
//! can't handle (mainly `.mkv`). Embeds via mpv's `wid` property: a native
//! child window/view owned per-platform - `window.rs` (Win32 HWND),
//! `window_linux.rs` (gtk::Socket/XEmbed), `window_macos.rs` (NSView).

mod probe;
mod session;
#[cfg(windows)]
pub(crate) mod window;
#[cfg(target_os = "linux")]
pub(crate) mod window_linux;
#[cfg(target_os = "macos")]
pub(crate) mod window_macos;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod pointer_bridge;

// One name (`window_impl`) `session.rs`/`commands/mpv.rs` use regardless of
// platform - exactly one of these `use`s is ever active per build.
#[cfg(windows)]
pub(crate) use window as window_impl;
#[cfg(target_os = "linux")]
pub(crate) use window_linux as window_impl;
#[cfg(target_os = "macos")]
pub(crate) use window_macos as window_impl;

pub use probe::{check_available, MpvCapability};
#[allow(unused_imports)]
pub use session::{EmbeddedMpvSession, MpvSession, SessionStatus};
#[allow(unused_imports)]
pub use window_impl::WindowBounds;
