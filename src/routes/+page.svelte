<script lang="ts">
  import { onMount } from 'svelte';
  import { playlistStore, settingsStore } from '$lib/stores';
  import { goto } from '$app/navigation';
  import { vodGetContinueWatching, getFavorites } from '$lib/api';
  import type { VodWatchProgress, FavoriteChannel } from '$lib/types';
  import HeroBanner from '$lib/components/dashboard/HeroBanner.svelte';
  import PlaylistQuickAccess from '$lib/components/dashboard/PlaylistQuickAccess.svelte';
  import FavoritesRail from '$lib/components/dashboard/FavoritesRail.svelte';
  import TrendingRail from '$lib/components/dashboard/TrendingRail.svelte';
  import ContinueWatchingRail from '$lib/components/favorites/ContinueWatchingRail.svelte';

  // Unfiltered, cross-playlist - `continueWatching` below scopes it to the
  // active playlist so switching playlists doesn't show leftover progress.
  let allContinueWatching = $state<VodWatchProgress[]>([]);

  onMount(async () => {
    await playlistStore.loadPlaylists();
    try {
      // `vod_get_continue_watching` already returns only in-progress rows
      // (past the 60s/2% threshold, not yet at completion - which deletes
      // the row, see `commands::vod_progress`); no extra filtering needed here.
      allContinueWatching = await vodGetContinueWatching(50);
    } catch (err) {
      console.error('Failed to load continue watching:', err);
    }
    await settingsStore.load();
  });

  let playlists = $derived(playlistStore.playlists);
  let settings = $derived(settingsStore.settings);

  let continueWatching = $derived(allContinueWatching.filter((item) => item.playlistId === playlistStore.activePlaylistId));

  let heroItem = $derived(continueWatching[0] ?? null);

  let hasPlaylists = $derived(playlists.length > 0);

  let favoriteChannels = $state<FavoriteChannel[]>([]);
  $effect(() => {
    const playlistId = playlistStore.activePlaylistId;
    if (!playlistId) {
      favoriteChannels = [];
      return;
    }
    getFavorites(playlistId)
      .then((list) => {
        favoriteChannels = list;
      })
      .catch((err) => {
        console.error('Failed to load favorites:', err);
        favoriteChannels = [];
      });
  });

  function handleAddPlaylist() {
    goto('/settings?tab=playlists&action=add');
  }
</script>

<div class="flex h-full w-full flex-col overflow-hidden bg-gray-900 text-white">
  {#if !hasPlaylists}
    <div class="flex flex-1 items-center justify-center">
      <div class="text-center max-w-md px-6">
        <img src="/streamflow-icon.svg" alt="" class="w-20 h-20 mx-auto mb-6" />
        <h1 class="text-3xl font-bold text-white mb-3">Welcome to StreamFlow</h1>
        <p class="text-gray-400 mb-8">Add your first playlist to get started.</p>
        <button
          class="rounded-xl bg-blue-600 px-8 py-3 text-base font-semibold text-white hover:bg-blue-500 transition-colors shadow-lg shadow-blue-600/20"
          onclick={handleAddPlaylist}
        >
          Add Playlist
        </button>
      </div>
    </div>
  {:else}
    <div class="flex-1 overflow-y-auto">
      <div class="max-w-7xl mx-auto px-4 py-6 space-y-10">
        {#if heroItem}
          <HeroBanner item={heroItem} />
        {/if}

        {#if continueWatching.length > 0}
          <section>
            <ContinueWatchingRail items={continueWatching} />
          </section>
        {/if}

        <section>
          <PlaylistQuickAccess items={playlists} />
        </section>

        <section>
          <FavoritesRail items={favoriteChannels} playlistId={playlistStore.activePlaylistId} />
        </section>

        <section>
          <TrendingRail />
        </section>
      </div>
    </div>
  {/if}
</div>
