mod catchup;
mod commands;
mod db;
mod error;
#[cfg(windows)]
mod mpv_player;
mod net;
mod parsers;
mod scheduler;
mod state;
mod stream_proxy;
mod types;

use state::AppState;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // ERROR only, by request - suppresses info/warn/debug/trace globally at
    // the subscriber level instead of editing every call site individually.
    tracing_subscriber::fmt().with_max_level(tracing::Level::ERROR).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(windows)]
            widen_mpv_dll_search_path(app);

            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join("streamflow.db");
            let pool = db::init_pool(&db_path)?;
            // `.no_proxy()`: reqwest auto-detects and routes through any
            // system/VPN/antivirus HTTP proxy by default, but a real STB
            // never does - and some proxies reset unusual-looking IPTV
            // traffic outright (confirmed: a Stalker request reqwest sent
            // got an immediate TCP reset, while curl succeeded instantly
            // with identical headers).
            let http = reqwest::Client::builder().no_proxy().tcp_nodelay(true).build()?;

            // Separate client with gzip disabled: the proxy must relay bytes
            // exactly as upstream sent them (decompressing a partial gzip
            // stream on a Range request can desync it), while the shared
            // `http` client (Xtream JSON API) benefits from gzip normally.
            // `.tcp_nodelay(true)` avoids Nagle's-algorithm buffering delay
            // on every HLS segment request. `.pool_idle_timeout(300s)` keeps
            // upstream connections warm across segment gaps - reqwest's 90s
            // default was measured evicting a still-in-use connection on
            // some CDNs, forcing a full TCP+TLS re-handshake mid-session;
            // matches a known-working reference proxy's `keepAliveMsecs: 300000`.
            let proxy_http = reqwest::Client::builder()
                .no_gzip()
                .no_proxy()
                .tcp_nodelay(true)
                .pool_idle_timeout(std::time::Duration::from_secs(300))
                .build()?;
            let proxy_port = tauri::async_runtime::block_on(stream_proxy::start(pool.clone(), proxy_http))?;
            tracing::info!("stream proxy listening on 127.0.0.1:{proxy_port}");

            // Real OS "Downloads" folder when available, falling back to a
            // folder under the app data dir if unresolvable.
            let app_handle = app.handle().clone();
            let downloads_dir = app.path().download_dir().unwrap_or_else(|_| app_data_dir.join("downloads")).join("StreamFlow");
            std::fs::create_dir_all(&downloads_dir)?;

            scheduler::spawn(pool.clone(), http.clone(), app.handle().clone());

            app.manage(AppState {
                db: pool,
                http,
                app: Some(app_handle),
                recovery_inflight: std::sync::Arc::new(Mutex::new(std::collections::HashSet::new())),
                stream_proxy_port: Mutex::new(Some(proxy_port)),
                downloads_dir,
                downloads: Mutex::new(HashMap::new()),
                players: Mutex::new(HashMap::new()),
                #[cfg(windows)]
                mpv_sessions: std::sync::Arc::new(Mutex::new(HashMap::new())),
                #[cfg(windows)]
                mpv_start_lock: tokio::sync::Mutex::new(()),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::playlist::import_m3u_playlist,
            commands::playlist::update_m3u_playlist,
            commands::playlist::add_xtream_playlist,
            commands::playlist::update_xtream_playlist,
            commands::playlist::add_stalker_playlist,
            commands::playlist::update_stalker_playlist,
            commands::playlist::get_playlists,
            commands::playlist::delete_playlist,
            commands::playlist::refresh_playlist,
            commands::channel::get_channels_by_playlist,
            commands::channel::search_channels,
            commands::channel::get_channel_by_id,
            commands::channel::m3u_is_catchup_supported,
            commands::channel::m3u_resolve_catchup_url,
            commands::epg::fetch_epg,
            commands::epg::get_epg_for_channel,
            commands::epg::get_current_program,
            commands::favorites::toggle_favorite,
            commands::favorites::get_favorites,
            commands::favorites::reorder_favorites,
            commands::favorites::get_recently_watched,
            commands::favorites::save_playback_position,
            commands::favorites::remove_watch_history_item,
            commands::favorites::clear_watch_history,
            commands::player::get_stream_proxy_port,
            commands::player::spawn_external_player,
            commands::player::kill_player,
            commands::player::get_player_status,
            #[cfg(windows)]
            commands::mpv::mpv_check_available,
            #[cfg(windows)]
            commands::mpv::mpv_start_session,
            #[cfg(windows)]
            commands::mpv::mpv_set_bounds,
            #[cfg(windows)]
            commands::mpv::mpv_stop_session,
            #[cfg(windows)]
            commands::mpv::mpv_get_session_state,
            #[cfg(windows)]
            commands::mpv::mpv_play_pause,
            #[cfg(windows)]
            commands::mpv::mpv_seek,
            #[cfg(windows)]
            commands::mpv::mpv_set_volume,
            #[cfg(windows)]
            commands::mpv::mpv_set_brightness,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::download::start_download,
            commands::download::get_download_progress,
            commands::download::pause_download,
            commands::download::resume_download,
            commands::download::cancel_download,
            commands::stalker::stalker_auth,
            commands::stalker::stalker_do_auth,
            commands::stalker::stalker_refresh_token,
            commands::stalker::stalker_watchdog_ping,
            commands::stalker::stalker_get_categories,
            commands::stalker::stalker_get_content,
            commands::stalker::stalker_get_vod_info,
            commands::stalker::stalker_get_series_info,
            commands::stalker::stalker_resolve_playback,
            commands::stalker::stalker_resolve_vod_episode,
            commands::stalker::stalker_stream_headers,
            commands::stalker::stalker_get_channels,
            commands::stalker::stalker_sync_epg,
            commands::stalker::stalker_get_short_epg,
            commands::stalker::stalker_derive_device_ids,
            commands::xtream::xtream_auth,
            commands::xtream::xtream_get_categories,
            commands::xtream::xtream_get_streams,
            commands::xtream::xtream_get_vod_info,
            commands::xtream::xtream_get_series_info,
            commands::xtream::xtream_resolve_catchup_url,
            commands::xtream::xtream_catchup_available,
            commands::xtream::xtream_channel_catchup_available,
            commands::xtream::xtream_resolve_catchup_url_for_channel,
            commands::xtream::xtream_get_short_epg,
            commands::xtream::xtream_sync_epg,
            commands::vod::vod_get_categories,
            commands::vod::vod_get_items,
            commands::vod::vod_get_top_rated,
            commands::vod::vod_get_items_live,
            commands::vod::vod_get_cached_item,
            commands::vod::vod_sync,
            commands::vod_progress::vod_save_progress,
            commands::vod_progress::vod_clear_progress,
            commands::vod_progress::vod_get_progress,
            commands::vod_progress::vod_get_progress_bulk,
            commands::vod_progress::vod_get_continue_watching,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// `bundle.resources` files land at `$INSTDIR\resources\...` in a packaged
/// build - not on Windows' default DLL search path. Must run before anything
/// can trigger the delay-loaded `libmpv-2.dll` import (see `build.rs`'s
/// `/DELAYLOAD` and `mpv_player::probe`), so it's called first in `setup()`.
/// Tries the packaged resource dir, falling back to the dev-tree path (`cargo
/// build`/`tauri dev` never populate `resource_dir()`) - harmless to call
/// with a dir that lacks the DLL; `mpv_player`'s probe is what actually
/// determines availability, this only widens where the OS loader looks.
#[cfg(windows)]
fn widen_mpv_dll_search_path(app: &tauri::App) {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::LibraryLoader::SetDllDirectoryW;

    let candidate = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("mpv-runtime"))
        .filter(|dir| dir.join("libmpv-2.dll").is_file())
        .or_else(|| {
            let dev_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("mpv-runtime");
            dev_dir.join("libmpv-2.dll").is_file().then_some(dev_dir)
        });

    let Some(dir) = candidate else {
        tracing::warn!("mpv-runtime/libmpv-2.dll not found in the resource dir or the dev tree - embedded MKV playback will be unavailable");
        return;
    };

    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    unsafe {
        if SetDllDirectoryW(PCWSTR(wide.as_ptr())).is_err() {
            tracing::warn!("SetDllDirectoryW failed for {}", dir.display());
        }
    }
}
