//! GTK/X11 equivalent of `window.rs` (same four-function contract). Embeds
//! mpv via a `gtk::Socket` (XEmbed, X11-only - why `lib.rs` forces
//! `GDK_BACKEND=x11`) inside a `gtk::Overlay` over the webview's
//! `default_vbox()` - GTK's compositing order paints it above by
//! construction, no `HWND_TOP`-style re-raise hack needed.
//!
//! Mouse forwarding isn't done here - mpv's own X11 client owns input once
//! embedded, not this socket. See `pointer_bridge.rs` instead.

use std::sync::{Arc, Mutex, OnceLock};

use gtk::prelude::*;
use tauri::AppHandle;

#[derive(Debug, Clone, Copy)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// GTK objects aren't `Send`/`Sync` (main-thread-only) - safe here since
/// every mutating call still goes through `on_main_thread`, mirroring
/// `window.rs`'s `SendHwnd`.
#[derive(Clone)]
pub struct SendSocket(pub gtk::Socket);
unsafe impl Send for SendSocket {}
unsafe impl Sync for SendSocket {}

#[derive(Clone)]
struct SendOverlay(gtk::Overlay);
unsafe impl Send for SendOverlay {}
unsafe impl Sync for SendOverlay {}

/// Read by the `Overlay`'s `get-child-position` closure - one slot is
/// enough since only one embedded session is ever alive at a time.
static CURRENT_BOUNDS: OnceLock<Arc<Mutex<WindowBounds>>> = OnceLock::new();

fn bounds_slot() -> &'static Arc<Mutex<WindowBounds>> {
    CURRENT_BOUNDS.get_or_init(|| Arc::new(Mutex::new(WindowBounds { x: 0, y: 0, width: 1, height: 1 })))
}

/// Built once (mirrors `window.rs`'s `CLASS_REGISTERED`) - reparenting the
/// vbox is a one-time structural change, reused for every session after.
static OVERLAY: OnceLock<SendOverlay> = OnceLock::new();

fn ensure_overlay(gtk_window: &gtk::ApplicationWindow, default_vbox: &gtk::Box) -> gtk::Overlay {
    OVERLAY
        .get_or_init(|| {
            gtk_window.remove(default_vbox);
            let overlay = gtk::Overlay::new();
            overlay.add(default_vbox);
            // GTK asks this "where should the overlaid child go" on every re-allocate - lets plain queue_resize() in set_bounds move the socket.
            overlay.connect_get_child_position(move |_overlay, _child| {
                let b = *bounds_slot().lock().unwrap();
                Some(gtk::Rectangle { x: b.x, y: b.y, width: b.width.max(1), height: b.height.max(1) })
            });
            gtk_window.add(&overlay);
            overlay.show_all();
            SendOverlay(overlay)
        })
        .0
        .clone()
}

/// Creates the embedded socket, positioned at `bounds`. Must run on the
/// main/UI thread - same requirement as Windows' Win32 message delivery.
pub fn create(
    _app: AppHandle,
    _session_id: String,
    gtk_window: gtk::ApplicationWindow,
    default_vbox: gtk::Box,
    bounds: WindowBounds,
) -> Result<SendSocket, String> {
    *bounds_slot().lock().map_err(|_| "mpv bounds lock poisoned".to_string())? = bounds;
    let overlay = ensure_overlay(&gtk_window, &default_vbox);

    let socket = gtk::Socket::new();
    overlay.add_overlay(&socket);
    socket.show();
    // No X11 window until realized - force it so .id() returns a real XID now.
    socket.realize();

    Ok(SendSocket(socket))
}

/// Updates the shared bounds slot and asks GTK to re-allocate - no
/// resize-vs-rescale distinction to worry about here (unlike Windows), the
/// socket's XID never changes, only its position/size within the overlay.
pub fn set_bounds(handle: SendSocket, bounds: WindowBounds) {
    if let Ok(mut b) = bounds_slot().lock() {
        *b = bounds;
    }
    handle.0.queue_resize();
}

/// X11 analogue of `window.rs`'s `SetWindowRgn` - X11 separates visual
/// shape from input shape (two calls) where Win32 unifies them into one.
pub fn set_region(handle: SendSocket, bounds: WindowBounds, exclude: &[WindowBounds]) {
    let Some(gdk_window) = handle.0.window() else { return };

    let full = cairo::Region::create_rectangle(&cairo::RectangleInt::new(0, 0, bounds.width.max(1), bounds.height.max(1)));
    for ex in exclude {
        let local_x1 = (ex.x - bounds.x).clamp(0, bounds.width);
        let local_y1 = (ex.y - bounds.y).clamp(0, bounds.height);
        let local_x2 = (ex.x + ex.width - bounds.x).clamp(0, bounds.width);
        let local_y2 = (ex.y + ex.height - bounds.y).clamp(0, bounds.height);
        if local_x2 <= local_x1 || local_y2 <= local_y1 {
            continue; // entirely outside the socket - nothing to subtract
        }
        let hole = cairo::RectangleInt::new(local_x1, local_y1, local_x2 - local_x1, local_y2 - local_y1);
        full.subtract_rectangle(&hole);
    }

    gdk_window.shape_combine_region(Some(&full), 0, 0);
    gdk_window.input_shape_combine_region(Some(&full), 0, 0);
}

pub fn destroy(handle: SendSocket) {
    if let Some(overlay) = OVERLAY.get() {
        overlay.0.remove(&handle.0);
    }
}

/// The X11 XID mpv's `wid` property wants, as a plain integer - the exact
/// analogue of Windows' HWND-as-`i64`.
pub fn wid_value(handle: &SendSocket) -> i64 {
    handle.0.id() as i64
}
