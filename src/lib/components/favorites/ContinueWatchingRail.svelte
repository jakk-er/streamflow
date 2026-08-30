<script lang="ts">
  import { goto } from '$app/navigation';
  import type { VodWatchProgress } from '$lib/types';

  let { items = [] }: { items?: VodWatchProgress[] } = $props();

  function getProgress(item: VodWatchProgress) {
    if (!item.totalSeconds || item.totalSeconds <= 0) return 0;
    return Math.round((item.positionSeconds / item.totalSeconds) * 100);
  }

  function formatTime(seconds: number) {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  }

  function displayTitle(item: VodWatchProgress): string {
    return item.contentType === 'series' && item.episodeTitle ? `${item.title} - ${item.episodeTitle}` : item.title;
  }

  // Just navigation - `VodDetail.svelte` owns actual resume logic (episode/position).
  async function handlePlay(item: VodWatchProgress) {
    await goto(`/vod/${item.vodItemId}?type=${item.contentType}`);
  }
</script>

<div class="space-y-3">
  <div class="flex items-center justify-between">
    <h2 class="text-lg font-semibold text-white">Continue Watching</h2>
  </div>

  <div class="flex gap-4 overflow-x-auto pb-2 snap-x snap-mandatory" style="scrollbar-width: none;">
    {#each items.slice(0, 10) as item (item.id)}
      <button
        class="flex-shrink-0 w-[280px] snap-start text-left group"
        onclick={() => handlePlay(item)}
      >
        <div class="relative aspect-video rounded-lg bg-gray-800 overflow-hidden">
          {#if item.cover}
            <img src={item.cover} alt="" class="h-full w-full object-cover" />
          {:else}
            <div class="flex h-full w-full items-center justify-center bg-gradient-to-br from-gray-700 to-gray-900">
              <svg xmlns="http://www.w3.org/2000/svg" class="h-12 w-12 text-gray-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                <path stroke-linecap="round" stroke-linejoin="round" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                <path stroke-linecap="round" stroke-linejoin="round" d="M15.91 11.672a.375.375 0 010 .656l-5.603 3.113a.375.375 0 01-.557-.328V8.887c0-.286.307-.466.557-.327l5.603 3.112z" />
              </svg>
            </div>
          {/if}

          <div class="absolute inset-0 bg-gradient-to-t from-black/80 via-transparent to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-300"></div>

          <div class="absolute bottom-0 left-0 right-0 p-3 opacity-0 group-hover:opacity-100 transition-opacity duration-300">
            <p class="text-sm text-white font-medium truncate">
              {displayTitle(item)}
            </p>
          </div>

          <div class="absolute bottom-0 left-0 right-0 h-1 bg-gray-700">
            <div class="h-full bg-green-500" style="width: {getProgress(item)}%"></div>
          </div>
        </div>

        <div class="mt-2">
          <p class="text-sm text-white truncate group-hover:text-blue-400 transition-colors">
            {displayTitle(item)}
          </p>
          <p class="text-xs text-gray-400 mt-1">
            {formatTime(item.totalSeconds - item.positionSeconds)} left
          </p>
        </div>
      </button>
    {/each}
  </div>
</div>
