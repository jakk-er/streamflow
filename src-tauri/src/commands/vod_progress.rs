use crate::db;
use crate::error::CommandResult;
use crate::state::AppState;
use crate::types::{VodContentType, VodProgressEpisodeRef, VodWatchProgress};
use std::collections::HashMap;
use tauri::State;

/// Below this, a title isn't worth remembering - a misclick shouldn't show
/// up in Continue Watching. Tracked once position reaches 60s OR 2% of
/// duration, whichever comes first.
fn meets_tracking_threshold(position_seconds: i64, total_seconds: i64) -> bool {
    if position_seconds >= 60 {
        return true;
    }
    total_seconds > 0 && (position_seconds as f64) >= (total_seconds as f64) * 0.02
}

/// Adult/18+/XXX category content is never written to watch-progress -
/// checked by category NAME, the only signal available (no separate "is
/// this adult" flag in the synced data). Deliberately a short, explicit
/// keyword list, not a broader heuristic.
const ADULT_CATEGORY_KEYWORDS: [&str; 4] = ["adult", "xxx", "18+", "porn"];

fn is_adult_category_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    ADULT_CATEGORY_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn vod_save_progress(
    state: State<'_, AppState>,
    playlist_id: String,
    content_type: VodContentType,
    vod_item_id: String,
    episode_id: Option<String>,
    season_number: Option<i64>,
    episode_number: Option<i64>,
    episode_title: Option<String>,
    position_seconds: i64,
    total_seconds: i64,
    title: String,
    cover: Option<String>,
) -> CommandResult<()> {
    if !meets_tracking_threshold(position_seconds, total_seconds) {
        return Ok(());
    }
    db::with_conn(&state.db, move |conn| {
        if !db::read_settings(conn).track_watch_history {
            return Ok(());
        }
        if let Some(category_name) = db::vod::category_name_for_item(conn, &playlist_id, content_type, &vod_item_id)? {
            if is_adult_category_name(&category_name) {
                return Ok(());
            }
        }
        let episode = episode_id.map(|id| VodProgressEpisodeRef {
            id,
            season_number: season_number.unwrap_or(0),
            episode_number,
            title: episode_title.unwrap_or_default(),
        });
        Ok(db::vod_progress::upsert(
            conn,
            &playlist_id,
            content_type,
            &vod_item_id,
            episode.as_ref(),
            position_seconds,
            total_seconds,
            &title,
            cover.as_deref(),
        )?)
    })
    .await
}

/// Called on completion (finished movie, or series with no next episode).
/// A full delete, not a "mark completed" status - a completed title then
/// shows "Play" not "Resume" for free, since there's no row to find.
#[tauri::command]
pub async fn vod_clear_progress(state: State<'_, AppState>, playlist_id: String, content_type: VodContentType, vod_item_id: String) -> CommandResult<()> {
    db::with_conn(&state.db, move |conn| Ok(db::vod_progress::delete(conn, &playlist_id, content_type, &vod_item_id)?)).await
}

#[tauri::command]
pub async fn vod_get_progress(
    state: State<'_, AppState>,
    playlist_id: String,
    content_type: VodContentType,
    vod_item_id: String,
) -> CommandResult<Option<VodWatchProgress>> {
    db::with_conn(&state.db, move |conn| Ok(db::vod_progress::get(conn, &playlist_id, content_type, &vod_item_id)?)).await
}

#[tauri::command]
pub async fn vod_get_progress_bulk(
    state: State<'_, AppState>,
    playlist_id: String,
    content_type: VodContentType,
    vod_item_ids: Vec<String>,
) -> CommandResult<HashMap<String, VodWatchProgress>> {
    db::with_conn(&state.db, move |conn| Ok(db::vod_progress::get_bulk(conn, &playlist_id, content_type, &vod_item_ids)?)).await
}

#[tauri::command]
pub async fn vod_get_continue_watching(state: State<'_, AppState>, limit: i64) -> CommandResult<Vec<VodWatchProgress>> {
    db::with_conn(&state.db, move |conn| Ok(db::vod_progress::list_continue_watching(conn, limit)?)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracking_threshold_matches_confirmed_spec() {
        assert!(meets_tracking_threshold(60, 6000)); // 60s floor
        assert!(!meets_tracking_threshold(59, 6000));
        assert!(meets_tracking_threshold(50, 1000)); // 5% of 1000 = 50, meets the 2% path
        assert!(!meets_tracking_threshold(10, 1000));
        assert!(!meets_tracking_threshold(5, 0)); // no known duration - only the 60s floor can pass
    }

    #[test]
    fn adult_category_names_are_detected_case_insensitively() {
        assert!(is_adult_category_name("FOR ✪ ADULTS X4"));
        assert!(is_adult_category_name("FOR ✪ PORNBOX ADULTS"));
        assert!(is_adult_category_name("xxx | hard hot tv"));
        assert!(is_adult_category_name("18+ Movies"));
        assert!(!is_adult_category_name("TR ✪ SINEMA HITS 2025"));
        assert!(!is_adult_category_name("US - Action"));
    }
}
