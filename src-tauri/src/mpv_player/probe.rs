//! One-time capability detection, cached for the app's lifetime.
//!
//! `libmpv2-sys` links `libmpv-2.dll` as a hard implicit load-time
//! dependency, which would normally fail the WHOLE APP's launch if missing,
//! not just embedded playback. `build.rs` adds `/DELAYLOAD:libmpv-2.dll` to
//! defer that resolution to first call - which makes this module load-
//! bearing: nothing may touch a libmpv2 API before `check_available()` has
//! confirmed (via a plain `LoadLibraryW` probe) the DLL exists. A real
//! delay-load failure raises an SEH exception, not a catchable panic/
//! `Result` - `catch_unwind` can't soften it, and this project's release
//! profile sets `panic = "abort"` anyway.

use std::sync::OnceLock;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::LoadLibraryW;

const MPV_DLL_NAME: &str = "libmpv-2.dll\0";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum MpvCapability {
    Available,
    Unavailable { reason: String },
}

impl MpvCapability {
    pub fn is_available(&self) -> bool {
        matches!(self, MpvCapability::Available)
    }
}

fn wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Probes for `libmpv-2.dll` WITHOUT touching any delay-loaded import - a
/// plain `LoadLibraryW`. The handle is intentionally leaked once found: this
/// app never wants the DLL unloaded mid-session, and the tiny leak beats a
/// use-after-unload the delay-load thunk could otherwise hit.
fn dll_present() -> bool {
    unsafe {
        match LoadLibraryW(PCWSTR(wide_z(MPV_DLL_NAME).as_ptr())) {
            Ok(handle) => {
                std::mem::forget(ManuallyDropHandle(handle));
                true
            }
            Err(_) => false,
        }
    }
}

// Prevents Rust from trying to do anything with the raw HMODULE beyond
// keeping the value alive for `std::mem::forget` above - HMODULE itself has
// no Drop impl, this wrapper only exists to make the intent explicit.
struct ManuallyDropHandle(#[allow(dead_code)] HMODULE);

fn run_probe() -> MpvCapability {
    if dll_present() {
        MpvCapability::Available
    } else {
        MpvCapability::Unavailable { reason: "libmpv-2.dll not found".into() }
    }
}

/// Cached for the app's session - re-probing per playback attempt would
/// re-pay `LoadLibraryW` for every single title on a machine where the
/// answer never changes.
static CAPABILITY: OnceLock<MpvCapability> = OnceLock::new();

pub fn check_available() -> MpvCapability {
    CAPABILITY.get_or_init(run_probe).clone()
}
