<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { playerStore } from '$lib/stores';
  import { mpvStartSession, mpvSetBounds, mpvStopSession, mpvGetSessionState } from '$lib/api';

  let {
    url,
    title,
    onready,
    onerror,
  }: {
    url: string;
    title: string;
    onready?: () => void;
    onerror?: (message: string) => void;
  } = $props();

  let containerEl: HTMLDivElement;
  let sessionId: string | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let unlistenPointer: UnlistenFn | null = null;
  let statusPoll: number | null = null;

  // The native window always covers the full video area and is never resized for overlay
  // UI - shrinking it on control-bar show/hide used to force mpv to rescale the whole
  // picture (visible "shrink then grow" glitch). Instead `currentExcludeRects()` clips
  // rects out via `mpv_set_bounds` -> Win32 `SetWindowRgn`, so HTML underneath shows
  // through and gets clicks while the video itself never moves.
  function currentBounds() {
    const rect = containerEl.getBoundingClientRect();
    return { x: rect.left, y: rect.top, width: Math.max(1, rect.width), height: Math.max(1, rect.height) };
  }

  // Sized for the tallest control-bar state (seek bar always shows - VOD only). Oversizing
  // just uncovers a sliver of black background; undersizing leaves part of the bar unclickable.
  const CONTROL_BAR_HEIGHT_PX = 120;
  // Covers NowPlayingOverlay's close button, permanently excluded since it has no
  // hover-to-reveal behavior. Oversized past its ~36px footprint for the same reason as above.
  const CLOSE_BUTTON_SIZE_PX = 64;

  function currentExcludeRects(rect: { x: number; y: number; width: number; height: number }) {
    const rects = [{ x: rect.x + rect.width - CLOSE_BUTTON_SIZE_PX - 8, y: rect.y + 8, width: CLOSE_BUTTON_SIZE_PX, height: CLOSE_BUTTON_SIZE_PX }];
    if (playerStore.controlsVisible) {
      rects.push({ x: rect.x, y: rect.y + rect.height - CONTROL_BAR_HEIGHT_PX, width: rect.width, height: CONTROL_BAR_HEIGHT_PX });
    }
    return rects;
  }

  async function pushBounds() {
    if (!sessionId || !containerEl) return;
    try {
      const bounds = currentBounds();
      await mpvSetBounds(sessionId, bounds, currentExcludeRects(bounds));
    } catch (err) {
      console.error('[EmbeddedMpvPlayer] mpv_set_bounds failed:', err);
    }
  }

  // mpv's `status` stays 'error' indefinitely once set; guards against re-calling
  // `onerror` on every 500ms poll while mounted waiting for the parent's retry.
  let errorReported = false;

  async function pollStatus() {
    if (!sessionId || errorReported) return;
    try {
      const state = await mpvGetSessionState(sessionId);
      if (state.status === 'error') {
        errorReported = true;
        onerror?.(state.error ?? 'mpv playback error');
      }
    } catch {
      // Session already gone (torn down elsewhere, e.g. a fast engine-
      // cascade retry) - not a playback error worth surfacing.
    }
  }

  onMount(() => {
    let cancelled = false;

    (async () => {
      try {
        const id = await mpvStartSession(url, title, currentBounds());
        if (cancelled) {
          // Destroyed (fast engine-cascade retry) before the async start
          // actually resolved - tear down immediately rather than leaking a
          // session/window nothing will ever stop.
          mpvStopSession(id).catch(() => {});
          return;
        }
        sessionId = id;
        playerStore.mpvSessionId = id;
        // `mpv_start_session` positions the window but applies no clip region - without
        // this call the close button stays covered until the async ResizeObserver fires.
        pushBounds();
        onready?.();
      } catch (err) {
        console.error('[EmbeddedMpvPlayer] mpv_start_session failed:', err);
        onerror?.(err instanceof Error ? err.message : String(err));
      }
    })();

    resizeObserver = new ResizeObserver(() => pushBounds());
    resizeObserver.observe(containerEl);
    document.addEventListener('fullscreenchange', pushBounds);

    listen<{ sessionId: string; kind: string }>('mpv-pointer', (event) => {
      if (event.payload.sessionId !== sessionId) return;
      if (event.payload.kind === 'move') {
        // Mirrors a real `<video>`'s `onmousemove` - a native window over
        // the video area swallows the DOM event entirely, so it has to be
        // forwarded from Rust (see `mpv_player::window`) instead.
        playerStore.showControlsTemporarily();
      } else if (event.payload.kind === 'dblclick') {
        playerStore.toggleFullscreen();
      }
      // "leave" is deliberately not wired to an instant hide here - see
      // `hideControlsNow`'s doc comment in `player.svelte.ts`.
    }).then((fn) => {
      if (cancelled) fn();
      else unlistenPointer = fn;
    });

    statusPoll = window.setInterval(pollStatus, 500);

    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    // Re-derive the clip region (not window bounds) on controls-visibility change, so
    // the control-bar rect gets excluded/re-covered as it shows/hides.
    void playerStore.controlsVisible;
    pushBounds();
  });

  onDestroy(() => {
    if (resizeObserver) resizeObserver.disconnect();
    document.removeEventListener('fullscreenchange', pushBounds);
    if (unlistenPointer) unlistenPointer();
    if (statusPoll !== null) clearInterval(statusPoll);
    if (sessionId) {
      const id = sessionId;
      sessionId = null;
      if (playerStore.mpvSessionId === id) playerStore.mpvSessionId = null;
      mpvStopSession(id).catch((err) => console.error('[EmbeddedMpvPlayer] mpv_stop_session failed:', err));
    }
  });
</script>

<div bind:this={containerEl} class="absolute inset-0"></div>
