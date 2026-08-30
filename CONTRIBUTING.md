# Contributing to StreamFlow

Thanks for considering a contribution. This is a small desktop app, so the process is intentionally lightweight.

## Dev setup

```bash
npm install
npm run tauri dev
```

Requires Node.js 18+, a stable Rust toolchain (1.77+), and your platform's [Tauri prerequisites](https://tauri.app/start/prerequisites/). See [README.md](README.md) for details, and [ARCHITECTURE.md](ARCHITECTURE.md) for how the codebase is organized before diving in.

**Never build a release with `cargo build --release` directly** — it skips the frontend build step. Always go through `npm run tauri build`.

## Before opening a PR

- `npm run lint` (svelte-check) and `npm run check` (tsc) for the frontend.
- `cd src-tauri && cargo check` (and `cargo test` if you touched `db/` or `net/`, where most of the test coverage lives) for the backend.
- Test the actual feature in the running app, not just the type checkers — they catch type errors, not behavior.

## Code style

- **Comments explain *why*, not *what*.** Keep them short — a sentence or two for a real non-obvious constraint (ordering requirement, protocol quirk, bug workaround). Don't narrate the investigation that led to a fix, and don't restate what the code already makes clear.
- **Svelte 5 runes** (`$state`, `$derived`, `$effect`) throughout the frontend — no legacy `$:` reactive statements or Svelte 4 store syntax.
- **Provider-agnostic where possible.** M3U/Xtream/Stalker differences belong in `net/`/`commands/` on the backend; the frontend and shared types should stay provider-agnostic except where playback resolution genuinely differs.
- Match the formatting/structure already in the file you're editing rather than introducing a new style.

## Reporting issues

Include your provider type (M3U / Xtream / Stalker), what you expected vs. what happened, and any relevant console/log output. Portal-specific bugs (especially Stalker, where server implementations vary a lot) are much easier to fix with a description of what the portal actually returned.

## Pull requests

- Keep PRs focused — one feature or fix per PR is easier to review than a bundle of unrelated changes.
- Describe *why* the change is needed, not just what it does.
- If it's a behavior change (not just a refactor), mention how you tested it.
