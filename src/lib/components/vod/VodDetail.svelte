<script lang="ts">
  import { playerStore, favoritesStore, playlistStore } from '$lib/stores';
  import { goto } from '$app/navigation';
  import type { StalkerContentType, VodDetails, SeriesDetails, VodResumeContext, VodWatchProgress, SeasonEpisode } from '$lib/types';
  import { stalkerResolvePlayback, stalkerStreamHeaders, vodGetProgress, vodClearProgress } from '$lib/api';
  import { wrapUrlThroughStreamProxy } from '$lib/utils/streamProxy';
  import { resolveEpisodeUrl, findUpcomingEpisodes, toResumeEpisodeQueue } from '$lib/utils/vodEpisodePlayback';
  import SeasonTabs from '$lib/components/vod/SeasonTabs.svelte';
  import DownloadButton from '$lib/components/vod/DownloadButton.svelte';
  import ChevronLeft from 'lucide-svelte/icons/chevron-left';

  let {
    detail,
    type = 'movie',
    autoplayAction,
  }: { detail: VodDetails | SeriesDetails; type?: 'movie' | 'series'; autoplayAction?: 'resume' | 'startover' | null } = $props();

  let isFavorite = $state(false);

  // Derived from `detail`'s own shape, not the `type` prop - `type` flips
  // synchronously on navigation while `vodStore.currentDetail` updates later
  // (async), so there's always a frame where they disagree. Only
  // `SeriesDetails` has a top-level `info` field, so this is a safe
  // discriminator regardless of which stale combination is rendering.
  const isSeries = $derived('info' in detail);
  const meta = $derived<VodDetails>(isSeries ? (detail as SeriesDetails).info : (detail as VodDetails));

  // `$effect`, not `onMount` - the route reuses this component instance
  // across different-title navigations, so `meta.id` changing is the actual
  // "load progress for the now-current title" signal.
  let progress = $state<VodWatchProgress | null>(null);
  // Distinct from progress being null/non-null (ambiguous between "not
  // loaded yet" and "loaded, nothing found") - the autoplay effect needs to
  // know the fetch for THIS title has settled before it's safe to act.
  let progressLoaded = $state(false);
  $effect(() => {
    const vodItemId = meta.id;
    const contentType = isSeries ? 'series' : 'movie';
    const playlist = playlistStore.activePlaylist;
    progressLoaded = false;
    // Reset alongside `progressLoaded` (component instance is reused across
    // titles) or a second title's own `?autoplay=...` would be silently
    // ignored since this would already read `true` from the first title.
    autoplayHandled = false;
    if (!playlist) {
      progress = null;
      progressLoaded = true;
      return;
    }
    vodGetProgress(playlist._id, contentType, vodItemId)
      .then((result) => {
        progress = result;
      })
      .catch((err) => {
        console.error('Failed to load watch progress:', err);
        progress = null;
      })
      .finally(() => {
        progressLoaded = true;
      });
  });

  // Fires playback once, the first time `progress` settles after arriving
  // with an autoplay action queued - Continue Watching's Resume/Start Over
  // buttons navigate here with `?autoplay=...` instead of duplicating this
  // page's resolve/resume logic.
  let autoplayHandled = false;
  $effect(() => {
    if (autoplayHandled || !autoplayAction || !progressLoaded) return;
    autoplayHandled = true;
    if (autoplayAction === 'startover') {
      void handleStartOver();
    } else {
      void handlePlay();
    }
  });

  const isResuming = $derived(!!progress && progress.positionSeconds > 0);
  const minutesLeft = $derived(() => {
    if (!progress || progress.totalSeconds <= 0) return null;
    const remaining = Math.max(0, progress.totalSeconds - progress.positionSeconds);
    return Math.max(1, Math.round(remaining / 60));
  });

  /** The episode a series' Play/Resume/Start Over button targets: whichever
   * one `progress` points to, or the first episode of the first season when
   * there's no progress yet. */
  function targetSeriesEpisode(seriesDetail: SeriesDetails): SeasonEpisode | null {
    if (progress?.episodeId && progress.seasonNumber != null) {
      const seasonEpisodes = seriesDetail.episodes[String(progress.seasonNumber)] ?? [];
      const found = seasonEpisodes.find((e) => e.id === progress!.episodeId);
      if (found) return found;
    }
    if (seriesDetail.seasons.length === 0) return null;
    const firstSeason = seriesDetail.seasons[0].seasonNumber;
    return seriesDetail.episodes[String(firstSeason)]?.[0] ?? null;
  }

  function getYear(dateStr?: string) {
    if (!dateStr) return null;
    const date = new Date(dateStr);
    if (isNaN(date.getTime())) return null;
    return date.getFullYear();
  }

  // Stalker's `direct_source` on `meta` is only a preview value resolved once
  // when the detail page loaded and can be dead by click time - movies
  // re-resolve fresh here. Every resolved URL (Stalker or Xtream/M3U) routes
  // through the local stream proxy: Xtream VOD hosts hit the same CORS
  // failure live channels did without it.
  //
  // `forceStartOver` lets the "Start Over" control ignore a saved position
  // even though `progress` still has one.
  /** Resolves and plays one series episode at `startPositionSeconds` - shared
   * by `handlePlay` (position from `progress`) and `handleStartOver` (episode
   * captured before `progress` is cleared, position forced to 0). */
  async function playSeriesEpisode(episode: SeasonEpisode, startPositionSeconds: number) {
    const playlist = playlistStore.activePlaylist;
    if (!playlist) return;
    try {
      const stalkerContentType = (meta.stalkerContentType ?? 'series') as StalkerContentType;
      const resolved = await resolveEpisodeUrl(playlist._id, playlist.playlistType === 'stalker', episode, stalkerContentType);
      if (!resolved) return;

      const resumeContext: VodResumeContext = {
        playlistId: playlist._id,
        contentType: 'series',
        vodItemId: meta.id,
        title: meta.name,
        cover: meta.cover,
        episode: { id: episode.id, seasonNumber: episode.season, episodeNumber: episode.episodeNum, title: episode.title },
        startPositionSeconds,
        upcomingEpisodes: toResumeEpisodeQueue(findUpcomingEpisodes(detail as SeriesDetails, episode.season, episode.id)),
      };
      await playerStore.play(resolved.url, episode.title, undefined, resolved.extension, 'vod', resumeContext);
    } catch (err) {
      console.error('Failed to resolve VOD playback URL:', err);
    }
  }

  async function handlePlay(forceStartOver = false) {
    const playlist = playlistStore.activePlaylist;
    if (!playlist) return;

    if (isSeries) {
      const episode = targetSeriesEpisode(detail as SeriesDetails);
      if (!episode) return;
      const isThisTheProgressEpisode = !forceStartOver && progress?.episodeId === episode.id;
      await playSeriesEpisode(episode, isThisTheProgressEpisode ? progress!.positionSeconds : 0);
      return;
    }

    try {
      let resolvedUrl = meta.directSource || meta.streamUrl || '';
      if (playlist.playlistType === 'stalker' && meta.cmd) {
        const contentType = (meta.stalkerContentType ?? 'vod') as StalkerContentType;
        resolvedUrl = await stalkerResolvePlayback(
          playlist._id,
          contentType,
          meta.cmd,
          meta.useHttpTmpLink,
          meta.useLoadBalancing
        );
      }
      if (!resolvedUrl) return;

      const { url, extension } = await wrapUrlThroughStreamProxy(playlist._id, resolvedUrl, true);
      if (url) {
        const resumeContext: VodResumeContext = {
          playlistId: playlist._id,
          contentType: 'movie',
          vodItemId: meta.id,
          title: meta.name,
          cover: meta.cover,
          startPositionSeconds: forceStartOver ? 0 : (progress?.positionSeconds ?? 0),
        };
        await playerStore.play(url, meta.name, undefined, extension, 'vod', resumeContext);
      }
    } catch (err) {
      console.error('Failed to resolve VOD playback URL:', err);
    }
  }

  /** Restarts from the beginning without entering playback first. For a
   * series this only resets whichever episode `progress` points to, never
   * back to episode 1. */
  async function handleStartOver() {
    const playlist = playlistStore.activePlaylist;
    if (!playlist) return;
    // Captured before clearing `progress` below - `targetSeriesEpisode`
    // falls back to episode 1 once `progress` is gone.
    const seriesEpisode = isSeries ? targetSeriesEpisode(detail as SeriesDetails) : null;
    try {
      await vodClearProgress(playlist._id, isSeries ? 'series' : 'movie', meta.id);
    } catch (err) {
      console.error('Failed to clear watch progress:', err);
    }
    progress = null;
    if (isSeries) {
      if (seriesEpisode) await playSeriesEpisode(seriesEpisode, 0);
    } else {
      await handlePlay(true);
    }
  }

  function safeHost(url: string | undefined): string | null {
    if (!url) return null;
    try {
      return new URL(url).host.toLowerCase();
    } catch {
      return null;
    }
  }

  // Same resolution as `handlePlay`, but for the download manager: it fetches
  // server-side via plain `reqwest`, so no CORS/proxy step is needed - just a
  // fresh URL and (for Stalker) the right headers.
  //
  // Portal identity headers are attached only when the resolved URL's host
  // matches the portal's own host - a `create_link` result usually redirects
  // to a third-party CDN, and sending the portal's MAC cookie/Bearer token
  // there would leak credentials. Mirrors `stream_proxy.rs`'s
  // `resolve_auth_headers` boundary, since downloads bypass that proxy.
  async function resolveForDownload(): Promise<{ url: string; headers?: [string, string][] } | null> {
    const playlist = playlistStore.activePlaylist;
    if (!playlist) return null;

    try {
      let resolvedUrl = meta.directSource || meta.streamUrl || '';
      if (playlist.playlistType === 'stalker' && meta.cmd) {
        const contentType = (meta.stalkerContentType ?? 'vod') as StalkerContentType;
        resolvedUrl = await stalkerResolvePlayback(
          playlist._id,
          contentType,
          meta.cmd,
          meta.useHttpTmpLink,
          meta.useLoadBalancing
        );
      }
      if (!resolvedUrl) return null;

      let headers: [string, string][] | undefined;
      if (playlist.playlistType === 'stalker') {
        const portalHost = safeHost(playlist.stalkerEndpoint ?? playlist.portalUrl);
        const targetHost = safeHost(resolvedUrl);
        if (portalHost && targetHost && portalHost === targetHost) {
          headers = await stalkerStreamHeaders(playlist._id);
        }
      }
      return { url: resolvedUrl, headers };
    } catch (err) {
      console.error('Failed to resolve download URL:', err);
      return null;
    }
  }

  async function handleFavorite() {
    // Placeholder: need playlistId and channelId equivalent
    // For now, just toggle local state
    isFavorite = !isFavorite;
  }

  function handleDownload() {
    // Handled by DownloadButton component
  }
</script>

<div class="flex h-full flex-col overflow-y-auto">
  <div class="mx-auto w-full max-w-7xl px-6 pt-6">
    <button
      class="flex items-center gap-1 text-sm text-gray-400 transition-colors hover:text-white"
      onclick={() => goto('/vod')}
    >
      <ChevronLeft class="h-4 w-4" />
      Back to {type === 'series' ? 'Series' : 'Movies'}
    </button>
  </div>
  <div class="flex flex-col md:flex-row gap-6 p-6 max-w-7xl mx-auto w-full">
    <div class="flex-shrink-0">
      <div class="aspect-[2/3] w-full max-w-[300px] overflow-hidden rounded-lg bg-gray-800">
        {#if meta.cover}
          <img src={meta.cover} alt={meta.name} class="h-full w-full object-cover" />
        {:else}
          <div class="flex h-full w-full items-center justify-center bg-gradient-to-br from-gray-700 to-gray-900">
            <span class="text-6xl font-bold text-gray-500">{meta.name.charAt(0).toUpperCase()}</span>
          </div>
        {/if}
      </div>
    </div>

    <div class="flex-1 min-w-0">
      <div class="flex items-start justify-between gap-4">
        <div>
          <h1 class="text-3xl font-bold text-white">{meta.name}</h1>
          <div class="mt-2 flex flex-wrap items-center gap-3 text-sm text-gray-400">
            {#if getYear(meta.releaseDate)}
              <span>{getYear(meta.releaseDate)}</span>
            {/if}
            {#if meta.rating}
              <span class="flex items-center gap-1 text-yellow-400">
                <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="currentColor" viewBox="0 0 24 24">
                  <path d="M11.48 3.499a.562.562 0 011.04 0l2.125 5.111a.563.563 0 00.475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 00-.182.557l1.285 5.385a.562.562 0 01-.84.61l-4.725-2.885a.563.563 0 00-.586 0L6.982 20.54a.562.562 0 01-.84-.61l1.285-5.386a.562.562 0 00-.182-.557l-4.204-3.602a.563.563 0 01.321-.988l5.518-.442a.563.563 0 00.475-.345L11.48 3.5z" />
                </svg>
                {meta.rating}
              </span>
            {/if}
            {#if meta.genre}
              <span class="rounded bg-gray-700 px-2 py-0.5 text-xs">{meta.genre}</span>
            {/if}
            {#if meta.tmdbId}
              <span class="rounded bg-blue-900/50 px-2 py-0.5 text-xs text-blue-300">TMDB</span>
            {/if}
          </div>
        </div>

        <div class="flex items-center gap-2">
          <button
            class={`rounded-full p-2 ${isFavorite ? 'text-yellow-400' : 'text-gray-400 hover:text-white'}`}
            onclick={handleFavorite}
            aria-label="Toggle favorite"
          >
            <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6" fill={isFavorite ? 'currentColor' : 'none'} viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M11.48 3.499a.562.562 0 011.04 0l2.125 5.111a.563.563 0 00.475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 00-.182.557l1.285 5.385a.562.562 0 01-.84.61l-4.725-2.885a.563.563 0 00-.586 0L6.982 20.54a.562.562 0 01-.84-.61l1.285-5.386a.562.562 0 00-.182-.557l-4.204-3.602a.563.563 0 01.321-.988l5.518-.442a.563.563 0 00.475-.345L11.48 3.5z" />
            </svg>
          </button>
        </div>
      </div>

      {#if meta.plot}
        <div class="mt-4">
          <h2 class="text-lg font-semibold text-white">Plot</h2>
          <p class="mt-2 text-gray-300 leading-relaxed">{meta.plot}</p>
        </div>
      {/if}

      {#if meta.cast}
        <div class="mt-4">
          <h2 class="text-lg font-semibold text-white">Cast</h2>
          <p class="mt-2 text-sm text-gray-400">{meta.cast}</p>
        </div>
      {/if}

      <div class="mt-6 flex flex-wrap items-center gap-3">
        <button
          class="flex items-center gap-2 rounded-lg bg-blue-600 px-6 py-3 font-medium text-white hover:bg-blue-500 transition-colors"
          onclick={() => handlePlay()}
        >
          <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" fill="currentColor" viewBox="0 0 24 24">
            <path d="M8 5v14l11-7z" />
          </svg>
          {isResuming ? 'Resume' : 'Play'}
        </button>

        {#if isResuming}
          <button
            class="rounded-lg px-3 py-2 text-sm text-gray-400 transition-colors hover:text-white"
            onclick={handleStartOver}
          >
            Start Over
          </button>
        {/if}

        {#if isResuming && minutesLeft() !== null}
          <span class="text-sm text-gray-400">{minutesLeft()} min left</span>
        {/if}

        <DownloadButton
          url={meta.directSource || meta.streamUrl || ''}
          title={meta.name}
          resolveUrl={resolveForDownload}
        />
      </div>

      {#if isSeries}
        <div class="mt-8">
          <SeasonTabs detail={detail as SeriesDetails} />
        </div>
      {/if}
    </div>
  </div>
</div>
