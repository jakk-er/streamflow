//! AppKit equivalent of `window.rs` (same four-function contract). Embeds
//! mpv as an `NSView` added above the webview's own view
//! (`addSubview:positioned:relativeTo:`) instead of a second window, so it
//! paints above by construction - no `HWND_TOP`-style re-raise needed.
//!
//! No single-call region-clip equivalent to `SetWindowRgn` exists here:
//! visual punch-out uses a `CAShapeLayer` mask, input punch-out needs a
//! custom `NSView` subclass overriding `hitTest:`.
//!
//! Mouse forwarding isn't done here either - see `pointer_bridge.rs`.

use std::sync::Mutex;

use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSView, NSWindowOrderingMode};
use objc2_core_graphics::CGPath;
use objc2_foundation::{CGPoint, CGRect, CGSize, MainThreadMarker, NSPoint, NSRect, NSSize};
use objc2_quartz_core::CAShapeLayer;
use tauri::AppHandle;

#[derive(Debug, Clone, Copy)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub struct MpvHostViewIvars {
    /// Local coords (AppKit-flipped) - checked by `hitTest:`, rebuilt on every `set_region` call.
    exclude: Mutex<Vec<WindowBounds>>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "StreamFlowMpvHostView"]
    #[ivars = MpvHostViewIvars]
    pub struct MpvHostView;

    unsafe impl NSObjectProtocol for MpvHostView {}

    impl MpvHostView {
        /// Returns `None` inside an excluded rect so the click falls through to what's beneath.
        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> Option<Retained<NSView>> {
            {
                let exclude = self.ivars().exclude.lock().unwrap();
                for ex in exclude.iter() {
                    if point.x >= ex.x as f64
                        && point.x <= (ex.x + ex.width) as f64
                        && point.y >= ex.y as f64
                        && point.y <= (ex.y + ex.height) as f64
                    {
                        return None;
                    }
                }
            }
            unsafe { msg_send![super(self), hitTest: point] }
        }
    }
);

impl MpvHostView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(MpvHostViewIvars { exclude: Mutex::new(Vec::new()) });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

/// AppKit is main-thread-only, not `Send`/`Sync` by default - safe here
/// since every mutating call goes through `on_main_thread`, mirroring
/// `window.rs`'s `SendHwnd`.
#[derive(Clone)]
pub struct SendView(pub Retained<MpvHostView>);
unsafe impl Send for SendView {}
unsafe impl Sync for SendView {}

/// AppKit's origin is bottom-left, the frontend's is top-left - flips Y.
fn to_nsrect(bounds: WindowBounds, superview_height: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(bounds.x as f64, superview_height - bounds.y as f64 - bounds.height.max(1) as f64),
        NSSize::new(bounds.width.max(1) as f64, bounds.height.max(1) as f64),
    )
}

/// `parent_ns_view` is the webview's own `NSView*` - the mpv host view is
/// added as its SIBLING (into its superview), not its child.
pub fn create(_app: AppHandle, _session_id: String, parent_ns_view: *mut std::ffi::c_void, bounds: WindowBounds) -> Result<SendView, String> {
    let mtm = MainThreadMarker::new().ok_or("mpv window_macos::create called off the main thread")?;
    let webview_view: &NSView = unsafe { &*(parent_ns_view as *mut NSView) };
    let superview = webview_view.superview().ok_or("webview NSView has no superview")?;

    let frame = to_nsrect(bounds, superview.frame().size.height);
    let view = MpvHostView::new(mtm, frame);
    view.setWantsLayer(true);

    unsafe {
        superview.addSubview_positioned_relativeTo(&view, NSWindowOrderingMode::Above, Some(webview_view));
    }

    Ok(SendView(view))
}

/// No resize-vs-rescale concern here unlike Windows - just tracks the div 1:1.
pub fn set_bounds(handle: SendView, bounds: WindowBounds) {
    let Some(superview) = handle.0.superview() else { return };
    let frame = to_nsrect(bounds, superview.frame().size.height);
    unsafe { handle.0.setFrame(frame) };
}

/// AppKit analogue of `SetWindowRgn`, split across a mask (visual) and `hitTest:` (input) - no single unified call here.
pub fn set_region(handle: SendView, bounds: WindowBounds, exclude: &[WindowBounds]) {
    let local: Vec<WindowBounds> = exclude
        .iter()
        .filter_map(|ex| {
            let x1 = (ex.x - bounds.x).clamp(0, bounds.width);
            let y1_top = (ex.y - bounds.y).clamp(0, bounds.height);
            let x2 = (ex.x + ex.width - bounds.x).clamp(0, bounds.width);
            let y2_top = (ex.y + ex.height - bounds.y).clamp(0, bounds.height);
            if x2 <= x1 || y2_top <= y1_top {
                return None; // entirely outside the view - nothing to subtract
            }
            let y1 = bounds.height - y2_top; // flip to this view's bottom-left-origin space
            Some(WindowBounds { x: x1, y: y1, width: x2 - x1, height: y2_top - y1_top })
        })
        .collect();

    *handle.0.ivars().exclude.lock().unwrap() = local.clone();

    // Full rect + each excluded rect as its own subpath, even-odd fill punches the hole.
    let full = CGRect { origin: CGPoint { x: 0.0, y: 0.0 }, size: CGSize { width: bounds.width.max(1) as f64, height: bounds.height.max(1) as f64 } };
    let mut rects = vec![full];
    for ex in &local {
        rects.push(CGRect {
            origin: CGPoint { x: ex.x as f64, y: ex.y as f64 },
            size: CGSize { width: ex.width.max(1) as f64, height: ex.height.max(1) as f64 },
        });
    }
    let path = unsafe { CGPath::with_rects(&rects, None) };
    let mask_layer = CAShapeLayer::new();
    unsafe { mask_layer.setPath(Some(&path)) };
    unsafe { mask_layer.setFillRule(Some(&objc2_foundation::NSString::from_str("even-odd"))) };
    if let Some(layer) = handle.0.layer() {
        unsafe { layer.setMask(Some(&mask_layer)) };
    }
}

pub fn destroy(handle: SendView) {
    unsafe { handle.0.removeFromSuperview() };
}

/// The `NSView*` mpv's `wid` wants, as a plain integer - analogue of Windows' HWND-as-`i64`.
pub fn wid_value(handle: &SendView) -> i64 {
    Retained::as_ptr(&handle.0) as i64
}
