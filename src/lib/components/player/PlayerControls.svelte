<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { playerStore, settingsStore } from '$lib/stores';
  import { mpvPlayPause, mpvSeek, mpvSetVolume, mpvSetBrightness, mpvGetSessionState } from '$lib/api';
  import Icon from '$lib/components/ui/Icon.svelte';
  import Button from '$lib/components/ui/Button.svelte';

  // Driven by the shared store, not a local timer - an embedded mpv session's forwarded
  // native pointer events need to feed the same show/hide logic as this component's own
  // `onmousemove`, and two independent timers would fight each other.
  let visible = $derived(playerStore.controlsVisible);
  // True when an embedded mpv session (a real native window, not a DOM element) drives
  // playback - controls then route through `mpv_*`/`pollMpvState()` instead of `getVideo()`.
  let usingMpv = $derived(!!playerStore.mpvSessionId);

  let isPaused = $state(false);
  let isMuted = $state(false);
  let volume = $state(1);
  let currentTime = $state(0);
  let duration = $state(0);
  let brightness = $state(100);
  let showVolumeSlider = $state(false);
  let showBrightnessSlider = $state(false);
  /** True while the user is dragging the seek handle - `timeupdate` must not
   * fight the drag by snapping the displayed position back to the video's
   * real (not-yet-seeked) position on every frame. */
  let seekingSeek = $state(false);

  function getVideo(): HTMLVideoElement | null {
    return document.querySelector('video');
  }

  function handleMouseMove() {
    playerStore.showControlsTemporarily();
  }

  function togglePlay() {
    if (usingMpv) {
      const id = playerStore.mpvSessionId;
      if (!id) return;
      mpvPlayPause(id, !isPaused).catch((err) => console.error('[PlayerControls] mpv_play_pause failed:', err));
      return;
    }
    const video = getVideo();
    if (!video) return;
    if (video.paused) {
      video.play();
    } else {
      video.pause();
    }
  }

  // mpv has no separate "muted" flag in this app's command surface - mute is
  // simulated by remembering the volume just before muting and restoring it
  // on unmute, same end result as `<video>.muted` from the user's POV.
  let mpvVolumeBeforeMute = 100;

  function toggleMute() {
    if (usingMpv) {
      const id = playerStore.mpvSessionId;
      if (!id) return;
      if (isMuted) {
        mpvSetVolume(id, mpvVolumeBeforeMute).catch((err) => console.error('[PlayerControls] mpv_set_volume failed:', err));
        isMuted = false;
      } else {
        mpvVolumeBeforeMute = volume * 100;
        mpvSetVolume(id, 0).catch((err) => console.error('[PlayerControls] mpv_set_volume failed:', err));
        isMuted = true;
      }
      return;
    }
    const video = getVideo();
    if (!video) return;
    video.muted = !video.muted;
    isMuted = video.muted;
  }

  function handleVolumeInput(e: Event) {
    const value = Number((e.target as HTMLInputElement).value);
    volume = value;
    if (usingMpv) {
      const id = playerStore.mpvSessionId;
      if (!id) return;
      mpvSetVolume(id, value * 100).catch((err) => console.error('[PlayerControls] mpv_set_volume failed:', err));
      if (value > 0) isMuted = false;
      return;
    }
    const video = getVideo();
    if (!video) return;
    video.volume = value;
    if (value > 0 && video.muted) {
      video.muted = false;
      isMuted = false;
    }
  }

  function handleSeekInput(e: Event) {
    // Only updates displayed time while dragging; actual seek happens on release
    // (`handleSeekCommit`) - some IPTV VOD sources fire a byte-range request per frame
    // of movement if `currentTime` is written continuously.
    seekingSeek = true;
    currentTime = Number((e.target as HTMLInputElement).value);
  }

  function handleSeekCommit(e: Event) {
    const value = Number((e.target as HTMLInputElement).value);
    if (usingMpv) {
      const id = playerStore.mpvSessionId;
      if (id && Number.isFinite(duration)) {
        mpvSeek(id, value).catch((err) => console.error('[PlayerControls] mpv_seek failed:', err));
      }
      seekingSeek = false;
      return;
    }
    const video = getVideo();
    if (video && Number.isFinite(video.duration)) {
      video.currentTime = value;
    }
    seekingSeek = false;
  }

  function skip(deltaSeconds: number) {
    if (usingMpv) {
      const id = playerStore.mpvSessionId;
      if (!id || !Number.isFinite(duration)) return;
      mpvSeek(id, Math.max(0, Math.min(duration, currentTime + deltaSeconds))).catch((err) => console.error('[PlayerControls] mpv_seek failed:', err));
      return;
    }
    const video = getVideo();
    if (!video || !Number.isFinite(video.duration)) return;
    video.currentTime = Math.max(0, Math.min(video.duration, video.currentTime + deltaSeconds));
  }

  /** "Start Over" is a plain seek to 0, not a URL re-resolve - instant, keeps whatever's
   * buffered. For a series this only resets the current episode. */
  function restartFromBeginning() {
    if (usingMpv) {
      const id = playerStore.mpvSessionId;
      if (!id) return;
      mpvSeek(id, 0).catch((err) => console.error('[PlayerControls] mpv_seek failed:', err));
      return;
    }
    const video = getVideo();
    if (!video) return;
    video.currentTime = 0;
  }

  function handleBrightnessInput(e: Event) {
    const value = Number((e.target as HTMLInputElement).value);
    brightness = value;
    if (usingMpv) {
      const id = playerStore.mpvSessionId;
      if (!id) return;
      // CSS filter can't reach mpv's native window - map 50..150% to its -100..100 scale.
      mpvSetBrightness(id, value - 100).catch((err) => console.error('[PlayerControls] mpv_set_brightness failed:', err));
      return;
    }
    const video = getVideo();
    if (video) {
      video.style.filter = value === 100 ? '' : `brightness(${value}%)`;
    }
  }

  function formatTime(seconds: number): string {
    if (!Number.isFinite(seconds) || seconds < 0) return '0:00';
    const total = Math.floor(seconds);
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    const ss = String(s).padStart(2, '0');
    return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
  }

  function onTimeUpdate() {
    if (seekingSeek) return;
    const video = getVideo();
    if (video) currentTime = video.currentTime;
  }
  function onDurationChange() {
    const video = getVideo();
    if (video) duration = video.duration;
  }
  function onVolumeChange() {
    const video = getVideo();
    if (!video) return;
    volume = video.volume;
    isMuted = video.muted;
  }
  function onPlayStateChange() {
    const video = getVideo();
    if (video) isPaused = video.paused;
  }

  let attachedVideo: HTMLVideoElement | null = null;
  function syncFromVideo(video: HTMLVideoElement) {
    volume = video.volume;
    isMuted = video.muted;
    isPaused = video.paused;
    duration = video.duration;
    currentTime = video.currentTime;
  }

  function attachListeners() {
    // While mpv is active, the still-mounted but hidden <video> carries no real state -
    // attaching would overwrite pollMpvState()'s values with stale idle defaults.
    if (usingMpv) {
      detachListeners();
      return;
    }
    const video = getVideo();
    if (!video || video === attachedVideo) return;
    detachListeners();
    attachedVideo = video;
    video.addEventListener('timeupdate', onTimeUpdate);
    video.addEventListener('durationchange', onDurationChange);
    video.addEventListener('loadedmetadata', onDurationChange);
    video.addEventListener('volumechange', onVolumeChange);
    video.addEventListener('play', onPlayStateChange);
    video.addEventListener('pause', onPlayStateChange);
    // The <video> element persists across plays, so it may carry a stale brightness
    // filter from a previous session - reapply the slider's current value.
    video.style.filter = brightness === 100 ? '' : `brightness(${brightness}%)`;
    syncFromVideo(video);
  }

  function detachListeners() {
    if (!attachedVideo) return;
    attachedVideo.removeEventListener('timeupdate', onTimeUpdate);
    attachedVideo.removeEventListener('durationchange', onDurationChange);
    attachedVideo.removeEventListener('loadedmetadata', onDurationChange);
    attachedVideo.removeEventListener('volumechange', onVolumeChange);
    attachedVideo.removeEventListener('play', onPlayStateChange);
    attachedVideo.removeEventListener('pause', onPlayStateChange);
    attachedVideo = null;
  }

  // mpv has no DOM element to attach listeners to, so polling stands in for
  // onTimeUpdate/onDurationChange/onVolumeChange/onPlayStateChange.
  async function pollMpvState() {
    const id = playerStore.mpvSessionId;
    if (!id) return;
    try {
      const s = await mpvGetSessionState(id);
      if (!seekingSeek) currentTime = s.positionSeconds;
      duration = s.durationSeconds ?? NaN;
      if (!isMuted) volume = s.volume / 100;
      isPaused = s.status === 'paused';
    } catch (err) {
      // Session already gone (torn down elsewhere, e.g. a fast engine-
      // cascade retry) - nothing to sync against.
    }
  }

  // The <video> element (rendered by sibling `VideoPlayer.svelte`) isn't guaranteed to
  // exist when this mounts - poll picks it up when it appears and re-attaches if swapped.
  // Same interval also drives `pollMpvState()`.
  let pollInterval: number | null = null;
  onMount(() => {
    attachListeners();
    pollInterval = window.setInterval(() => {
      attachListeners();
      pollMpvState();
    }, 300);
  });

  onDestroy(() => {
    if (pollInterval !== null) clearInterval(pollInterval);
    detachListeners();
  });

  async function toggleExternal() {
    const url = playerStore.currentUrl;
    const title = playerStore.currentTitle;
    if (!url) return;
    // No pre-emptive `stop()` here - it used to clear `currentUrl` immediately, hiding
    // the inline overlay before the spawn even attempted, so a failed spawn killed
    // inline playback with nothing on screen. `play()`'s external branch now only
    // touches inline state once the spawn succeeds; failure leaves inline video running
    // and surfaces the error via `playerStore.lastError`.
    // Respects the user's configured default player instead of always launching mpv
    // (previously a bug: choosing VLC in Settings had no effect). Any other value
    // (html5/embedded-mpv/unset) falls back to mpv, the only other valid target.
    const preferred = settingsStore.settings?.videoPlayer === 'vlc' ? 'vlc' : 'mpv';
    try {
      await playerStore.play(url, title ?? undefined, preferred);
    } catch {
      // Already recorded as `playerStore.lastError` and logged inside
      // `play()` itself - nothing further to do here.
    }
  }

  function toggleFullscreen() {
    playerStore.toggleFullscreen();
  }

  // Live TV has no seekable timeline (mpegts.js/hls.js report `Infinity` or a shifting
  // duration) - the seek bar only shows for VOD/catch-up with a real finite duration.
  let showSeekBar = $derived(playerStore.sourceKind === 'vod' && Number.isFinite(duration) && duration > 0);
</script>

<div
  class="absolute inset-0 z-10"
  onmousemove={handleMouseMove}
  onmouseleave={() => playerStore.hideControlsNow()}
  role="application"
>
  {#if visible}
    <div class="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/90 via-black/40 to-transparent p-4 transition-opacity duration-300">
      {#if showSeekBar}
        <div class="mb-2 flex items-center gap-3">
          <span class="text-xs tabular-nums text-text-secondary w-12 text-right">{formatTime(currentTime)}</span>
          <input
            type="range"
            min="0"
            max={duration}
            step="1"
            value={currentTime}
            oninput={handleSeekInput}
            onchange={handleSeekCommit}
            class="flex-1 h-1.5 accent-primary cursor-pointer"
            aria-label="Seek"
          />
          <span class="text-xs tabular-nums text-text-secondary w-12">{formatTime(duration)}</span>
        </div>
      {/if}

      <div class="flex items-center gap-2">
        <Button variant="ghost" size="sm" onclick={togglePlay} aria-label={isPaused ? 'Play' : 'Pause'}>
          <Icon name={isPaused ? 'Play' : 'Pause'} size={20} />
        </Button>

        {#if showSeekBar}
          <Button variant="ghost" size="sm" onclick={() => skip(-10)} aria-label="Rewind 10 seconds">
            <Icon name="RotateCcw" size={18} />
          </Button>
          <Button variant="ghost" size="sm" onclick={() => skip(10)} aria-label="Forward 10 seconds">
            <Icon name="RotateCw" size={18} />
          </Button>
          <Button variant="ghost" size="sm" onclick={restartFromBeginning} aria-label="Start over">
            <Icon name="SkipBack" size={18} />
          </Button>
        {/if}

        <div
          class="relative flex items-center"
          onmouseenter={() => (showVolumeSlider = true)}
          onmouseleave={() => (showVolumeSlider = false)}
          role="group"
          aria-label="Volume"
        >
          <Button variant="ghost" size="sm" onclick={toggleMute} aria-label={isMuted ? 'Unmute' : 'Mute'}>
            <Icon name={isMuted ? 'VolumeX' : 'Volume2'} size={20} />
          </Button>
          {#if showVolumeSlider}
            <input
              type="range"
              min="0"
              max="1"
              step="0.05"
              value={isMuted ? 0 : volume}
              oninput={handleVolumeInput}
              class="w-20 h-1.5 accent-primary cursor-pointer ml-1"
              aria-label="Volume level"
            />
          {/if}
        </div>

        <div
          class="relative flex items-center"
          onmouseenter={() => (showBrightnessSlider = true)}
          onmouseleave={() => (showBrightnessSlider = false)}
          role="group"
          aria-label="Brightness"
        >
          <Button variant="ghost" size="sm" aria-label="Brightness">
            <Icon name="Sun" size={20} />
          </Button>
          {#if showBrightnessSlider}
            <input
              type="range"
              min="50"
              max="150"
              step="5"
              value={brightness}
              oninput={handleBrightnessInput}
              class="w-20 h-1.5 accent-primary cursor-pointer ml-1"
              aria-label="Brightness level"
            />
          {/if}
        </div>

        <span class="text-sm text-text-primary truncate max-w-[200px]">
          {playerStore.currentTitle ?? 'Live TV'}
        </span>

        <div class="flex-1"></div>

        <Button variant="ghost" size="sm" onclick={toggleExternal} aria-label="Open in external player">
          <Icon name="ExternalLink" size={16} />
          <span class="text-xs">External</span>
        </Button>

        <Button variant="ghost" size="sm" onclick={toggleFullscreen} aria-label="Fullscreen">
          <Icon name="Maximize" size={20} />
        </Button>
      </div>
    </div>
  {/if}
</div>
