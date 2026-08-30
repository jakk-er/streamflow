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
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
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
}
