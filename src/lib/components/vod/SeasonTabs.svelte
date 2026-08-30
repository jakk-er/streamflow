<script lang="ts">
  import { playerStore, playlistStore } from '$lib/stores';
  import type { SeriesDetails, SeasonEpisode, StalkerContentType, VodResumeContext } from '$lib/types';
  import { vodGetProgress } from '$lib/api';
  import { resolveEpisodeUrl, findUpcomingEpisodes, toResumeEpisodeQueue } from '$lib/utils/vodEpisodePlayback';

  let { detail }: { detail: SeriesDetails } = $props();

  let selectedSeason = $state<number>(1);

  $effect(() => {
    if (detail.seasons.length > 0 && !detail.seasons.find(s => s.seasonNumber === selectedSeason)) {
      selectedSeason = detail.seasons[0].seasonNumber;
    }
  });

  let currentSeasonEpisodes = $derived(() => {
    const key = String(selectedSeason);
    return detail.episodes[key] || [];
  });

  // Fetched once per series, not per-episode - a series has exactly one
  // progress row, pointing at whichever episode is current. `$effect`, not
  // `onMount`: the parent route reuses this component instance across
  // different-title navigations, so `detail.info.id` changing is the real
  // "load progress for the now-current series" signal.
  let progress = $state<Awaited<ReturnType<typeof vodGetProgress>>>(null);
  $effect(() => {
    const seriesId = detail.info.id;
    const playlist = playlistStore.activePlaylist;
    if (!playlist) {
      progress = null;
      return;
    }
    vodGetProgress(playlist._id, 'series', seriesId)
      .then((result) => {
        progress = result;
      })
      .catch((err) => {
        console.error('Failed to load series watch progress:', err);
        progress = null;
      });
  });

  // Mirrors `VodDetail.svelte`'s `handlePlay`: an episode's `directSource`
  // was resolved once when the detail loaded and can be dead by click time,
  // so Stalker re-resolves fresh from `episode.cmd`/`seriesParam`. Every URL
  // routes through the stream proxy regardless of provider. Shared logic
  // lives in `vodEpisodePlayback.ts` (also used by the player's auto-advance).
  async function playEpisode(episode: SeasonEpisode) {
    const playlist = playlistStore.activePlaylist;
    if (!playlist) return;

    try {
      const stalkerContentType = (detail.info.stalkerContentType ?? 'series') as StalkerContentType;
      const resolved = await resolveEpisodeUrl(playlist._id, playlist.playlistType === 'stalker', episode, stalkerContentType);
      if (!resolved) return;

      const isResumingThisEpisode = progress?.episodeId === episode.id && progress.positionSeconds > 0;
      const resumeContext: VodResumeContext = {
        playlistId: playlist._id,
        contentType: 'series',
        vodItemId: detail.info.id,
        title: detail.info.name,
        cover: detail.info.cover,
        episode: { id: episode.id, seasonNumber: episode.season, episodeNumber: episode.episodeNum, title: episode.title },
        startPositionSeconds: isResumingThisEpisode ? progress!.positionSeconds : 0,
        upcomingEpisodes: toResumeEpisodeQueue(findUpcomingEpisodes(detail, episode.season, episode.id)),
      };
      await playerStore.play(resolved.url, episode.title, undefined, resolved.extension, 'vod', resumeContext);
    } catch (err) {
      console.error('Failed to resolve episode playback URL:', err);
    }
  }

  function getProgress(episode: SeasonEpisode): number {
    if (progress?.episodeId !== episode.id || progress.totalSeconds <= 0) return 0;
    return Math.round((progress.positionSeconds / progress.totalSeconds) * 100);
  }
</script>

{#if detail.seasons.length <= 1}
  {#if currentSeasonEpisodes().length > 0}
    <div class="space-y-2">
      {#each currentSeasonEpisodes() as episode (episode.id)}
        <button
          class="flex w-full items-center gap-4 rounded-lg bg-gray-800 p-3 text-left hover:bg-gray-700 transition-colors"
          onclick={() => playEpisode(episode)}
        >
          <div class="flex-shrink-0 w-16 h-10 rounded bg-gray-700 overflow-hidden">
            {#if episode.cover}
              <img src={episode.cover} alt="" class="h-full w-full object-cover" />
            {:else}
              <div class="flex h-full w-full items-center justify-center text-xs text-gray-500">
                E{episode.episodeNum ?? ''}
              </div>
            {/if}
          </div>

          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2">
              <span class="text-sm text-white truncate">{episode.title}</span>
              {#if episode.episodeNum}
                <span class="text-xs text-gray-500">E{episode.episodeNum}</span>
              {/if}
            </div>
            {#if episode.plot}
              <p class="mt-1 truncate text-xs text-gray-400">{episode.plot}</p>
            {/if}
          </div>

          {#if getProgress(episode) > 0}
            <div class="flex-shrink-0 w-16">
              <div class="h-1 rounded-full bg-gray-700 overflow-hidden">
                <div class="h-full bg-blue-500" style="width: {getProgress(episode)}%"></div>
              </div>
            </div>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
{:else}
  <div class="flex gap-2 overflow-x-auto border-b border-gray-700">
    {#each detail.seasons as season}
      <button
        class={`flex-shrink-0 px-4 py-2 text-sm font-medium transition-colors ${
          selectedSeason === season.seasonNumber
            ? 'border-b-2 border-blue-500 text-blue-400'
            : 'text-gray-400 hover:text-white'
        }`}
        onclick={() => (selectedSeason = season.seasonNumber)}
      >
        {season.name || `Season ${season.seasonNumber}`}
      </button>
    {/each}
  </div>

  {#if currentSeasonEpisodes().length > 0}
    <div class="mt-4 space-y-2">
      {#each currentSeasonEpisodes() as episode (episode.id)}
        <button
          class="flex w-full items-center gap-4 rounded-lg bg-gray-800 p-3 text-left hover:bg-gray-700 transition-colors"
          onclick={() => playEpisode(episode)}
        >
          <div class="flex-shrink-0 w-16 h-10 rounded bg-gray-700 overflow-hidden">
            {#if episode.cover}
              <img src={episode.cover} alt="" class="h-full w-full object-cover" />
            {:else}
              <div class="flex h-full w-full items-center justify-center text-xs text-gray-500">
                E{episode.episodeNum ?? ''}
              </div>
            {/if}
          </div>

          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2">
              <span class="text-sm text-white truncate">{episode.title}</span>
              {#if episode.episodeNum}
                <span class="text-xs text-gray-500">E{episode.episodeNum}</span>
              {/if}
            </div>
            {#if episode.plot}
              <p class="mt-1 truncate text-xs text-gray-400">{episode.plot}</p>
            {/if}
          </div>

          {#if getProgress(episode) > 0}
            <div class="flex-shrink-0 w-16">
              <div class="h-1 rounded-full bg-gray-700 overflow-hidden">
                <div class="h-full bg-blue-500" style="width: {getProgress(episode)}%"></div>
              </div>
            </div>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
{/if}
