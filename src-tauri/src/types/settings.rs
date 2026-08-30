use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[serde(rename = "DARK_THEME")]
    Dark,
    #[serde(rename = "LIGHT_THEME")]
    Light,
    #[serde(rename = "SYSTEM_THEME")]
    System,
}

/// `Videojs`/`Artplayer` were removed (never a real dependency or selectable
/// option, so no settings blob could contain one). `EmbeddedMpv` stays even
/// though it's no longer offered either (the mpv engine now activates
/// automatically by file extension) - it WAS once selectable, so a real
/// settings blob could still reference it, and `read_settings` resets ALL
/// settings on any deserialization failure, so removing the variant would
/// silently wipe more than just this field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VideoPlayer {
    Html5,
    #[serde(rename = "embedded-mpv")]
    EmbeddedMpv,
    Mpv,
    Vlc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoverSize {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SortChannelsBy {
    Default,
    NameAz,
    NameZa,
}

fn default_language() -> String {
    "en".to_string()
}

fn default_true() -> bool {
    true
}

/// Whole-object replace with no version field. Every field carries
/// `#[serde(default)]` so a blob saved before a field was added still
/// deserializes after an app update.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "Theme::default")]
    pub theme: Theme,
    #[serde(default = "default_true")]
    pub show_epg: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epg_source: Option<Vec<String>>,
    #[serde(default)]
    pub enable_analytics: bool,
    #[serde(default = "VideoPlayer::default")]
    pub video_player: VideoPlayer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mpv_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlc_path: Option<String>,
    #[serde(default)]
    pub hide_category_names: bool,
    #[serde(default)]
    pub show_channel_number: bool,
    #[serde(default = "default_true")]
    pub track_watch_history: bool,
    #[serde(default = "SortChannelsBy::default")]
    pub sort_channels_by: SortChannelsBy,
    #[serde(default = "CoverSize::default")]
    pub cover_size: CoverSize,
    /// User-chosen VOD grid column count, clamped by `VodGrid.svelte` to
    /// whatever the window width fits - degrades safely on a smaller window.
    /// `None` falls back to that component's default count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vod_grid_columns: Option<i64>,
    /// Playlist to auto-activate on launch; `None` means no auto-select. Not
    /// validated against the `playlists` table - a stale id is harmless
    /// since the frontend checks existence before acting on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_playlist_id: Option<String>,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::System
    }
}

impl Default for VideoPlayer {
    fn default() -> Self {
        // StreamFlow plays back via a plain <video> element + hls.js/mpegts.js
        // (see VideoPlayer.svelte) — 'html5' is the only option the frontend
        // player store treats as in-app inline playback with no backend call.
        VideoPlayer::Html5
    }
}

impl Default for SortChannelsBy {
    fn default() -> Self {
        SortChannelsBy::Default
    }
}

impl Default for CoverSize {
    fn default() -> Self {
        CoverSize::Medium
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            language: default_language(),
            theme: Theme::default(),
            show_epg: true,
            epg_source: None,
            enable_analytics: false,
            video_player: VideoPlayer::default(),
            mpv_path: None,
            vlc_path: None,
            hide_category_names: false,
            show_channel_number: false,
            track_watch_history: true,
            sort_channels_by: SortChannelsBy::default(),
            cover_size: CoverSize::default(),
            vod_grid_columns: None,
            default_playlist_id: None,
        }
    }
}
