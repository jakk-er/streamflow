pub mod channel;
pub mod download;
pub mod epg;
pub mod favorites;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
pub mod mpv;
pub mod player;
pub mod playlist;
pub mod settings;
pub mod stalker;
pub mod vod;
pub mod vod_progress;
pub mod xtream;
