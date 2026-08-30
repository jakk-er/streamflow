//! In-process embedded mpv playback for content the native `<video>` element
//! can't handle - typically Xtream/Stalker VOD in `.mkv` (HEVC + AC3/EAC3/
//! DTS). Chromium has never supported the Matroska container - a demuxer
//! gap, not a codec gap - so no engine cascade (mpegts.js/hls.js) helps;
//! only a real second decoder does.
//!
//! Embeds via mpv's `wid` property: a plain Win32 child window this module
//! owns, handed to libmpv as its render target. mpv draws into it directly
//! with its own GPU output (`vo=gpu`) - no off-screen framebuffer, no
//! frame-transport protocol. Audio plays through mpv's own WASAPI output,
//! independent of the video window.

mod probe;
mod session;
pub(crate) mod window;

pub use probe::{check_available, MpvCapability};
#[allow(unused_imports)]
pub use session::{EmbeddedMpvSession, MpvSession, SessionStatus};
#[allow(unused_imports)]
pub use window::WindowBounds;
