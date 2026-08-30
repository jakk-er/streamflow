use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaylistType {
    M3u,
    Xtream,
    Stalker,
}

impl PlaylistType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlaylistType::M3u => "m3u",
            PlaylistType::Xtream => "xtream",
            PlaylistType::Stalker => "stalker",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "m3u" => Some(PlaylistType::M3u),
            "xtream" => Some(PlaylistType::Xtream),
            "stalker" => Some(PlaylistType::Stalker),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StalkerAccountInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expire_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tariff_plan_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    #[serde(rename = "_id")]
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    pub playlist_type: PlaylistType,
    pub import_date: String,
    pub last_usage: String,
    pub count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referrer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epg_urls: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_epg_urls: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_epg_urls: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_epg_urls: Option<Vec<String>>,
    pub auto_refresh: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_temporary: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portal_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_full_stalker_portal: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalker_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalker_session_identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalker_watchdog_timeout: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalker_timeslot: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalker_serial_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalker_device_id1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalker_device_id2: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalker_signature1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalker_signature2: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalker_account_info: Option<StalkerAccountInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden_group_titles: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalker_login_completed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalker_not_valid: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalker_endpoint: Option<String>,
}

impl Playlist {
    /// Single source of truth for "does this Stalker playlist need the full
    /// handshake/token flow, or is it token-free?" — matches iptvnator's
    /// `isFullStalkerPortalPlaylist()`; every call site must use THIS
    /// function, not re-derive it locally (iptvnator had this diverge across
    /// copies and it caused real bugs). `is_full_stalker_portal` is set once
    /// by discovery and trusted after; the URL-shape fallback only matters
    /// for legacy rows that predate that field.
    pub fn is_full_stalker_portal(&self) -> bool {
        if let Some(explicit) = self.is_full_stalker_portal {
            return explicit;
        }
        let url = self.stalker_endpoint.as_deref().or(self.portal_url.as_deref()).unwrap_or("");
        url.contains("/stalker_portal") || url.contains("/server/load.php")
    }
}
