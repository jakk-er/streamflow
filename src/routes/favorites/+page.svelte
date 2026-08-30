<script lang="ts">
  import { onMount } from 'svelte';
  import { favoritesStore, playlistStore } from '$lib/stores';
  import FavoriteList from '$lib/components/favorites/FavoriteList.svelte';

  onMount(async () => {
    await favoritesStore.loadFavorites();
  });

  let favorites = $derived(favoritesStore.favorites);
  let loading = $derived(favoritesStore.loading);
  let playlistMap = $derived(new Map(playlistStore.playlists.map(p => [p._id, p])));
</script>

<div class="flex h-full w-full flex-col overflow-hidden bg-gray-900 text-white">
  <div class="flex-shrink-0 border-b border-gray-700 p-4">
    <div class="flex items-center gap-3">
      <h1 class="text-2xl font-bold">Global Favorites</h1>
      <span class="rounded-full bg-blue-600 px-3 py-0.5 text-sm font-medium text-white">
        {favorites.length}
      </span>
    </div>
  </div>

  <div class="flex-1 overflow-y-auto p-4">
    {#if loading}
      <div class="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
        {#each Array(6) as _}
          <div class="animate-pulse">
            <div class="aspect-[2/3] rounded-lg bg-gray-800"></div>
            <div class="mt-2 h-4 rounded bg-gray-800"></div>
            <div class="mt-1 h-3 w-2/3 rounded bg-gray-800"></div>
          </div>
        {/each}
      </div>
    {:else if favorites.length === 0}
      <div class="flex flex-col items-center justify-center py-16">
        <svg xmlns="http://www.w3.org/2000/svg" class="h-16 w-16 text-gray-600 mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          <path stroke-linecap="round" stroke-linejoin="round" d="M11.48 3.499a.562.562 0 011.04 0l2.125 5.111a.563.563 0 00.475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 00-.182.557l1.285 5.385a.562.562 0 01-.84.61l-4.725-2.885a.563.563 0 00-.586 0L6.982 20.54a.563.563 0 01-.84-.61l1.285-5.386a.563.563 0 00-.182-.557l-4.204-3.602a.563.563 0 01.321-.988l5.518-.442a.563.563 0 00.475-.345L11.48 3.5z" />
        </svg>
        <p class="text-lg text-gray-400">No favorites yet</p>
        <p class="mt-2 text-sm text-gray-500">Star a channel or movie to add it here</p>
        <a href="/live" class="mt-4 rounded bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-500">
          Browse Channels
        </a>
      </div>
    {:else}
      <FavoriteList items={favorites} />
    {/if}
  </div>
</div>
