use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum XtreamStreamType {
    Live,
    Movie,
    Series,
    Radio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XtreamCategory {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub category_id: String,
    pub category_name: String,
    pub parent_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XtreamStream {
    pub num: i64,
    pub name: String,
    pub stream_type: XtreamStreamType,
    pub stream_id: i64,
    pub stream_icon: String,
    pub added: String,
    pub category_id: String,
    pub custom_sid: String,
    pub direct_source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epg_channel_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tv_archive: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tv_archive_duration: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating_imdb: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xtream_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_extension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_series: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XtreamServerInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub https_port: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtmp_port: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_protocol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_now: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_now: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XtreamUserInfo {
    pub username: String,
    pub password: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub auth: i64,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exp_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_connections: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_output_formats: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_trial: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_cons: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_info: Option<XtreamServerInfo>,
}
