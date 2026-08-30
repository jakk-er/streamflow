<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { playerStore, playlistStore } from '$lib/stores';
  import Icon from '$lib/components/ui/Icon.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
  import Hls from 'hls.js';
  import Mpegts from 'mpegts.js';
  import { getPlaybackMediaExtensionFromUrl } from '$lib/utils/playbackMediaExtension';
  import { vodSaveProgress, vodClearProgress, mpvGetSessionState, mpvSeek } from '$lib/api';
  import { resolveEpisodeUrl } from '$lib/utils/vodEpisodePlayback';
  import type { VodResumeContext } from '$lib/types';
  import EmbeddedMpvPlayer from './EmbeddedMpvPlayer.svelte';
  import PlayerControls from './PlayerControls.svelte';

  // mpegts.js has its own internal logging system, independent of `console.*` level -
  // silences its verbose per-chunk demuxer/loader narration. Idempotent, safe on every mount.
  Mpegts.LoggingControl.applyConfig({
    enableVerbose: false,
    enableDebug: false,
    enableInfo: false,
    enableWarn: false,
    enableError: true,
  });

  let videoRef: HTMLVideoElement;
  let hlsInstance: Hls | null = null;
  let mpegtsPlayer: Mpegts.Player | null = null;
  let error = $state<string | null>(null);
  let loading = $state(true);
  let retryCount = 0;
  let retryTimer: number | null = null;

  // Live IPTV connections routinely drop/reconnect on their own (edge node cycling) - not
  // fatal. Retrying for closer to a minute before giving up (matching VLC/Kodi/iptvnator)
  // avoids surfacing normal reconnects as a dead "Playback Error" screen.
  const MAX_AUTO_RETRIES = 10;
  const RETRY_DELAYS_MS = [500, 1000, 2000, 3000, 5000];
  // Low-tier Stalker/Ministra panels often cap concurrent connections per MAC and reject
  // a slot conflict with a plain 5xx/429 rather than a real network error - retrying fast
  // just re-hits the still-full slot. Backing off harder gives it time to free up.
  const SERVER_REJECTION_RETRY_DELAYS_MS = [2000, 4000, 6000, 8000, 10000];
  const MAX_SERVER_REJECTION_RETRIES = 5;
  let lastErrorWasServerRejection = false;

  // Extension-based engine selection is a best-effort guess (panels routinely mislabel
  // streams) - if the guessed engine rejects the content, cascade through the others once
  // each: mpegts.js -> hls.js -> native <video> -> mpv, skipping the initial guess.
  // `attemptedEngines` prevents re-trying the same engine twice (which would otherwise
  // loop forever between two engines both rejecting the same content).
  // Chromium never supports Matroska at all (demuxer gap) - native `<video>` is a
  // guaranteed failure there, not a guess, and such VOD usually needs mpv anyway
  // (HEVC/AC3/DTS rips). VOD-only; live's cascade is untouched (see `setupPlayer()`).
  const NATIVE_INCOMPATIBLE_VOD_EXTENSIONS = new Set(['mkv', 'avi', 'wmv', 'flv', 'vob', 'mpeg']);

  let engineOverride: 'mpegts' | 'hls' | 'native' | 'mpv' | null = null;
  let attemptedEngines = new Set<'mpegts' | 'hls' | 'native' | 'mpv'>();
  /** True only while the embedded mpv engine is mounted - gates `<EmbeddedMpvPlayer>` and
   * CSS-hides (not unmounts) native `<video>` so `videoRef`'s binding survives engine switches. */
  let mpvEngineActive = $state(false);
  /** Bumped on every `setupPlayer()` call to force `{#key}` to fully recreate
   * `<EmbeddedMpvPlayer>` even on a same-URL mpv retry - otherwise flipping
   * `mpvEngineActive` false-then-true within one synchronous call can get coalesced by
   * Svelte's batching, silently reusing an already-stopped session. */
  let mpvMountKey = $state(0);
  /** Plain (non-reactive) source for `mpvMountKey`'s next value - NOT `mpvMountKey += 1`.
   * That form reads `mpvMountKey` inside the url-watching `$effect` that also writes it,
   * which made the effect depend on its own write and retrigger itself infinitely (caused
   * a real runaway `mpv_start_session` loop). Only ever writing this plain counter avoids
   * the self-dependency. */
  let mpvMountKeyCounter = 0;

  function tryNextEngine(next: 'mpegts' | 'hls' | 'native' | 'mpv'): boolean {
    if (attemptedEngines.has(next)) return false;
    engineOverride = next;
    destroyPlayer();
    setupPlayer();
    return true;
  }

  // `allowed_output_formats` is an account-level claim, not per-stream - the format picked
  // at import time can be wrong for one channel. Rather than cascade formats live (too
  // noisy, shouldn't yank a stream mid-play over a blip), this is a quiet STARTUP-ONLY
  // safety net: if the first attempt never renders a frame, swap `.ts`<->`.m3u8` once and
  // retry before falling into normal error UI. `hasRenderedFrame` marks startup as over -
  // this never fires again for the URL past that point. Wastes one retry cycle on a
  // genuine provider rejection (bad creds etc, looks identical), but recovers real
  // per-channel format mismatches, so it stays.
  let hasRenderedFrame = false;
  let formatFallbackAttempted = false;
  /** True only for the url-change triggered by `tryFormatFallbackOnce` itself, so the
   * effect below doesn't reset `formatFallbackAttempted` and loop the swap forever. Any
   * other url change still gets a fresh attempt. */
  let isFormatFallbackReplay = false;

  function buildAlternateFormatUrl(proxiedUrl: string): { proxiedUrl: string; extension: 'ts' | 'm3u8' } | null {
    let parsed: URL;
    try {
      parsed = new URL(proxiedUrl);
    } catch {
      return null;
    }
    const providerUrl = parsed.searchParams.get('url');
    const playlistId = parsed.searchParams.get('playlist_id');
    if (!providerUrl || !playlistId) return null;

    let swappedProviderUrl: string;
    let extension: 'ts' | 'm3u8';
    if (/\.m3u8(\?|$)/i.test(providerUrl)) {
      swappedProviderUrl = providerUrl.replace(/\.m3u8(\?|$)/i, '.ts$1');
      extension = 'ts';
    } else if (/\.ts(\?|$)/i.test(providerUrl)) {
      swappedProviderUrl = providerUrl.replace(/\.ts(\?|$)/i, '.m3u8$1');
      extension = 'm3u8';
    } else {
      return null;
    }

    const newParams = new URLSearchParams({ playlist_id: playlistId, url: swappedProviderUrl });
    return { proxiedUrl: `${parsed.origin}${parsed.pathname}?${newParams.toString()}`, extension };
  }

  /** Returns true if a format swap was actually attempted (caller must stop
   * and let the URL change re-trigger playback rather than also scheduling
   * a normal retry). */
  function tryFormatFallbackOnce(): boolean {
    // Live-only: a live Xtream channel can genuinely have both a `.ts` and `.m3u8` URL for
    // the same content, so swapping is a real alternate endpoint. A VOD title has no such
    // twin (exactly one `container_extension`) - swapping there fabricates a URL that was
    // never real, wasting a retry cycle chasing a 404 on a genuinely broken declaration.
    if (playerStore.sourceKind !== 'live') return false;
    if (hasRenderedFrame || formatFallbackAttempted || !url) return false;
    const swapped = buildAlternateFormatUrl(url);
    if (!swapped) return false;
    formatFallbackAttempted = true;
    isFormatFallbackReplay = true;
    playerStore.play(swapped.proxiedUrl, playerStore.currentTitle ?? undefined, undefined, swapped.extension, playerStore.sourceKind ?? undefined);
    return true;
  }

  let url = $derived(playerStore.currentUrl);
  let isFullscreen = $derived(playerStore.isFullscreen);

  // ---------------------------------------------------------------------
  // Resume/Continue-Watching tracking - active only when `playerStore.resumeContext` is
  // set (VOD-inline only; live TV and external mpv/VLC never set it).
  // ---------------------------------------------------------------------
  const SAVE_PROGRESS_INTERVAL_MS = 10_000;
  // Threshold for showing the (non-auto) "Next Episode" button - deliberately
  // tighter than the true-completion check below, so it only appears once
  // the episode is genuinely almost over rather than several minutes early.
  const NEXT_EPISODE_BUTTON_MIN_REMAINING_SECONDS = 60;
  const NEXT_EPISODE_BUTTON_DURATION_FRACTION = 0.03;
  // "Actually finished" for the polling fallback (mpv has no DOM `ended` event, so this is
  // its only completion signal; other engines trigger primarily off the real event).
  const TRUE_END_REMAINING_SECONDS = 2;

  let saveProgressInterval: number | null = null;
  // Plain mirror of `playerStore.resumeContext`, not read live - by the time the
  // url-watching effect re-runs to do a final save, the store already reflects the NEW
  // title/episode, not the one that just finished.
  let trackedContext: VodResumeContext | null = null;
  let hasSeekedToResumePosition = false;
  let completionHandled = false;
  let showNextEpisodeButton = $state(false);
  let nextEpisodeButtonTitle = $state('');
  // Set when the user dismisses the button for the current episode; reset per-URL.
  // Doesn't affect auto-advance, which is tied only to true completion.
  let nextEpisodeButtonDismissed = false;

  function stopSaveProgressInterval() {
    if (saveProgressInterval !== null) {
      clearInterval(saveProgressInterval);
      saveProgressInterval = null;
    }
  }

  function startSaveProgressInterval() {
    stopSaveProgressInterval();
    if (!trackedContext) return;
    saveProgressInterval = window.setInterval(() => {
      void saveCurrentProgress();
    }, SAVE_PROGRESS_INTERVAL_MS);
  }

  /** Reads the actual current position/duration from whichever engine is
   * live right now - `mpvGetSessionState` for the embedded mpv session (no
   * DOM element drives playback there), `videoRef` directly otherwise. */
  async function readCurrentPlaybackTime(): Promise<{ position: number; duration: number } | null> {
    if (mpvEngineActive && playerStore.mpvSessionId) {
      try {
        const state = await mpvGetSessionState(playerStore.mpvSessionId);
        if (!state.durationSeconds || state.durationSeconds <= 0) return null;
        return { position: state.positionSeconds, duration: state.durationSeconds };
      } catch {
        return null;
      }
    }
    if (videoRef && Number.isFinite(videoRef.duration) && videoRef.duration > 0) {
      return { position: videoRef.currentTime, duration: videoRef.duration };
    }
    return null;
  }

  /** Whichever of 3%-of-duration or 60s remaining is larger for this title's length -
   * governs only when the manual "Next Episode" button appears, never auto-advance. */
  function hasReachedNearEndThreshold(position: number, duration: number): boolean {
    const remaining = duration - position;
    return remaining <= Math.max(duration * NEXT_EPISODE_BUTTON_DURATION_FRACTION, NEXT_EPISODE_BUTTON_MIN_REMAINING_SECONDS);
  }

  /** True completion - the only thing allowed to trigger auto-advance. Polling fallback
   * that covers mpv (no DOM `ended`) and catches anything the real event might miss. */
  function hasReachedTrueEnd(position: number, duration: number): boolean {
    return duration - position <= TRUE_END_REMAINING_SECONDS;
  }

  async function saveCurrentProgress() {
    const ctx = trackedContext;
    if (!ctx) return;
    const timing = await readCurrentPlaybackTime();
    if (!timing) return;

    if (hasReachedTrueEnd(timing.position, timing.duration)) {
      await handleCompletion(ctx);
      return;
    }

    if (
      !completionHandled &&
      !showNextEpisodeButton &&
      !nextEpisodeButtonDismissed &&
      ctx.contentType === 'series' &&
      (ctx.upcomingEpisodes?.length ?? 0) > 0 &&
      hasReachedNearEndThreshold(timing.position, timing.duration)
    ) {
      showNextEpisodeButton = true;
      nextEpisodeButtonTitle = ctx.upcomingEpisodes![0].title;
    }

    try {
      await vodSaveProgress({
        playlistId: ctx.playlistId,
        contentType: ctx.contentType,
        vodItemId: ctx.vodItemId,
        episodeId: ctx.episode?.id,
        seasonNumber: ctx.episode?.seasonNumber,
        episodeNumber: ctx.episode?.episodeNumber,
        episodeTitle: ctx.episode?.title,
        positionSeconds: Math.floor(timing.position),
        totalSeconds: Math.floor(timing.duration),
        title: ctx.title,
        cover: ctx.cover,
      });
    } catch (err) {
      console.error('Failed to save watch progress:', err);
    }
  }

  /** Movie: clears progress entirely (a completed movie shows "Play", not "Resume").
   * Series with a next episode queued: auto-advances immediately (only runs on true
   * completion, no countdown). Series with nothing left: clears progress too. */
  async function handleCompletion(ctx: VodResumeContext) {
    if (completionHandled) return;
    completionHandled = true;
    stopSaveProgressInterval();
    showNextEpisodeButton = false;

    if (ctx.contentType === 'movie') {
      try {
        await vodClearProgress(ctx.playlistId, 'movie', ctx.vodItemId);
      } catch (err) {
        console.error('Failed to clear watch progress:', err);
      }
      return;
    }

    const [next, ...rest] = ctx.upcomingEpisodes ?? [];
    if (!next) {
      try {
        await vodClearProgress(ctx.playlistId, 'series', ctx.vodItemId);
      } catch (err) {
        console.error('Failed to clear watch progress:', err);
      }
      return;
    }

    await playNextEpisode(ctx, next, rest);
  }

  /** User clicked "Next Episode" before true completion - jumps now. Reuses
   * `completionHandled` so a stray `ended`/poll tick can't also fire and double-advance. */
  function playNextEpisodeNow() {
    const ctx = trackedContext;
    if (!ctx || completionHandled) return;
    const [next, ...rest] = ctx.upcomingEpisodes ?? [];
    if (!next) return;
    completionHandled = true;
    stopSaveProgressInterval();
    showNextEpisodeButton = false;
    void playNextEpisode(ctx, next, rest);
  }

  /** Just hides the button - auto-advance at true completion still happens regardless. */
  function dismissNextEpisodeButton() {
    showNextEpisodeButton = false;
    nextEpisodeButtonDismissed = true;
  }

  async function playNextEpisode(
    ctx: VodResumeContext,
    next: NonNullable<VodResumeContext['upcomingEpisodes']>[number],
    rest: NonNullable<VodResumeContext['upcomingEpisodes']>
  ) {
    const playlist = playlistStore.playlists.find((p) => p._id === ctx.playlistId);
    try {
      const resolved = await resolveEpisodeUrl(ctx.playlistId, playlist?.playlistType === 'stalker', next);
      if (!resolved) return;
      const newContext: VodResumeContext = {
        playlistId: ctx.playlistId,
        contentType: 'series',
        vodItemId: ctx.vodItemId,
        title: ctx.title,
        cover: ctx.cover,
        episode: { id: next.id, seasonNumber: next.seasonNumber, episodeNumber: next.episodeNumber, title: next.title },
        startPositionSeconds: 0,
        upcomingEpisodes: rest,
      };
      await playerStore.play(resolved.url, next.title, undefined, resolved.extension, 'vod', newContext);
    } catch (err) {
      console.error('Failed to auto-advance to the next episode:', err);
    }
  }

  /** Seeks to `resumeContext.startPositionSeconds` once, the first moment each engine can
   * accept a seek - called from `onPlaybackStarted`, guarded to run once per URL. */
  function seekToResumePositionIfNeeded() {
    if (hasSeekedToResumePosition) return;
    hasSeekedToResumePosition = true;
    const startAt = playerStore.resumeContext?.startPositionSeconds;
    if (!startAt || startAt <= 0) return;
    if (mpvEngineActive && playerStore.mpvSessionId) {
      mpvSeek(playerStore.mpvSessionId, startAt).catch((err) => console.error('Failed to seek to resume position:', err));
    } else if (videoRef) {
      videoRef.currentTime = startAt;
    }
  }

  function scheduleRetry() {
    if (retryTimer !== null) return;
    // Show the spinner during an automatic reconnect instead of leaving the
    // last frozen frame on screen - a silent reconnect attempt looks
    // identical to a genuine hang otherwise.
    loading = true;
    const delays = lastErrorWasServerRejection ? SERVER_REJECTION_RETRY_DELAYS_MS : RETRY_DELAYS_MS;
    const delay = delays[Math.min(retryCount, delays.length - 1)];
    retryCount += 1;
    retryTimer = window.setTimeout(() => {
      retryTimer = null;
      destroyPlayer();
      setupPlayer();
    }, delay);
  }

  function handleFatalError(message: string) {
    if (tryFormatFallbackOnce()) return;
    const maxRetries = lastErrorWasServerRejection ? MAX_SERVER_REJECTION_RETRIES : MAX_AUTO_RETRIES;
    if (retryCount < maxRetries) {
      scheduleRetry();
      return;
    }
    error = message;
    loading = false;
  }

  // 503/429 mean the portal refused the connection (commonly a per-MAC concurrent-
  // connection cap) - not a network fault worth retrying fast.
  function isServerRejectionStatus(code: unknown): boolean {
    return typeof code === 'number' && (code === 429 || (code >= 500 && code < 600));
  }

  // 401/403/404 are deterministic - the same request gets the same rejection every time,
  // so hls.js's own retry budget can never succeed on it. Distinct from
  // `isServerRejectionStatus`: that means "retry slowly", this means "switch engine now".
  function isPermanentRejectionStatus(code: unknown): boolean {
    return typeof code === 'number' && (code === 401 || code === 403 || code === 404);
  }

  function manualRetry() {
    retryCount = 0;
    lastErrorWasServerRejection = false;
    engineOverride = null;
    attemptedEngines = new Set();
    hasRenderedFrame = false;
    formatFallbackAttempted = false;
    destroyPlayer();
    setupPlayer();
  }

  function setupPlayer() {
    if (!url || !videoRef) return;

    error = null;
    loading = true;

    // Stalker (and many IPTV) live channels are raw MPEG-TS (`extension=ts` in the query
    // string), not HLS - hls.js can't play those. `currentExtension` wins when set: a
    // Stalker URL routed through the stream proxy carries its real extension inside its
    // `url` query value, not the outer URL, so callers that know it pass it explicitly.
    const extension = playerStore.currentExtension ?? getPlaybackMediaExtensionFromUrl(url);
    const isLive = playerStore.sourceKind === 'live';

    // An undetected extension on VOD almost always means a standard progressive container
    // native `<video>` already understands, not raw MPEG-TS - so the "no extension ->
    // assume raw ts" fallback is live-only. Applying it to VOD used to feed real MP4/MKV
    // into mpegts.js's TS demuxer, which rejected them and left the player spinning forever.
    const assumeRawTs = extension === 'ts' || (!extension && isLive);

    // VOD in a container Chromium can never demux (see `NATIVE_INCOMPATIBLE_VOD_EXTENSIONS`)
    // - native `<video>`'s failure there is guaranteed, so skip straight to mpv. VOD only.
    const needsMpv = !isLive && !!extension && NATIVE_INCOMPATIBLE_VOD_EXTENSIONS.has(extension);

    // `engineOverride` (set by `tryNextEngine` after the initially-guessed
    // engine rejected the content) wins over the extension heuristic - it
    // reflects an actual observed failure, not a guess.
    const engine: 'mpegts' | 'hls' | 'native' | 'mpv' =
      engineOverride ?? (needsMpv ? 'mpv' : assumeRawTs ? 'mpegts' : extension === 'm3u8' ? 'hls' : 'native');
    attemptedEngines.add(engine);
    mpvEngineActive = engine === 'mpv';
    mpvMountKey = ++mpvMountKeyCounter;

    if (engine === 'mpegts' && Mpegts.isSupported()) {
      // No `headers` config here (or for hls.js below): credentialed URLs are routed
      // through the local stream proxy, which attaches them server-side same-origin.
      mpegtsPlayer = Mpegts.createPlayer(
        { type: 'mpegts', isLive, url },
        {
          // Demuxing/remuxing off the main thread, so decode work doesn't compete with UI
          // rendering and cause jank on lower-end machines.
          enableWorker: true,
          // Live-only tuning: bounds SourceBuffer memory growth, and disables
          // `liveBufferLatencyChasing`'s hard-seek-to-live-edge behavior, which was firing
          // constantly on cheap reseller CDN jitter and causing a visible "pause then
          // jump" every time buffered content drifted past the latency threshold. Passive
          // TV viewing doesn't need to hug the live edge, so letting it drift is smoother.
          // `stashInitialSize` enlarges the IO loader's read-ahead cushion so a bursty CDN
          // can't drain it faster than it refills and starve the demuxer between chunks.
          ...(isLive
            ? {
                autoCleanupSourceBuffer: true,
                autoCleanupMaxBackwardDuration: 30,
                autoCleanupMinBackwardDuration: 15,
                liveBufferLatencyChasing: false,
                enableStashBuffer: true,
                stashInitialSize: 1024 * 1024,
              }
            : {}),
        }
      );
      mpegtsPlayer.attachMediaElement(videoRef);
      mpegtsPlayer.on(Mpegts.Events.ERROR, (type: string, details: string, info: any) => {
        const wrongFormat =
          details === Mpegts.ErrorDetails.MEDIA_FORMAT_UNSUPPORTED ||
          details === Mpegts.ErrorDetails.MEDIA_FORMAT_ERROR ||
          details === Mpegts.ErrorDetails.MEDIA_CODEC_UNSUPPORTED;
        if (wrongFormat && (tryNextEngine('hls') || tryNextEngine('native') || tryNextEngine('mpv'))) {
          return;
        }
        // `NETWORK_UNRECOVERABLE_EARLY_EOF` is what a live reconnect looks like from
        // mpegts.js's side (an aborted fetch body surfaces as a real ERROR, not
        // `LOADING_COMPLETE`, which is reserved for a clean upstream close). Treated as a
        // server rejection (slow retry) since it's a brand-new upstream connection and
        // retrying fast risks re-requesting before the provider released the old slot.
        const earlyEof = details === Mpegts.ErrorDetails.NETWORK_UNRECOVERABLE_EARLY_EOF;
        lastErrorWasServerRejection =
          earlyEof || (details === Mpegts.ErrorDetails.NETWORK_STATUS_CODE_INVALID && isServerRejectionStatus(info?.code));
        handleFatalError('Playback error: ' + type);
      });
      // A live TS feed through our own stream proxy never carries a Content-Length (open-
      // ended relay), so when the upstream connection is cut for any reason, the loader's
      // `reader.read()` resolves `{done: true}` and mpegts.js treats that as a normal
      // "download complete" - firing `LOADING_COMPLETE`, not `ERROR`. Left unhandled that
      // silently freezes the frame with no spinner/error/reconnect. VOD's the case where
      // `LOADING_COMPLETE` is legitimate, so this only reconnects for `isLive`.
      mpegtsPlayer.on(Mpegts.Events.LOADING_COMPLETE, () => {
        if (!isLive) return;
        // Slow schedule: a live disconnect means a brand-new TCP connection has to be
        // re-established, and many Xtream/Stalker panels cap concurrent connections per
        // account - reconnecting fast risks re-requesting before the old slot is released
        // server-side (looks like: connects, parses a PAT/PMT, dies again, repeat).
        lastErrorWasServerRejection = true;
        handleFatalError('Stream ended unexpectedly');
      });
      mpegtsPlayer.load();
      // Not clearing `loading` here: `.load()` only starts the fetch, no frame has
      // rendered yet. `handlePlaying` is the single source of truth for that.
    } else if (engine === 'hls' && Hls.isSupported()) {
      // Narrowed to explicit `.m3u8`/HLS detection - previously any non-mp4/mpv extension
      // routed into hls.js's manifest parser, including VOD containers native `<video>`
      // already plays directly.
      hlsInstance = new Hls({
        // Bounds memory growth on long-running live sessions, same idea as mpegts.js's
        // autoCleanupSourceBuffer above.
        backBufferLength: 30,
        // hls.js already retries fragment/manifest blips internally but gives up fast by
        // default; a longer budget absorbs brief CDN jitter before this component retries.
        fragLoadingMaxRetry: 6,
        levelLoadingMaxRetry: 6,
        manifestLoadingMaxRetry: 6,
      });
      hlsInstance.loadSource(url);
      hlsInstance.attachMedia(videoRef);
      hlsInstance.on(Hls.Events.ERROR, (event: any, data: any) => {
        // 401/403/404 checked before `!data.fatal` and reacted to on the first occurrence,
        // not after hls.js exhausts its own retry budget - a permission/not-found
        // rejection is deterministic, so waiting out 6 retries just stalls needlessly.
        // `stopLoad()` first so the old instance's in-flight retry can't keep firing this
        // listener after `tryNextEngine` has already torn it down.
        const permanentRejection =
          (data.details === Hls.ErrorDetails.FRAG_LOAD_ERROR ||
            data.details === Hls.ErrorDetails.LEVEL_LOAD_ERROR ||
            data.details === Hls.ErrorDetails.MANIFEST_LOAD_ERROR) &&
          isPermanentRejectionStatus(data.response?.code);
        if (permanentRejection) {
          hlsInstance?.stopLoad();
          if (tryNextEngine('mpegts') || tryNextEngine('native') || tryNextEngine('mpv')) return;
        }

        if (!data.fatal) return;
        // Beyond manifest load/parse failure, a manifest that DOES parse can still be the
        // wrong content: FRAG_PARSING_ERROR (segments aren't valid HLS media, e.g. raw TS)
        // and the codec-incompatible errors are all "wrong engine" signals worth cascading on.
        const wrongFormat =
          data.details === Hls.ErrorDetails.MANIFEST_PARSING_ERROR ||
          data.details === Hls.ErrorDetails.MANIFEST_LOAD_ERROR ||
          data.details === Hls.ErrorDetails.FRAG_PARSING_ERROR ||
          data.details === Hls.ErrorDetails.MANIFEST_INCOMPATIBLE_CODECS_ERROR ||
          data.details === Hls.ErrorDetails.BUFFER_INCOMPATIBLE_CODECS_ERROR;
        if (wrongFormat && (tryNextEngine('mpegts') || tryNextEngine('native') || tryNextEngine('mpv'))) {
          return;
        }
        lastErrorWasServerRejection = isServerRejectionStatus(data.response?.code);
        handleFatalError('Playback error: ' + data.type);
      });
      hlsInstance.on(Hls.Events.MANIFEST_PARSED, () => {
        retryCount = 0;
        loading = false;
      });
    } else if (engine === 'mpv') {
      // No `videoRef` wiring - `<EmbeddedMpvPlayer>` (gated on `mpvEngineActive`) owns the
      // session lifecycle; `loading` clears via its `onready`, errors route through
      // `onerror` into the same `tryNextEngine`/`handleFatalError` path as other engines.
    } else {
      // Common path for VOD movies/episodes - native decoder, no demuxer needed. Loading
      // clears on the real `playing` event via `handlePlaying`, not here.
      videoRef.src = url;
    }

    if (engine !== 'mpv') {
      videoRef.play().catch(() => {
        // Autoplay may be blocked
      });
    }
  }

  function destroyPlayer() {
    if (retryTimer !== null) {
      clearTimeout(retryTimer);
      retryTimer = null;
    }
    if (mpegtsPlayer) {
      mpegtsPlayer.pause();
      mpegtsPlayer.unload();
      mpegtsPlayer.detachMediaElement();
      mpegtsPlayer.destroy();
      mpegtsPlayer = null;
    }
    if (hlsInstance) {
      hlsInstance.destroy();
      hlsInstance = null;
    }
    if (videoRef) {
      videoRef.pause();
      // NOT `videoRef.src = ''` - the browser resolves `''` against the document URL and
      // tries to load THAT, firing a real `error` event. Since <video> stays mounted
      // (CSS-hidden) even while mpv owns playback, that spurious error used to feed
      // `handleError`'s cascade/retry machinery and tear down/restart mpv in a runaway
      // loop. `removeAttribute` + `load()` clears the source silently instead.
      videoRef.removeAttribute('src');
      videoRef.load();
    }
    // Unmounts `<EmbeddedMpvPlayer>` (if mounted) - its own `onDestroy` handles the real
    // session teardown via Svelte's normal unmount lifecycle.
    mpvEngineActive = false;
  }

  $effect(() => {
    // Runs on every re-run, not just inside `if (url)` - `url` becoming falsy (stop())
    // needs the same "save then stop tracking" treatment as switching titles.
    if (trackedContext) {
      void saveCurrentProgress();
    }
    stopSaveProgressInterval();
    showNextEpisodeButton = false;
    nextEpisodeButtonDismissed = false;

    if (url) {
      hasRenderedFrame = false;
      retryCount = 0;
      lastErrorWasServerRejection = false;
      engineOverride = null;
      attemptedEngines = new Set();
      hasSeekedToResumePosition = false;
      completionHandled = false;
      if (isFormatFallbackReplay) {
        // This IS the fallback attempt in progress - leave `formatFallbackAttempted` set
        // so it can't trigger again and swap formats forever.
        isFormatFallbackReplay = false;
      } else {
        formatFallbackAttempted = false;
      }
      destroyPlayer();
      setupPlayer();

      trackedContext = playerStore.resumeContext;
      startSaveProgressInterval();
    } else {
      trackedContext = null;
    }
  });

  function handleFullscreenChange() {
    if (isFullscreen) {
      if (document.fullscreenElement !== playerRoot) {
        playerRoot?.requestFullscreen?.();
      }
    } else if (document.fullscreenElement === playerRoot) {
      document.exitFullscreen?.();
    }
  }

  // The browser can exit fullscreen on its own (Escape key, OS gesture)
  // without going through toggleFullscreen(), so the store has to be told
  // when that happens or the next toggle click would be a no-op.
  function handleNativeFullscreenChange() {
    if (!document.fullscreenElement) {
      playerStore.isFullscreen = false;
    }
  }

  $effect(() => {
    handleFullscreenChange();
  });

  let playerRoot: HTMLDivElement;

  onMount(() => {
    document.addEventListener('fullscreenchange', handleNativeFullscreenChange);
  });

  onDestroy(() => {
    document.removeEventListener('fullscreenchange', handleNativeFullscreenChange);
    // Best-effort final save before teardown (e.g. navigating away mid-playback).
    // Fire-and-forget: `onDestroy` can't `await`.
    if (trackedContext) {
      void saveCurrentProgress();
    }
    stopSaveProgressInterval();
    destroyPlayer();
  });

  function handleDblClick() {
    playerStore.toggleFullscreen();
    showFullscreenOverlay = true;
  }

  function handleKeyDown(event: KeyboardEvent) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      playerStore.toggleFullscreen();
      showFullscreenOverlay = true;
    }
  }

  let showFullscreenOverlay = $state(false);

  $effect(() => {
    if (showFullscreenOverlay) {
      const timer = setTimeout(() => {
        showFullscreenOverlay = false;
      }, 800);
      return () => clearTimeout(timer);
    }
  });

  function handleError() {
    // The hidden <video> stays mounted while mpv owns playback and never actually drives
    // it, so its `error` events are meaningless and must not feed the retry/cascade below.
    if (mpvEngineActive) return;
    // A container native decode can't handle is usually raw MPEG-TS mislabeled - worth
    // trying mpegts.js/hls.js before giving up, same cascade idea as the other engines.
    if (tryNextEngine('mpegts') || tryNextEngine('hls') || tryNextEngine('mpv')) return;
    handleFatalError('Playback error');
  }

  /** Shared "something actually started playing" logic - called by `handlePlaying` for
   * non-mpv engines, and directly by `handleMpvReady` (no DOM event to hang off of). */
  function onPlaybackStarted() {
    loading = false;
    // A frame rendered, so earlier retry-budget hiccups no longer matter - without this a
    // channel that recovers stays one blip away from permanently giving up.
    retryCount = 0;
    lastErrorWasServerRejection = false;
    // Marks startup over - the one-shot format fallback must never fire again for this URL.
    hasRenderedFrame = true;
    seekToResumePositionIfNeeded();
  }

  function handlePlaying() {
    // Hidden <video>'s events are meaningless while mpv owns playback - see `handleError`.
    if (mpvEngineActive) return;
    onPlaybackStarted();
  }

  function handleMpvReady() {
    onPlaybackStarted();
  }

  /** Immediate save on pause, not just the periodic interval. */
  function handlePause() {
    if (mpvEngineActive) return;
    if (trackedContext) void saveCurrentProgress();
  }

  /** A real `ended` event is a faster completion signal than the periodic poll; both
   * call `handleCompletion`, guarded by `completionHandled` so whichever fires first wins. */
  function handleEnded() {
    if (mpvEngineActive) return;
    if (trackedContext) void handleCompletion(trackedContext);
  }

  function handleMpvError(message: string) {
    // mpv's error is a plain message with no structured HTTP status, so there's no way to
    // tell a transient blip from a provider rejection here. Always using the slow backoff
    // (reserved elsewhere for confirmed rejections) trades a little responsiveness for
    // never repeating the connection-storm that guessing wrong on a real mpv session caused.
    lastErrorWasServerRejection = true;
    // mpv is the most capable engine - reached as the first attempt for a container
    // `<video>` can't demux, or as the last resort after everything else failed. Either
    // way nothing left would recover this, so straight to fatal-error/retry.
    handleFatalError('Playback error: ' + message);
  }
</script>

<div bind:this={playerRoot} class="relative h-full w-full rounded-xl overflow-hidden shadow-2xl bg-black" ondblclick={handleDblClick} onkeydown={handleKeyDown} role="button" tabindex="0">
  <video
    bind:this={videoRef}
    autoplay
    controls={false}
    playsinline
    class="h-full w-full object-contain"
    class:hidden={mpvEngineActive}
    onerror={handleError}
    onplaying={handlePlaying}
    onpause={handlePause}
    onended={handleEnded}
  ></video>

  {#key mpvMountKey}
    {#if mpvEngineActive && url}
      <EmbeddedMpvPlayer url={url} title={playerStore.currentTitle ?? 'Video'} onready={handleMpvReady} onerror={handleMpvError} />
    {/if}
  {/key}

  {#if loading}
    <div class="absolute inset-0 flex items-center justify-center bg-black/80">
      <Icon name="Loader2" size={48} class="animate-spin text-primary" />
    </div>
  {/if}

  {#if error}
    <div class="absolute inset-0 flex flex-col items-center justify-center gap-4 bg-black/80">
      <EmptyState
        icon="AlertCircle"
        title="Playback Error"
        description={error}
      />
      <button
        class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-white hover:opacity-90"
        onclick={manualRetry}
      >
        Retry
      </button>
    </div>
  {/if}

  {#if showFullscreenOverlay}
    <div class="absolute inset-0 flex items-center justify-center bg-black/40 animate-fade-in">
      <Icon name="Maximize" size={64} class="text-white drop-shadow-lg" />
    </div>
  {/if}

  {#if showNextEpisodeButton}
    <div class="absolute bottom-24 right-6 z-20 flex items-center gap-4 rounded-lg bg-black/85 px-5 py-4 shadow-xl animate-fade-in">
      <div class="min-w-0">
        <p class="text-xs text-text-muted">Next episode</p>
        <p class="mt-0.5 max-w-[16rem] truncate text-sm font-medium text-white">{nextEpisodeButtonTitle}</p>
      </div>
      <button
        class="flex-shrink-0 rounded-md bg-white/10 p-1.5 text-white transition-colors hover:bg-white/20"
        onclick={dismissNextEpisodeButton}
        aria-label="Dismiss"
        title="Dismiss"
      >
        <Icon name="X" size={16} />
      </button>
      <button
        class="flex-shrink-0 rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-primary/90"
        onclick={playNextEpisodeNow}
      >
        Play Now
      </button>
    </div>
  {/if}

  <!-- Nested inside playerRoot, not a caller sibling - the Fullscreen API only shows the
       requested element plus its descendants; a sibling used to vanish entirely on
       fullscreen since no z-index can substitute for being a descendant. -->
  <PlayerControls />
</div>
