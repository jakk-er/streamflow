<script lang="ts">
  import { playlistStore } from '$lib/stores';
  import { goto } from '$app/navigation';
  import type { Playlist } from '$lib/types';

  let { items = [] }: { items?: Playlist[] } = $props();

  function getTypeColor(type: string | undefined) {
    switch (type) {
      case 'm3u': return 'from-green-500 to-emerald-600';
      case 'xtream': return 'from-blue-500 to-indigo-600';
      case 'stalker': return 'from-purple-500 to-pink-600';
      default: return 'from-gray-500 to-gray-600';
    }
  }

  function getTypeLabel(type: string | undefined) {
    switch (type) {
      case 'm3u': return 'M3U';
      case 'xtream': return 'Xtream';
      case 'stalker': return 'Stalker';
      default: return type || 'Unknown';
    }
  }

  function handleClick(playlist: Playlist) {
    playlistStore.setActive(playlist._id);
    goto('/live');
  }

  function handleAddClick() {
    goto('/settings?tab=playlists&action=add');
  }
</script>

<div class="space-y-4">
  <div class="flex items-center justify-between">
    <h2 class="text-lg font-semibold text-white">Your Playlists</h2>
  </div>

  <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
    {#each items as playlist}
      <button
        class="group relative flex flex-col rounded-xl bg-gray-800 p-4 text-left hover:bg-gray-750 transition-all duration-200 border border-gray-700 hover:border-gray-600 hover:shadow-lg"
        onclick={() => handleClick(playlist)}
      >
        <div class="flex items-center gap-3 mb-3">
          <div class="w-10 h-10 rounded-lg bg-gradient-to-br {getTypeColor(playlist.playlistType)} flex items-center justify-center flex-shrink-0">
            <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3" />
            </svg>
          </div>
          <div class="min-w-0 flex-1">
            <h3 class="text-sm font-medium text-white truncate group-hover:text-blue-400 transition-colors">
              {playlist.title}
            </h3>
            <span class="inline-block mt-1 rounded bg-gray-700 px-2 py-0.5 text-xs text-gray-300">
              {getTypeLabel(playlist.playlistType)}
            </span>
          </div>
        </div>

        <p class="text-xs text-gray-400 truncate">
          {playlist.serverUrl || playlist.url || 'No URL'}
        </p>

        <div class="absolute top-3 right-3 opacity-0 group-hover:opacity-100 transition-opacity">
          <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
          </svg>
        </div>
      </button>
    {/each}

    <button
      class="flex flex-col items-center justify-center rounded-xl border-2 border-dashed border-gray-700 p-4 text-center hover:border-gray-500 hover:bg-gray-800/50 transition-colors min-h-[120px]"
      onclick={handleAddClick}
    >
      <div class="w-10 h-10 rounded-full bg-gray-700 flex items-center justify-center mb-2">
        <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 4v16m8-8H4" />
        </svg>
      </div>
      <span class="text-sm font-medium text-gray-400">Add Playlist</span>
    </button>
  </div>
</div>
