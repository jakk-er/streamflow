use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FavoriteType {
    Channel,
    Global,
}

impl FavoriteType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FavoriteType::Channel => "channel",
            FavoriteType::Global => "global",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "channel" => Some(FavoriteType::Channel),
            "global" => Some(FavoriteType::Global),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteChannel {
    pub id: String,
    pub channel_id: String,
    pub playlist_id: String,
    pub favorite_type: FavoriteType,
    pub created_at: String,
    /// Denormalized from `channels` at read time so the UI can render a real
    /// name/logo without a second lookup — `None` if the channel was since
    /// deleted (e.g. playlist refresh/delete) but the favorite row remains.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_logo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchHistoryItem {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playlist_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_type: Option<String>,
    pub position_seconds: i64,
    pub total_seconds: i64,
    pub watched_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_logo: Option<String>,
}
