# Architecture

A high-level map of how StreamFlow is put together. For setup/dev commands, see [README.md](README.md); for contribution guidelines, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Overview

StreamFlow is a Tauri 2 desktop app: a Rust backend (`src-tauri/`) exposing typed commands over IPC, and a SvelteKit/Svelte 5 frontend (`src/`) that calls them. There's no separate server — the Rust backend, the local SQLite database, and a small local HTTP proxy all run in-process alongside the webview.

```
┌─────────────────────────────┐        IPC (invoke)        ┌───────────────────────────────┐
│  Frontend (SvelteKit/Svelte) │ ──────────────────────────▶ │  Backend (Rust / Tauri)        │
│  src/lib/{stores,api,...}    │ ◀────────────────────────── │  src-tauri/src/commands/*.rs   │
└─────────────────────────────┘                              └──────────────┬─────────────────┘
                                                                              │
                              ┌────────────────────────────┬────────────────┼─────────────────┐
                              ▼                             ▼                ▼                 ▼
                     SQLite (r2d2 pool)         Provider clients (net/)   Stream proxy    Embedded mpv
                     src-tauri/src/db/          M3U / Xtream / Stalker    (axum, local)   src-tauri/src/mpv_player/
```

## Backend (`src-tauri/src`)

- **`commands/`** — one `#[tauri::command]` per frontend-callable operation (playlists, channels, EPG, VOD, favorites, downloads, settings, player control, Stalker- and Xtream-specific auth/catalog calls). Each command is a thin async wrapper: validate input, call into `net/`/`db/`, map errors to `CommandError`.
- **`net/`** — HTTP clients for each provider protocol:
  - `net/xtream.rs` — Xtream Codes API (JSON).
  - `net/stalker/` — Stalker/Ministra portal API (MAC-based device auth, token refresh, category/content listing, `create_link` playback resolution). Split into `auth.rs` (handshake/login), `content.rs` (catalog/EPG), `identity.rs` (device-id derivation, request signing).
  - `net/downloader.rs` — streams a VOD file to disk for the offline-download feature.
- **`db/`** — one module per table/domain (`channels`, `vod`, `favorites`, `epg`, `playlists`, `downloads`, ...) over a `r2d2`-pooled SQLite connection (WAL mode). `db/schema.rs` owns migrations.
- **`parsers/`** — M3U and XMLTV parsing (M3U has no separate VOD API; movies/series there are just channels with a `group-title`).
- **`mpv_player/`** (Windows only) — embeds `libmpv2` as a native child window for content the browser's `<video>` element can't demux (e.g. MKV/HEVC), with its own session lifecycle (`session.rs`), capability probing (`probe.rs`), and window management (`window.rs`).
- **`stream_proxy.rs`** — a local `axum` HTTP server that relays playback URLs same-origin, attaching provider auth headers server-side. Used for every provider, not just Stalker — most reseller panels don't send permissive CORS headers for direct playback either.
- **`catchup.rs`** — resolves "watch from start" archive/timeshift URLs for both M3U's `tvg-rec`-style and Xtream's native catch-up.
- **`scheduler.rs`** — background jobs (auto-refresh, periodic sync).
- **`types/`** — shared request/response/DB-row shapes, serialized `camelCase` to match the frontend.

### Provider normalization

M3U, Xtream, and Stalker are three different protocols with different auth models and catalog shapes. The backend normalizes all of them into one `Channel` shape for Live TV and one `VodCatalogItem` shape for Movies/Series before they ever reach the frontend — the UI layer doesn't branch on provider type except where playback resolution genuinely differs (e.g. Stalker's `create_link`).

### Why a local stream proxy

Browsers enforce CORS and can't attach custom auth headers to a `<video>`/HLS request. The proxy runs on `127.0.0.1`, so the webview's request to it is same-origin (no CORS issue), and the proxy attaches whatever headers/tokens the upstream provider needs before forwarding.

## Frontend (`src`)

- **`routes/`** — SvelteKit pages: dashboard, Live TV, VOD (browse + detail), Favorites, History, Settings.
- **`lib/stores/`** — one Svelte 5 rune-based store per domain (`channel`, `vod`, `playlist`, `player`, `favorites`, `epg`, `settings`, Stalker/Xtream session state). Stores own all async state (loading/error/data) and talk to the backend exclusively through `lib/api/`.
- **`lib/api/`** — thin `invoke()` wrappers, one file per backend command group, typed against `lib/types/`.
- **`lib/components/`** — organized by feature area (`channel/`, `vod/`, `player/`, `epg/`, `settings/`, `favorites/`, `dashboard/`, `layout/`, `ui/`).
- **`lib/utils/`** — pure helpers (error formatting, stream-proxy URL wrapping, playback extension detection, episode-playback resolution shared between manual play and auto-advance).

### Playback engine selection

`VideoPlayer.svelte` picks an engine per stream based on its container/extension: mpegts.js for raw `.ts` (typical for live TV), hls.js for `.m3u8`, native `<video>` for anything else the browser can demux, and the embedded mpv engine as a fallback for containers Chromium can't (MKV and similar). A failure at one engine can trigger a fallback attempt at the next, tracked so it never loops.

### Stalker's lazy-loading tradeoff

Stalker portals have no bulk "get everything" catalog endpoint that reliably returns all categories, and no id-based single-item lookup. So VOD/series browsing there is deliberately lazy: categories/pages are fetched live as the user scrolls, opportunistically cached into the local `vod_items` table as a side effect. This trades catalog completeness on first browse for a fast "click a category, see results immediately" experience — see `vod.svelte.ts` and `commands/vod.rs`'s `vod_get_items_live` for the details, including the portal-wide search and DB-cache-fallback mechanisms built on top of it.

## Data flow example: playing a live channel

1. User clicks a channel → `channelStore.selectChannel` → `+page.svelte`'s `playChannel`.
2. If Stalker, `stalkerResolvePlayback` calls the backend, which calls the portal's `create_link` to get a real (temporary) stream URL; M3U/Xtream URLs are already playable.
3. The resolved URL is wrapped through the local stream proxy (`wrapUrlThroughStreamProxy`).
4. `playerStore.play(...)` hands the proxied URL to `VideoPlayer.svelte`, which picks a playback engine and starts playback.
