use crate::types::StalkerContentItem;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The two catalog kinds the local VOD cache tracks - distinct from
/// `StreamType` (has `Live`/`Radio` too), matching the frontend's
/// `'movie' | 'series'` type and the DB's `content_type` CHECK constraint,
/// so it serializes as the same lowercase string everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VodContentType {
    Movie,
    Series,
}

impl VodContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            VodContentType::Movie => "movie",
            VodContentType::Series => "series",
        }
    }
}

/// One row from the local VOD/series catalog cache (`vod_items`), populated
/// in bulk by `vod_sync`, read back by `vod_get_items`/`vod_get_categories`.
/// Only carries catalog-LIST-level fields (what a bulk provider call returns
/// for free) - full detail (plot/cast, seasons/episodes) is fetched and
/// cached lazily on first view via the existing detail commands, not
/// eagerly during sync (no bulk detail endpoint on either protocol).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VodCatalogItem {
    /// The provider's own id (Xtream `stream_id`/`series_id` as a string,
    /// Stalker's own `id`) — NOT a locally-generated UUID, so `/vod/[id]`
    /// routing and the existing detail commands (which already take a
    /// provider id) need no change.
    pub id: String,
    pub content_type: VodContentType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_extension: Option<String>,
    /// Populated only for Stalker rows (`None` for Xtream) - Stalker has no
    /// id-based lookup endpoint, so detail commands need the full original
    /// row back, same as the live-pagination flow this replaces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalker_item: Option<StalkerContentItem>,
}

/// One live-fetched page from `vod_get_items_live` - a specific-category
/// Stalker browse. `items` reuses the same `VodCatalogItem` shape
/// `vod_get_items` returns, so frontend detail-lookup plumbing needs no
/// changes regardless of whether an item came from cache or a live fetch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VodLivePage {
    pub items: Vec<VodCatalogItem>,
    pub page: i64,
    pub total_pages: i64,
    pub total_items: i64,
}

/// One row from `vod_watch_progress` — a title in "Continue Watching". For a
/// movie, `episode_*` are all `None`. For a series, `vod_item_id` always
/// names the series, and `episode_*` describes the relevant episode: to
/// resume (`position_seconds > 0`) or the next unwatched one (`== 0`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VodWatchProgress {
    pub id: String,
    pub playlist_id: String,
    pub content_type: VodContentType,
    pub vod_item_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season_number: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode_number: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode_title: Option<String>,
    pub position_seconds: i64,
    pub total_seconds: i64,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    pub updated_at: String,
}

/// The episode half of a `vod_watch_progress` upsert — `None` for a movie.
/// Bundled rather than four separate `Option` params, to keep
/// `db::vod_progress::upsert`'s signature readable.
#[derive(Debug, Clone)]
pub struct VodProgressEpisodeRef {
    pub id: String,
    pub season_number: i64,
    pub episode_number: Option<i64>,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamType {
    Live,
    Movie,
    Series,
    Radio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonEpisode {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode_num: Option<i64>,
    pub title: String,
    pub season: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_extension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<EpisodeInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_source: Option<String>,
    /// Stalker-only: raw `cmd`/`series` index for re-resolving a fresh
    /// playback URL right before play (a `create_link` result can already be
    /// a dead temporary link by the time the user clicks Play).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_param: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub name: String,
    pub season_number: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub air_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesDetails {
    pub info: VodDetails,
    pub seasons: Vec<SeasonInfo>,
    pub episodes: HashMap<String, Vec<SeasonEpisode>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VodDetails {
    pub id: String,
    pub name: String,
    pub stream_type: StreamType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_extension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season_number: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode_number: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cast: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmdb_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seasons: Option<Vec<SeasonInfo>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episodes: Option<HashMap<String, Vec<SeasonEpisode>>>,
    /// Stalker-only, see `SeasonEpisode.cmd`. The Stalker `type` ("vod"/
    /// "series") this detail was fetched under — needed to re-resolve `cmd`s,
    /// since that's a different axis than `stream_type` (a `type=vod` row can
    /// itself be flagged as a series).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_http_tmp_link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_load_balancing: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalker_content_type: Option<String>,
}
