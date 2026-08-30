fn main() {
    tauri_build::build();

    // libmpv2-sys links against `mpv` (i.e. `mpv.lib` on the MSVC toolchain
    // this project builds with) via a plain `cargo:rustc-link-lib=mpv` in its
    // own build.rs - it expects that import library to already be on the
    // linker's search path, it doesn't vendor or generate one itself. The
    // real `libmpv-2.dll`'s dev package only ships a GNU/MinGW-format
    // `libmpv.dll.a`, which this project's MSVC linker (`link.exe`) cannot
    // consume directly - `native-libs/mpv/lib/mpv.lib` is a proper COFF
    // import library generated from `libmpv-2.dll`'s actual export table via
    // `dumpbin /exports` + `lib /def:...` (see native-libs/mpv/README.md for
    // the exact reproduction steps), not a copy of the upstream dev package.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if target_os == "windows" {
        let native_lib_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("native-libs/mpv/lib");
        println!("cargo:rustc-link-search=native={}", native_lib_dir.display());

        // `libmpv-2.dll` is a hard IMPLICIT load-time dependency by default
        // once linked (the loader resolves every implicit import before
        // `main()`/`tauri::Builder` ever runs) - if the DLL is missing at
        // runtime, the WHOLE APP fails to launch, not just embedded-mpv
        // playback, and no amount of `Result`/`try` in our own code can catch
        // that because it happens before any of our code runs. `/DELAYLOAD`
        // defers resolving libmpv-2.dll's imports to first actual call
        // instead, so `mpv_player::probe` can check the DLL is present
        // (`LoadLibraryW`) and report a clean "unavailable" - never touching
        // the delay-loaded import at all - rather than segfaulting the
        // process on first call in the missing-DLL case.
        println!("cargo:rustc-link-arg=/DELAYLOAD:libmpv-2.dll");
        println!("cargo:rustc-link-arg=delayimp.lib");
    }

    // Linux: no checked-in stub - links against the system `libmpv-dev` package directly (no MSVC/MinGW ABI mismatch to work around here).

    // macOS: prefers `native-libs/mpv-macos-runtime/libmpv.dylib` (CI copies Homebrew's dylib there and fixes its ID to
    // @rpath first - see release.yml - since Homebrew's own copy has an absolute /opt/homebrew/... ID baked in that
    // would otherwise get linked straight into the binary). Falls back to Homebrew's own copy for local dev builds
    // where that fixup hasn't been run - fine for testing, just not what a distributed release should link against.
    if target_os == "macos" {
        let fixed_copy = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("native-libs/mpv-macos-runtime");
        if fixed_copy.join("libmpv.dylib").is_file() {
            println!("cargo:rustc-link-search=native={}", fixed_copy.display());
        } else if let Ok(output) = std::process::Command::new("brew").args(["--prefix", "mpv"]).output() {
            if output.status.success() {
                let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
                println!("cargo:rustc-link-search=native={prefix}/lib");
            }
        }
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");
    }
}
