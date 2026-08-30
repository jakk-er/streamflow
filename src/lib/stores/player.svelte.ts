import { spawnExternalPlayer, killPlayer, getPlayerStatus } from '$lib/api';
import { formatError } from '$lib/utils/errors';
import type { VodResumeContext } from '$lib/types';

function createPlayerStore() {
  let currentUrl = $state<string | null>(null);
  let currentTitle = $state<string | null>(null);
  let playerType = $state<string | null>(null);
  /**
   * Pre-computed media extension for callers whose playable URL doesn't
   * carry it directly - e.g. a Stalker channel through the stream proxy,
   * where the real extension lives in the pre-proxy URL, not the
   * `127.0.0.1` one the player gets. `VideoPlayer` derives it from
   * `currentUrl` when unset.
   */
  let currentExtension = $state<string | null>(null);
  let externalSessionId = $state<string | null>(null);
  let isPlaying = $state(false);
  let isFullscreen = $state(false);
  /**
   * Which page's playback flow started the current stream - lets Live TV's
   * own inline player tell "mine" apart from "something else is playing",
   * and lets the global `NowPlayingOverlay` avoid rendering a second player
   * on top of it for the same URL.
   */
  let sourceKind = $state<'live' | 'vod' | null>(null);
  /**
   * The backend's own error text from the last failed `play()` (e.g. "mpv"
   * not found), surfaced by `NowPlayingOverlay` (mounted at the layout level
   * regardless of `currentUrl`). Previously only a `console.error` - a
   * failed `spawnExternalPlayer()` looked identical to nothing happening.
   */
  let lastError = $state<string | null>(null);
  /**
   * Set while an embedded (in-process) mpv session drives playback instead
   * of `<video>`/mpegts.js/hls.js - see `EmbeddedMpvPlayer.svelte`.
   * `VideoPlayer.svelte`/`PlayerControls.svelte` both branch on this since a
   * real native window, not a DOM element, is rendering.
   */
  let mpvSessionId = $state<string | null>(null);
  /**
   * Set only by VOD-inline call sites (`VodDetail.svelte`/`SeasonTabs.svelte`),
   * never by live playback or external players. `VideoPlayer.svelte` gates
   * its whole resume-tracking logic on this being non-null, so mpv/VLC and
   * live TV never touch watch-progress by construction.
   */
  let resumeContext = $state<VodResumeContext | null>(null);
  /**
   * Shared between `PlayerControls.svelte` (owns the hover/idle timer) and
   * `EmbeddedMpvPlayer.svelte` (needs to reserve height for the control bar
   * so it stays clickable - the native mpv window paints over and
   * intercepts input regardless of DOM z-index/hover).
   */
  let controlsVisible = $state(true);
  let controlsHideTimer: number | null = null;

  /** Shows the control bar and (re)starts the 3s idle-hide timer - single
   * source of truth for both `<video>`'s DOM `mousemove` and embedded mpv's
   * forwarded pointer-move, so the two never fight over separate timers. */
  function showControlsTemporarily() {
    controlsVisible = true;
    if (controlsHideTimer !== null) clearTimeout(controlsHideTimer);
    controlsHideTimer = window.setTimeout(() => {
      controlsVisible = false;
      controlsHideTimer = null;
    }, 3000);
  }

  /** Instant hide for `onmouseleave` off the whole player area - embedded
   * mpv's "pointer left" signal uses the timer above instead. */
  function hideControlsNow() {
    if (controlsHideTimer !== null) {
      clearTimeout(controlsHideTimer);
      controlsHideTimer = null;
    }
    controlsVisible = false;
  }

  let statusInterval: number | null = null;

  async function play(
    url: string,
    title?: string,
    type?: string,
    extension?: string,
    kind?: 'live' | 'vod',
    newResumeContext?: VodResumeContext
  ) {
    // Guards against a duplicate/overlapping call for content already
    // current - e.g. a double-click on Play before the first click's async
    // resolution finishes. Inline only (external/mpv/vlc re-launching on
    // purpose is legitimate): for embedded mpv each call opens a genuinely
    // independent upstream connection, and two landing close together
    // caused a real connection-storm incident.
    const isInline = type !== 'external' && type !== 'mpv' && type !== 'vlc';
    if (isInline && url === currentUrl && isPlaying) {
      return;
    }
    if (type === 'external' || type === 'mpv' || type === 'vlc') {
      try {
        const sessionId = await spawnExternalPlayer(type || 'mpv', url, title);
        lastError = null;
        externalSessionId = sessionId;
        statusInterval = window.setInterval(() => {
          pollStatus();
        }, 5000);
        currentUrl = url;
        currentTitle = title ?? null;
        playerType = type || 'mpv';
        currentExtension = extension ?? null;
        // Falls back to the existing value when `kind` is omitted - "open in
        // external player" re-plays the current stream under a new `type`
        // and shouldn't forget what kind of content it was.
        sourceKind = kind ?? sourceKind;
        isPlaying = true;
        // mpv/VLC must never create/modify resume state, regardless of caller.
        resumeContext = null;
      } catch (err) {
        console.error('Failed to spawn external player:', err);
        // Deliberately doesn't touch `currentUrl`/`playerType`/`isPlaying` -
        // whatever was already playing inline keeps playing on a failed
        // external-launch attempt.
        lastError = formatError(err);
        throw err;
      }
    } else {
      currentUrl = url;
      currentTitle = title ?? null;
      playerType = type ?? 'html5';
      currentExtension = extension ?? null;
      sourceKind = kind ?? sourceKind;
      isPlaying = true;
      lastError = null;
      resumeContext = newResumeContext ?? null;
    }
  }

  function clearError() {
    lastError = null;
  }

  async function stop() {
    if (externalSessionId) {
      if (statusInterval !== null) {
        clearInterval(statusInterval);
        statusInterval = null;
      }
      try {
        await killPlayer(externalSessionId);
      } catch (err) {
        console.error('Failed to kill player:', err);
      }
      externalSessionId = null;
    }
    currentUrl = null;
    currentTitle = null;
    playerType = null;
    sourceKind = null;
    currentExtension = null;
    isPlaying = false;
    mpvSessionId = null;
    resumeContext = null;
  }

  function toggleFullscreen() {
    isFullscreen = !isFullscreen;
  }

  async function pollStatus() {
    if (!externalSessionId) return;
    try {
      const running = await getPlayerStatus(externalSessionId);
      if (!running) {
        // Full reset (matches `stop()`) - leaving `currentUrl`/`playerType`
        // behind left the app in a stuck ambiguous state once mpv/VLC exited
        // on its own: "is something playing" checks still saw an external
        // session, so nothing could resume inline, yet nothing was actually
        // playing.
        if (statusInterval !== null) {
          clearInterval(statusInterval);
          statusInterval = null;
        }
        externalSessionId = null;
        currentUrl = null;
        currentTitle = null;
        playerType = null;
        sourceKind = null;
        currentExtension = null;
        isPlaying = false;
        resumeContext = null;
      }
    } catch (err) {
      console.error('Failed to get player status:', err);
    }
  }

  return {
    get currentUrl() { return currentUrl; },
    get currentTitle() { return currentTitle; },
    get playerType() { return playerType; },
    get currentExtension() { return currentExtension; },
    get sourceKind() { return sourceKind; },
    get externalSessionId() { return externalSessionId; },
    get isPlaying() { return isPlaying; },
    get isFullscreen() { return isFullscreen; },
    set isFullscreen(value: boolean) { isFullscreen = value; },
    get lastError() { return lastError; },
    get mpvSessionId() { return mpvSessionId; },
    set mpvSessionId(value: string | null) { mpvSessionId = value; },
    get resumeContext() { return resumeContext; },
    get controlsVisible() { return controlsVisible; },
    set controlsVisible(value: boolean) { controlsVisible = value; },
    showControlsTemporarily,
    hideControlsNow,
    play,
    stop,
    toggleFullscreen,
    clearError,
  };
}

export const playerStore = createPlayerStore();
