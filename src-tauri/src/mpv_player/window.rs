//! The native child window mpv renders into, and nothing else - no GL
//! context, no message loop of our own. Created on Tauri's main/UI thread
//! (`commands/mpv.rs`'s `on_main_thread`) since a window's messages can only
//! be delivered to the thread that created it - anywhere else leaves it
//! silently undispatched.
//!
//! Positioned to exactly cover `EmbeddedMpvPlayer.svelte`'s `<div>`
//! placeholder and raised above the WebView2 window on every move/resize -
//! that's what makes it look "embedded" despite being a separate native
//! window. Wherever it covers, DOM mouse events never reach the webview (OS
//! hit-tests native siblings first), so `WM_MOUSEMOVE`/`WM_LBUTTONDBLCLK`/
//! `WM_MOUSELEAVE` are forwarded back as Tauri events (`mpv-pointer`) to keep
//! hover-controls and double-click-fullscreen working. Playback control
//! itself doesn't need this - `PlayerControls.svelte` drives that directly
//! via `mpv_*` commands.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{CombineRgn, CreateRectRgn, DeleteObject, GetStockObject, SetWindowRgn, BLACK_BRUSH, RGN_DIFF};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, LoadCursorW, RegisterClassExW, SetWindowLongPtrW, SetWindowPos,
    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, HWND_TOP, IDC_ARROW, SWP_NOACTIVATE, SWP_SHOWWINDOW, WM_DESTROY, WM_ERASEBKGND,
    WM_LBUTTONDBLCLK, WM_MOUSEMOVE, WM_NCCREATE, WM_NCDESTROY, WNDCLASSEXW, WS_CHILD, WS_EX_NOPARENTNOTIFY, WS_VISIBLE,
};

/// Physical-pixel bounds relative to the parent's client area - what
/// `SetWindowPos`/`CreateWindowExW` want directly. Converted once from the
/// frontend's logical (CSS) pixels in `commands/mpv.rs` via `scale_factor()`.
#[derive(Debug, Clone, Copy)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// `HWND` is a plain opaque handle, not a dereferenced pointer, so moving
/// the value between threads is safe - Win32's real thread-affinity
/// (message delivery) depends on which thread *created* the window, not
/// which holds this value. Every mutating call still goes through
/// `on_main_thread` regardless.
#[derive(Clone, Copy)]
pub struct SendHwnd(pub HWND);
unsafe impl Send for SendHwnd {}
unsafe impl Sync for SendHwnd {}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PointerEvent<'a> {
    session_id: &'a str,
    kind: &'a str,
}

struct UserData {
    app: AppHandle,
    session_id: String,
    last_move_emit: Instant,
    tracking_leave: bool,
}

const CLASS_NAME: PCWSTR = w!("StreamFlowMpvVideo");
/// Hover-to-reveal-controls only needs "is the pointer still moving", not
/// every coordinate - avoids the IPC/JS churn of forwarding every raw
/// `WM_MOUSEMOVE` at ~60fps.
const MOUSE_MOVE_THROTTLE: Duration = Duration::from_millis(150);

static CLASS_REGISTERED: OnceLock<()> = OnceLock::new();

fn register_class() {
    CLASS_REGISTERED.get_or_init(|| unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(GetStockObject(BLACK_BRUSH).0),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        // A zero return likely means it's already registered (no matching
        // unregister on teardown - the class is process-lifetime, cheap,
        // and reused). Any real failure surfaces later via `CreateWindowExW`.
        RegisterClassExW(&wc);
    });
}

/// Creates the child window, positioned at `bounds` and raised above every
/// sibling (WebView2 included) so it's visible. Must run on the main/UI
/// thread - see the module doc comment.
pub fn create(app: AppHandle, session_id: String, parent: HWND, bounds: WindowBounds) -> Result<SendHwnd, String> {
    register_class();
    let user_data = Box::new(UserData {
        app,
        session_id,
        last_move_emit: Instant::now() - MOUSE_MOVE_THROTTLE,
        tracking_leave: false,
    });
    let user_data_ptr = Box::into_raw(user_data);

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOPARENTNOTIFY,
            CLASS_NAME,
            w!(""),
            WS_CHILD | WS_VISIBLE,
            bounds.x,
            bounds.y,
            bounds.width.max(1),
            bounds.height.max(1),
            Some(parent),
            None,
            None,
            Some(user_data_ptr as *const _ as *const std::ffi::c_void),
        )
    };

    let hwnd = match hwnd {
        Ok(hwnd) => hwnd,
        Err(e) => {
            // `CreateWindowExW` never took ownership on failure - reclaim
            // and drop it ourselves, or it leaks.
            unsafe { drop(Box::from_raw(user_data_ptr)) };
            return Err(format!("CreateWindowExW failed: {e}"));
        }
    };

    unsafe {
        let _ = SetWindowPos(hwnd, Some(HWND_TOP), bounds.x, bounds.y, bounds.width.max(1), bounds.height.max(1), SWP_SHOWWINDOW | SWP_NOACTIVATE);
    }

    Ok(SendHwnd(hwnd))
}

/// Repositions/resizes and re-raises to the top of the z-order - called on
/// every placeholder-div resize/move. Deliberately NEVER shrunk to make room
/// for overlay UI - resizing the actual mpv window forces it to rescale the
/// whole picture, causing a visible "shrink then grow back" glitch;
/// `set_region` is the correct tool for overlay UI instead. Re-raised every
/// call (not just at creation) since WebView2 can re-assert its own
/// z-order internally and bury this window again.
pub fn set_bounds(hwnd: SendHwnd, bounds: WindowBounds) {
    unsafe {
        let _ = SetWindowPos(hwnd.0, Some(HWND_TOP), bounds.x, bounds.y, bounds.width.max(1), bounds.height.max(1), SWP_SHOWWINDOW | SWP_NOACTIVATE);
    }
}

/// Clips the window's visible AND hit-testable area to `bounds` minus
/// `exclude` rects (converted to window-local coordinates here). Wherever a
/// rect is excluded, mpv isn't drawn there - the real HTML underneath shows
/// through and gets mouse input, with the video staying the same size
/// throughout. This is what makes overlay UI possible without `set_bounds`'s
/// resize-and-rescale glitch (see its doc comment). An empty `exclude` still
/// builds the full-rect region rather than using `SetWindowRgn(None, ...)`,
/// keeping behavior uniform.
pub fn set_region(hwnd: SendHwnd, bounds: WindowBounds, exclude: &[WindowBounds]) {
    unsafe {
        let region = CreateRectRgn(0, 0, bounds.width.max(1), bounds.height.max(1));
        for ex in exclude {
            let local_x1 = (ex.x - bounds.x).clamp(0, bounds.width);
            let local_y1 = (ex.y - bounds.y).clamp(0, bounds.height);
            let local_x2 = (ex.x + ex.width - bounds.x).clamp(0, bounds.width);
            let local_y2 = (ex.y + ex.height - bounds.y).clamp(0, bounds.height);
            if local_x2 <= local_x1 || local_y2 <= local_y1 {
                continue; // entirely outside the window - nothing to subtract
            }
            let hole = CreateRectRgn(local_x1, local_y1, local_x2, local_y2);
            CombineRgn(Some(region), Some(region), Some(hole), RGN_DIFF);
            let _ = DeleteObject(hole.into());
        }
        // `SetWindowRgn` takes ownership of `region` on success - must NOT
        // be deleted here. On failure it leaks a tiny GDI object; not worth
        // extra branching given how rarely this fails for a valid window.
        let _ = SetWindowRgn(hwnd.0, Some(region), true);
    }
}

pub fn destroy(hwnd: SendHwnd) {
    // `UserData` is reclaimed in `wnd_proc`'s `WM_NCDESTROY` handler, not
    // here - `DestroyWindow` synchronously drives the window's message
    // sequence before returning, so the Box is already gone by then.
    unsafe {
        let _ = DestroyWindow(hwnd.0);
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // `WM_NCCREATE` is the first message a window receives, sent
    // synchronously from `CreateWindowExW` before it returns - our boxed
    // `UserData` pointer arrives via `CREATESTRUCTW::lpCreateParams` and is
    // stashed in `GWLP_USERDATA` for retrieval on every later message.
    if msg == WM_NCCREATE {
        let create_struct = lparam.0 as *const CREATESTRUCTW;
        if !create_struct.is_null() {
            let user_data_ptr = (*create_struct).lpCreateParams as isize;
            let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, user_data_ptr);
        }
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    let user_data_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut UserData;

    match msg {
        // Skip the default background erase - it'd flash the class
        // background brush before mpv's first frame lands, and mpv repaints
        // its own content regardless.
        WM_ERASEBKGND => return LRESULT(1),

        WM_MOUSEMOVE if !user_data_ptr.is_null() => {
            let data = &mut *user_data_ptr;
            if !data.tracking_leave {
                let mut tme = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: hwnd, dwHoverTime: 0 };
                // Windows auto-cancels tracking once it fires - re-armed via
                // `tracking_leave = false` in `WM_MOUSELEAVE`, so this runs
                // once per hover session, not every move.
                if TrackMouseEvent(&mut tme).is_ok() {
                    data.tracking_leave = true;
                }
            }
            let now = Instant::now();
            if now.duration_since(data.last_move_emit) >= MOUSE_MOVE_THROTTLE {
                data.last_move_emit = now;
                let _ = data.app.emit("mpv-pointer", PointerEvent { session_id: &data.session_id, kind: "move" });
            }
        }

        WM_MOUSELEAVE if !user_data_ptr.is_null() => {
            let data = &mut *user_data_ptr;
            data.tracking_leave = false;
            let _ = data.app.emit("mpv-pointer", PointerEvent { session_id: &data.session_id, kind: "leave" });
        }

        WM_LBUTTONDBLCLK if !user_data_ptr.is_null() => {
            let data = &mut *user_data_ptr;
            let _ = data.app.emit("mpv-pointer", PointerEvent { session_id: &data.session_id, kind: "dblclick" });
        }

        // `WM_DESTROY` fires while the window is still valid; `WM_NCDESTROY`
        // is the true last message, once nothing could reference it -
        // the correct point to reclaim `UserData`.
        WM_NCDESTROY if !user_data_ptr.is_null() => {
            let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(user_data_ptr));
        }

        WM_DESTROY => {}

        _ => {}
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}
