//! Daily auto-refresh scheduler for the `Playlist.auto_refresh` flag.
//!
//! Wall-clock anchored to each playlist's `import_date` LOCAL time-of-day: a
//! playlist added at 5:00pm re-refreshes every day at ~5:00pm thereafter, on
//! this machine's local clock, not UTC. Ticks once a minute (cheap: one
//! `SELECT` plus time math); no cron crate needed, `chrono` covers it.

use crate::db::{self, DbPool};
use crate::state::AppState;
use crate::types::{Playlist, PlaylistType};
use chrono::{DateTime, Local, Timelike};
use reqwest::Client;
use std::time::Duration;

/// Spawns the scheduler as a detached background task. Called once from
/// `lib.rs`'s `setup()`, right after the stream proxy starts.
pub fn spawn(db: DbPool, http: Client, app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            run_due_playlists(&db, &http, &app).await;
        }
    });
}

async fn run_due_playlists(db: &DbPool, http: &Client, app: &tauri::AppHandle) {
    let playlists = match db::with_conn(db, |conn| Ok(db::playlists::list(conn)?)).await {
        Ok(playlists) => playlists,
        Err(e) => {
            tracing::warn!("scheduler: failed to list playlists: {e}");
            return;
        }
    };

    let now_local = Local::now();
    for playlist in playlists.into_iter().filter(|p| p.auto_refresh) {
        if !is_due(&playlist, now_local) {
            continue;
        }
        tracing::info!("scheduler: auto-refreshing playlist '{}' ({})", playlist.title, playlist.id);
        let state = AppState::detached(db.clone(), http.clone(), Some(app.clone()));
        match crate::commands::playlist::refresh_playlist_inner(&state, &playlist.id).await {
            Ok(refreshed) => {
                // Xtream keeps eager VOD sync (fast, complete). Stalker
                // VOD/series is remote-first/lazy (see `vod_get_items_live`)
                // - a daily crawl here would reintroduce the wait that
                // redesign removed; manual `vod_sync` is the only way to
                // force a full Stalker resync.
                if refreshed.playlist_type != PlaylistType::Stalker {
                    if let Err(e) = crate::commands::vod::vod_sync_playlist(&state, &refreshed).await {
                        tracing::warn!("scheduler: VOD sync failed for playlist '{}': {e}", playlist.title);
                    }
                }
            }
            Err(e) => {
                // One broken playlist (dead server, expired account) must
                // never stop other due playlists' refresh this tick -
                // matches this app's best-effort background-task philosophy.
                tracing::warn!("scheduler: auto-refresh failed for playlist '{}': {e}", playlist.title);
            }
        }
    }
}

/// Due when local time matches `import_date`'s local time-of-day to the
/// minute, and it hasn't already been refreshed today (local calendar date).
/// Both checks convert to `Local` first - mixing UTC and local math would
/// let them disagree near midnight.
fn is_due(playlist: &Playlist, now_local: DateTime<Local>) -> bool {
    let Some(anchor_local) = parse_local(&playlist.import_date) else {
        return false;
    };
    if now_local.hour() != anchor_local.hour() || now_local.minute() != anchor_local.minute() {
        return false;
    }
    if let Some(update_date) = &playlist.update_date {
        if let Some(last_refreshed_local) = parse_local(update_date) {
            if last_refreshed_local.date_naive() == now_local.date_naive() {
                return false;
            }
        }
    }
    true
}

fn parse_local(rfc3339: &str) -> Option<DateTime<Local>> {
    DateTime::parse_from_rfc3339(rfc3339).ok().map(|dt| dt.with_timezone(&Local))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn playlist_at(import_hour: u32, import_minute: u32, update_date: Option<&str>) -> Playlist {
        // A fixed UTC anchor date/time whose LOCAL hour/minute is what
        // `is_due` actually reads - constructed via `Local` directly so the
        // test doesn't need to know this machine's UTC offset.
        let anchor_local = Local.with_ymd_and_hms(2026, 1, 1, import_hour, import_minute, 0).unwrap();
        let mut playlist = crate::types::Playlist {
            id: "p1".into(),
            title: "Test".into(),
            filename: None,
            playlist_type: crate::types::PlaylistType::Xtream,
            import_date: anchor_local.to_rfc3339(),
            last_usage: anchor_local.to_rfc3339(),
            count: 0,
            url: None,
            user_agent: None,
            referrer: None,
            origin: None,
            file_path: None,
            epg_urls: None,
            detected_epg_urls: None,
            manual_epg_urls: None,
            disabled_epg_urls: None,
            auto_refresh: true,
            update_date: update_date.map(str::to_string),
            update_state: None,
            position: None,
            is_temporary: None,
            server_url: None,
            username: None,
            password: None,
            mac_address: None,
            portal_url: None,
            is_full_stalker_portal: None,
            server_timezone: None,
            stalker_token: None,
            stalker_session_identity: None,
            stalker_watchdog_timeout: None,
            stalker_timeslot: None,
            stalker_serial_number: None,
            stalker_device_id1: None,
            stalker_device_id2: None,
            stalker_signature1: None,
            stalker_signature2: None,
            stalker_account_info: None,
            hidden_group_titles: None,
            stalker_login_completed: None,
            stalker_not_valid: None,
            stalker_endpoint: None,
        };
        // `import_date`/`update_date` are the only fields `is_due` reads —
        // silence the "field never read" concern by touching `playlist.id`.
        let _ = &playlist.id;
        playlist.import_date = anchor_local.to_rfc3339();
        playlist
    }

    #[test]
    fn due_when_local_time_matches_anchor_and_never_refreshed() {
        let playlist = playlist_at(17, 0, None);
        let now = Local.with_ymd_and_hms(2026, 6, 15, 17, 0, 0).unwrap();
        assert!(is_due(&playlist, now));
    }

    #[test]
    fn not_due_outside_the_anchor_minute() {
        let playlist = playlist_at(17, 0, None);
        let now = Local.with_ymd_and_hms(2026, 6, 15, 17, 1, 0).unwrap();
        assert!(!is_due(&playlist, now));
    }

    #[test]
    fn not_due_if_already_refreshed_today() {
        let playlist = playlist_at(17, 0, Some(&Local.with_ymd_and_hms(2026, 6, 15, 17, 0, 0).unwrap().to_rfc3339()));
        let now = Local.with_ymd_and_hms(2026, 6, 15, 17, 0, 0).unwrap();
        assert!(!is_due(&playlist, now));
    }

    #[test]
    fn due_again_the_next_day_at_the_same_anchor_time() {
        let playlist = playlist_at(17, 0, Some(&Local.with_ymd_and_hms(2026, 6, 15, 17, 0, 0).unwrap().to_rfc3339()));
        let now = Local.with_ymd_and_hms(2026, 6, 16, 17, 0, 0).unwrap();
        assert!(is_due(&playlist, now));
    }

    #[test]
    fn unparseable_import_date_is_never_due() {
        let mut playlist = playlist_at(17, 0, None);
        playlist.import_date = "not-a-date".into();
        let now = Local.with_ymd_and_hms(2026, 6, 15, 17, 0, 0).unwrap();
        assert!(!is_due(&playlist, now));
    }

    #[test]
    fn parse_local_rejects_garbage_but_accepts_rfc3339() {
        assert!(parse_local("garbage").is_none());
        assert!(parse_local(&Utc::now().to_rfc3339()).is_some());
    }
}
