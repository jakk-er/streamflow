<script lang="ts">
  import { goto } from '$app/navigation';
  import { vodGetTopRated } from '$lib/api';
  import { playlistStore } from '$lib/stores';
  import type { VodCatalogItem } from '$lib/types';

  let movies = $state<VodCatalogItem[]>([]);
  let loading = $state(false);

  // Rating comes from the local `vod_items` cache, not TMDB (see `vod_get_top_rated`).
  // Re-runs on playlist change so switching playlists clears stale titles.
  $effect(() => {
    const playlistId = playlistStore.activePlaylistId;
    if (!playlistId) {
      movies = [];
      return;
    }
    loading = true;
    vodGetTopRated(playlistId, 6)
      .then((items) => {
        movies = items;
      })
      .catch((err) => {
        console.error('Failed to load trending movies:', err);
        movies = [];
      })
      .finally(() => {
        loading = false;
      });
  });

  function handleClick(item: VodCatalogItem) {
    goto(`/vod/${item.id}?type=movie`);
  }
</script>

{#if loading}
  <div class="space-y-3">
    <h2 class="text-lg font-semibold text-white">Trending</h2>
    <div class="flex gap-4 overflow-x-auto pb-2 snap-x snap-mandatory" style="scrollbar-width: none;">
      {#each Array(6) as _}
        <div class="flex-shrink-0 w-[150px] snap-start">
          <div class="animate-pulse">
            <div class="aspect-[2/3] rounded-lg bg-gray-800 mb-2"></div>
            <div class="h-3 rounded bg-gray-800 mb-1"></div>
            <div class="h-2 w-2/3 rounded bg-gray-800"></div>
          </div>
        </div>
      {/each}
    </div>
  </div>
{:else if movies.length > 0}
  <div class="space-y-3">
    <h2 class="text-lg font-semibold text-white">Trending</h2>

    <div class="flex gap-4 overflow-x-auto pb-2 snap-x snap-mandatory" style="scrollbar-width: none;">
      {#each movies as item (item.id)}
        <button class="flex-shrink-0 w-[150px] snap-start text-left group" onclick={() => handleClick(item)}>
          <div class="relative aspect-[2/3] overflow-hidden rounded-lg bg-gray-800">
            {#if item.cover}
              <img src={item.cover} alt="" class="h-full w-full object-cover" />
            {:else}
              <div class="flex h-full w-full items-center justify-center bg-gradient-to-br from-gray-700 to-gray-900">
                <span class="text-3xl font-bold text-gray-500">{item.name.charAt(0).toUpperCase()}</span>
              </div>
            {/if}

            <div class="absolute inset-0 bg-black/0 transition-colors duration-300 group-hover:bg-black/20"></div>

            {#if item.rating}
              <span class="absolute top-2 left-2 rounded bg-black/70 px-1.5 py-0.5 text-xs font-medium text-yellow-400">
                ★ {item.rating}
              </span>
            {/if}
          </div>

          <p class="mt-2 truncate text-sm text-white group-hover:text-blue-400 transition-colors">
            {item.name}
          </p>
        </button>
      {/each}
    </div>
  </div>
{/if}
