# libmpv Windows dev files (build-time only, not committed)

`lib/mpv.lib` is an MSVC-compatible import library generated from the real
`libmpv-2.dll`'s export table. The upstream dev package
(`mpv-dev-x86_64-<date>-git-<hash>.7z` from
https://sourceforge.net/projects/mpv-player-windows/files/libmpv/) only ships
a GNU/MinGW-format `libmpv.dll.a`, which this project's MSVC linker
(`link.exe`, since `rustc --version --verbose` on this toolchain reports
`host: x86_64-pc-windows-msvc`) cannot consume directly.

`include/mpv/` is the upstream dev package's headers, unmodified - kept for
reference; `libmpv2-sys` ships its own pre-generated bindings and does not
need these to build.

## Reproducing `lib/mpv.lib`

1. Download and extract an `mpv-dev-x86_64-*.7z` (non-`v3` variant, for
   broader CPU compatibility) from the URL above. It contains
   `libmpv-2.dll`, `libmpv.dll.a`, and `include/mpv/*.h`.
2. Using the MSVC Build Tools (locate via `vswhere.exe -latest -find
   '**/dumpbin.exe'` / `'**/lib.exe'`):
   ```
   dumpbin /exports libmpv-2.dll > exports.txt
   ```
3. Turn the export list into a `.def` file (`EXPORTS` header + one symbol
   name per line, extracted from `exports.txt`'s 4-column export table rows).
4. ```
   lib /def:libmpv-2.def /out:mpv.lib /machine:x64
   ```
5. Copy the resulting `mpv.lib` here, and copy `libmpv-2.dll` itself to
   `../../mpv-runtime/` (the app's actual bundled runtime resource - see
   `tauri.conf.json`'s `bundle.resources` and `src-tauri/build.rs`).

`mpv-runtime/libmpv-2.dll` (~116MB) and `native-libs/mpv/lib/mpv.lib` are
fetched/generated build artifacts, not source. `mpv.lib` is tiny and committed
directly; the DLL is committed via Git LFS (see `.gitattributes`) so CI
Windows builds have it without re-running the steps above on every release.
