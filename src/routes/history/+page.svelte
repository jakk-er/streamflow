<script lang="ts">
  import { onMount } from 'svelte';
  import { favoritesStore, playlistStore } from '$lib/stores';
  import WatchHistoryList from '$lib/components/favorites/WatchHistoryList.svelte';

  onMount(async () => {
    await favoritesStore.loadRecentlyWatched(100);
  });

  let recentlyWatched = $derived(favoritesStore.recentlyWatched);
  let playlists = $derived(playlistStore.playlists);
  let playlistMap = $derived(new Map(playlists.map(p => [p._id, p])));

  // Live TV position is never tracked (only a hardcoded 0/0 "watched at"
  // bump - see `live/+page.svelte`'s `savePosition` call), so every entry
  // here has `totalSeconds <= 0` - no "partway through" bucket to split out.

  let clearing = $state(false);

  async function handleClearAll() {
    if (clearing) return;
    clearing = true;
    try {
      await favoritesStore.clearHistory();
    } finally {
      clearing = false;
    }
  }
</script>

<div class="flex h-full w-full flex-col overflow-hidden bg-gray-900 text-white">
  <div class="flex-shrink-0 flex items-center justify-between border-b border-gray-700 p-4">
    <h1 class="text-2xl font-bold">Recently Watched</h1>
    {#if recentlyWatched.length > 0}
      <button
        class="rounded-lg bg-gray-800 px-3 py-1.5 text-sm text-gray-300 hover:bg-gray-700 hover:text-white transition-colors disabled:opacity-50"
        onclick={handleClearAll}
        disabled={clearing}
      >
        Clear All
      </button>
    {/if}
  </div>

  <div class="flex-1 overflow-y-auto">
    {#if recentlyWatched.length === 0}
      <div class="flex flex-col items-center justify-center py-16">
        <svg xmlns="http://www.w3.org/2000/svg" class="h-16 w-16 text-gray-600 mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <p class="text-lg text-gray-400">No watch history yet</p>
        <p class="mt-2 text-sm text-gray-500">Start watching to build your history</p>
        <a href="/live" class="mt-4 rounded bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-500">
          Browse Channels
        </a>
      </div>
    {:else}
      <div class="p-4">
        <h2 class="text-lg font-semibold mb-4">Watch History</h2>
        <WatchHistoryList items={recentlyWatched} />
      </div>
    {/if}
  </div>
</div>
