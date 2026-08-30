<script lang="ts">
  import { goto } from '$app/navigation';
  import type { VodWatchProgress } from '$lib/types';

  let { item = null }: { item?: VodWatchProgress | null } = $props();

  async function handlePlay() {
    if (!item) return;
    // Just navigation - `VodDetail.svelte` owns actual resume logic (episode/position).
    await goto(`/vod/${item.vodItemId}?type=${item.contentType}`);
  }

  let displayTitle = $derived(item?.contentType === 'series' && item.episodeTitle ? `${item.title} - ${item.episodeTitle}` : (item?.title ?? ''));
</script>

{#if item}
  <div class="relative w-full h-[300px] rounded-2xl overflow-hidden bg-gray-800">
    <div class="absolute inset-0 bg-gradient-to-br from-gray-700 to-gray-900">
      {#if item.cover}
        <img src={item.cover} alt="" class="h-full w-full object-cover" />
      {:else}
        <div class="flex h-full w-full items-center justify-center">
          <svg xmlns="http://www.w3.org/2000/svg" class="h-24 w-24 text-gray-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
            <path stroke-linecap="round" stroke-linejoin="round" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            <path stroke-linecap="round" stroke-linejoin="round" d="M15.91 11.672a.375.375 0 010 .656l-5.603 3.113a.375.375 0 01-.557-.328V8.887c0-.286.307-.466.557-.327l5.603 3.112z" />
          </svg>
        </div>
      {/if}
    </div>

    <div class="absolute inset-0 bg-gradient-to-t from-black/90 via-black/40 to-transparent"></div>

    <div class="absolute bottom-0 left-0 right-0 p-6">
      <div class="flex items-end justify-between gap-4">
        <div class="min-w-0">
          <h1 class="text-2xl font-bold text-white truncate">
            {displayTitle}
          </h1>
          <p class="text-sm text-gray-300 mt-1">Continue watching</p>
        </div>

        <div class="flex items-center gap-3 flex-shrink-0">
          <button
            class="rounded-xl bg-gray-800/80 backdrop-blur-sm px-4 py-2 text-sm font-medium text-white hover:bg-gray-700 transition-colors border border-gray-700"
            onclick={handlePlay}
          >
            More Info
          </button>
          <button
            class="rounded-xl bg-blue-600 px-6 py-2 text-sm font-semibold text-white hover:bg-blue-500 transition-colors shadow-lg shadow-blue-600/20 flex items-center gap-2"
            onclick={handlePlay}
          >
            <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="currentColor" viewBox="0 0 24 24">
              <path d="M8 5v14l11-7z" />
            </svg>
            Resume
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
