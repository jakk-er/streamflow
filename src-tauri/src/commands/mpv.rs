//! Tauri command surface for in-process embedded mpv playback (MKV/HEVC VOD
//! `native <video>` can't handle - see `mpv_player`'s module doc). Commands
//! that touch the native child window marshal onto the main/UI thread via
//! `on_main_thread`, since window messages only deliver to the thread that
//! created it. `Mpv` handle calls (play/pause/seek/volume) skip that - the
//! client API is documented safe from any thread.

use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::error::{CommandError, CommandResult};
use crate::mpv_player::{self, window, EmbeddedMpvSession, MpvCapability, MpvSession};
use crate::state::AppState;

/// Runs `f` on the main/UI thread and awaits its result. Needed for every
/// Win32 window-mutating call (`CreateWindowExW`/`SetWindowPos`/
/// `DestroyWindow`) - see the module doc comment.
async fn on_main_thread<F, T>(app: &AppHandle, f: F) -> CommandResult<T>
where
    F: FnOnce() -> CommandResult<T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| CommandError::Internal("main window not found".into()))?;
    window
        .run_on_main_thread(move || {
            let _ = tx.send(f());
        })
        .map_err(|e| CommandError::Internal(format!("run_on_main_thread failed: {e}")))?;
    rx.await.map_err(|_| CommandError::Internal("main-thread mpv task was dropped before completing".into()))?
}

fn get_session(state: &AppState, session_id: &str) -> CommandResult<Arc<MpvSession>> {
    let sessions = state.mpv_sessions.lock().map_err(|_| CommandError::Internal("mpv session registry lock poisoned".into()))?;
    sessions.get(session_id).cloned().ok_or_else(|| CommandError::NotFound(format!("mpv session {session_id}")))
}

#[tauri::command]
pub async fn mpv_check_available() -> CommandResult<MpvCapability> {
    Ok(mpv_player::check_available())
}

/// `x`/`y`/`width`/`height` are logical (CSS) pixels from the placeholder
/// `<div>`'s `getBoundingClientRect()`, matching the origin `SetWindowPos`
/// expects. Converted to physical pixels via the window's scale factor
/// before reaching Win32.
#[tauri::command]
pub async fn mpv_start_session(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    title: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    start_position_seconds: Option<f64>,
) -> CommandResult<String> {
    if !mpv_player::check_available().is_available() {
        return Err(CommandError::Api("Embedded playback isn't available on this system (libmpv-2.dll not found).".into()));
    }

    // Held for the whole function, not just the drain below - see
    // `AppState::mpv_start_lock`'s doc comment. Without it, near-simultaneous
    // calls can each pass the drain-check before any of them `insert`s below,
    // creating multiple independent sessions in parallel (observed for real).
    let _start_guard = state.mpv_start_lock.lock().await;

    // Hard invariant: only one embedded mpv session may be alive at a time.
    // A frontend bug that starts a new session before the previous one's
    // teardown completes (e.g. a fast retry loop) would otherwise pile up
    // concurrent sessions, each hammering the same upstream and multiplying
    // into a connection storm the provider sees as a DDoS (observed for
    // real). Draining unconditionally here makes that impossible.
    let stale: Vec<Arc<MpvSession>> = {
        let mut sessions = state.mpv_sessions.lock().map_err(|_| CommandError::Internal("mpv session registry lock poisoned".into()))?;
        sessions.drain().map(|(_, s)| s).collect()
    };
    if !stale.is_empty() {
        tracing::warn!("mpv_start_session: {} pre-existing session(s) still registered - stopping before starting a new one", stale.len());
        let app_for_cleanup = app.clone();
        let _ = on_main_thread(&app_for_cleanup, move || {
            for s in stale {
                s.stop();
            }
            Ok(())
        })
        .await;
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let sid = session_id.clone();
    let app_for_thread = app.clone();
    let hwnd = on_main_thread(&app, move || {
        let main_window = app_for_thread.get_webview_window("main").ok_or_else(|| CommandError::Internal("main window not found".into()))?;
        let parent = main_window.hwnd().map_err(|e| CommandError::Internal(format!("failed to get main window HWND: {e}")))?;
        let scale = main_window.scale_factor().unwrap_or(1.0);
        let bounds = window::WindowBounds {
            x: (x * scale).round() as i32,
            y: (y * scale).round() as i32,
            width: (width * scale).round() as i32,
            height: (height * scale).round() as i32,
        };
        window::create(app_for_thread.clone(), sid.clone(), parent, bounds).map_err(CommandError::Api)
    })
    .await?;

    let session = match MpvSession::start(session_id.clone(), url, title, hwnd, start_position_seconds) {
        Ok(session) => session,
        Err(e) => {
            // Window was already created above; if mpv fails to start it
            // would otherwise leak with nothing left to destroy it.
            let _ = on_main_thread(&app, move || {
                window::destroy(hwnd);
                Ok(())
            })
            .await;
            return Err(CommandError::Api(format!("Failed to start embedded mpv session: {e}")));
        }
    };

    let mut sessions = state.mpv_sessions.lock().map_err(|_| CommandError::Internal("mpv session registry lock poisoned".into()))?;
    sessions.insert(session_id.clone(), session);
    Ok(session_id)
}

#[derive(serde::Deserialize)]
pub struct RectDto {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Called on every placeholder-div resize/move and controls-visibility
/// toggle. `x`/`y`/`width`/`height` are always the FULL video area - the
/// window never shrinks for overlay UI (see `window::set_bounds`).
/// `exclude_rects` are the parts overlay HTML currently owns (control bar,
/// close button) - see `window::set_region`.
#[tauri::command]
pub async fn mpv_set_bounds(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    exclude_rects: Vec<RectDto>,
) -> CommandResult<()> {
    let session = get_session(&state, &session_id)?;
    let hwnd = session.hwnd();
    let app_for_thread = app.clone();
    on_main_thread(&app, move || {
        let main_window = app_for_thread.get_webview_window("main").ok_or_else(|| CommandError::Internal("main window not found".into()))?;
        let scale = main_window.scale_factor().unwrap_or(1.0);
        let to_physical = |r: &RectDto| window::WindowBounds {
            x: (r.x * scale).round() as i32,
            y: (r.y * scale).round() as i32,
            width: (r.width * scale).round() as i32,
            height: (r.height * scale).round() as i32,
        };
        let bounds = to_physical(&RectDto { x, y, width, height });
        let exclude: Vec<window::WindowBounds> = exclude_rects.iter().map(to_physical).collect();
        window::set_bounds(hwnd, bounds);
        window::set_region(hwnd, bounds, &exclude);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn mpv_stop_session(app: AppHandle, state: State<'_, AppState>, session_id: String) -> CommandResult<()> {
    let session = {
        let mut sessions = state.mpv_sessions.lock().map_err(|_| CommandError::Internal("mpv session registry lock poisoned".into()))?;
        sessions.remove(&session_id)
    };
    let Some(session) = session else { return Ok(()) };
    on_main_thread(&app, move || {
        session.stop();
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn mpv_get_session_state(state: State<'_, AppState>, session_id: String) -> CommandResult<EmbeddedMpvSession> {
    let session = get_session(&state, &session_id)?;
    Ok(session.snapshot())
}

#[tauri::command]
pub async fn mpv_play_pause(state: State<'_, AppState>, session_id: String, paused: bool) -> CommandResult<()> {
    let session = get_session(&state, &session_id)?;
    session.play_pause(paused).map_err(CommandError::Api)
}

#[tauri::command]
pub async fn mpv_seek(state: State<'_, AppState>, session_id: String, position_seconds: f64) -> CommandResult<()> {
    let session = get_session(&state, &session_id)?;
    session.seek(position_seconds).map_err(CommandError::Api)
}

#[tauri::command]
pub async fn mpv_set_volume(state: State<'_, AppState>, session_id: String, volume: f64) -> CommandResult<()> {
    let session = get_session(&state, &session_id)?;
    session.set_volume(volume).map_err(CommandError::Api)
}

#[tauri::command]
pub async fn mpv_set_brightness(state: State<'_, AppState>, session_id: String, brightness: f64) -> CommandResult<()> {
    let session = get_session(&state, &session_id)?;
    session.set_brightness(brightness).map_err(CommandError::Api)
}
