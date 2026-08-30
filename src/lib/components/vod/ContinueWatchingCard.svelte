<script lang="ts">
  import { goto } from '$app/navigation';
  import type { VodWatchProgress } from '$lib/types';

  let { item }: { item: VodWatchProgress } = $props();

  let displaySubtitle = $derived(
    item.contentType === 'series' && item.episodeTitle
      ? `S${item.seasonNumber ?? '?'}:E${item.episodeNumber ?? '?'} - ${item.episodeTitle}`
      : null
  );

  let progressPercent = $derived(item.totalSeconds > 0 ? Math.min(100, Math.max(0, (item.positionSeconds / item.totalSeconds) * 100)) : 0);

  function minutesLeft(): number {
    const remaining = Math.max(0, item.totalSeconds - item.positionSeconds);
    return Math.max(1, Math.round(remaining / 60));
  }

  // Both buttons navigate to the detail page instead of resolving/playing
  // here directly, so the resolve/resume logic stays owned by one place.
  // `?autoplay=resume|startover` tells it to fire that logic immediately on
  // arrival - see `VodDetail.svelte`'s autoplay effect.
  function goToDetail(autoplay: 'resume' | 'startover') {
    void goto(`/vod/${item.vodItemId}?type=${item.contentType}&autoplay=${autoplay}`);
  }
</script>

<div class="w-[360px] flex-shrink-0 snap-start">
  <div class="relative aspect-video overflow-hidden rounded-lg bg-gray-800">
    {#if item.cover}
      <img src={item.cover} alt="" class="h-full w-full object-cover" />
    {:else}
      <div class="flex h-full w-full items-center justify-center bg-gradient-to-br from-gray-700 to-gray-900">
        <span class="text-4xl font-bold text-gray-500">{item.title.charAt(0).toUpperCase()}</span>
      </div>
    {/if}

    <div class="absolute inset-0 bg-gradient-to-t from-black/90 via-black/20 to-transparent"></div>

    <div class="absolute inset-x-0 bottom-0 p-3">
      <p class="truncate text-sm font-semibold text-white">{item.title}</p>
      {#if displaySubtitle}
        <p class="mt-0.5 truncate text-xs text-gray-300">{displaySubtitle}</p>
      {/if}
      <p class="mt-1 text-xs text-gray-400">{minutesLeft()} min left</p>

      <div class="mt-2 flex items-center gap-2">
        <button
          class="flex flex-1 items-center justify-center gap-1.5 rounded-md bg-blue-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-blue-500"
          onclick={() => goToDetail('resume')}
        >
          <svg xmlns="http://www.w3.org/2000/svg" class="h-3.5 w-3.5" fill="currentColor" viewBox="0 0 24 24">
            <path d="M8 5v14l11-7z" />
          </svg>
          Resume
        </button>
        <button
          class="rounded-md bg-white/10 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-white/20"
          onclick={() => goToDetail('startover')}
        >
          Start Over
        </button>
      </div>
    </div>

    <div class="absolute inset-x-0 bottom-0 h-1 bg-black/50">
      <div class="h-full bg-blue-500" style="width: {progressPercent}%"></div>
    </div>
  </div>
</div>
