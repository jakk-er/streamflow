use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StalkerContentType {
    Itv,
    Radio,
    Vod,
    Series,
}

impl StalkerContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            StalkerContentType::Itv => "itv",
            StalkerContentType::Radio => "radio",
            StalkerContentType::Vod => "vod",
            StalkerContentType::Series => "series",
        }
    }
}

/// The durable bits of a Stalker session, persisted onto the playlist row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StalkerSessionInfo {
    pub token: String,
    pub endpoint: String,
    pub full_portal: bool,
    pub watchdog_timeout: i64,
    pub timeslot: i64,
    pub not_valid: bool,
    pub login_completed: bool,
    pub session_fingerprint: String,
}

/// Login-required and refusal states are not thrown as command errors — the
/// UI branches on `kind` (e.g. shows `StalkerLoginForm` for `loginRequired`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StalkerAuthOutcome {
    Success { session: StalkerSessionInfo },
    LoginRequired,
    LoginRejected { message: String },
    DeviceConflict { message: String },
    Blocked { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StalkerCategory {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

/// A raw catalog row from `get_ordered_list` (ITV/radio/VOD/series). Used
/// both as `StalkerContentPage::data` elements and as the `item` argument to
/// detail/playback commands - Stalker has no single-item lookup endpoint, so
/// the frontend round-trips whatever row it has. Keep this the ONE
/// definition; a drifted duplicate fails at IPC deserialization, not compile time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StalkerContentItem {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actors: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub director: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating_imdb: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_id: Option<String>,
    pub is_series: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_files: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_http_tmp_link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_load_balancing: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genres_str: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StalkerContentPage<T = StalkerContentItem> {
    pub data: Vec<T>,
    pub total_items: i64,
    pub max_page_items: i64,
    pub cur_page: i64,
    pub total_pages: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StalkerChannel {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tv_genre_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xmltv_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_http_tmp_link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_load_balancing: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<i64>,
}
