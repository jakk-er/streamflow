<script lang="ts">
  import { favoritesStore, playlistStore, playerStore } from '$lib/stores';
  import { goto } from '$app/navigation';
  import type { FavoriteChannel } from '$lib/types';

  let { item }: { item: FavoriteChannel } = $props();

  let playlist = $derived(playlistStore.playlists.find(p => p._id === item.playlistId));
  let isFav = $derived(favoritesStore.isFavorite(item.channelId));

  let displayName = $derived(item.channelName ?? `Favorite #${item.channelId.slice(0, 8)}`);

  function getInitial(name: string) {
    return name.charAt(0).toUpperCase();
  }

  function handleToggleFavorite() {
    favoritesStore.toggle(item.channelId, item.playlistId, item.favoriteType);
  }

  function handleFavoriteKeyDown(event: KeyboardEvent) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      event.stopPropagation();
      favoritesStore.toggle(item.channelId, item.playlistId, item.favoriteType);
    }
  }

  async function handlePlay() {
    // For now, navigate to live page since we don't have direct stream URL in FavoriteChannel
    await goto('/live');
  }
</script>

<button
  class="group text-left"
  onclick={handlePlay}
>
  <div class="relative aspect-[2/3] overflow-hidden rounded-lg bg-gray-800">
    {#if item.channelLogo}
      <img src={item.channelLogo} alt="" class="h-full w-full object-cover" />
    {:else}
      <div class="flex h-full w-full items-center justify-center bg-gradient-to-br from-gray-700 to-gray-900">
        <span class="text-4xl font-bold text-gray-500">{getInitial(displayName)}</span>
      </div>
    {/if}

    <div class="absolute inset-0 bg-black/0 transition-colors duration-300 group-hover:bg-black/20"></div>

    <div class="absolute top-2 left-2">
      <span class="rounded bg-black/70 px-2 py-0.5 text-xs text-gray-300">
        {item.favoriteType === 'global' ? 'Global' : 'Channel'}
      </span>
    </div>

    {#if playlist}
      <span class="absolute bottom-2 left-2 rounded bg-black/70 px-2 py-0.5 text-xs text-gray-300 truncate max-w-[80%]">
        {playlist.title}
      </span>
    {/if}

    <div class="absolute inset-0 flex items-center justify-center opacity-0 transition-opacity duration-300 group-hover:opacity-100">
      <div class="rounded-full bg-white/20 p-3 backdrop-blur-sm">
        <svg xmlns="http://www.w3.org/2000/svg" class="h-8 w-8 text-white" fill="currentColor" viewBox="0 0 24 24">
          <path d="M8 5v14l11-7z" />
        </svg>
      </div>
    </div>
  </div>

  <div class="mt-2">
    <h3 class="truncate text-sm font-medium text-white group-hover:text-blue-400 transition-colors">
      {displayName}
    </h3>
    <p class="mt-1 truncate text-xs text-gray-400">
      {new Date(item.createdAt).toLocaleDateString()}
    </p>
  </div>

  <div
    class={`absolute top-2 right-2 p-1.5 rounded-full ${isFav ? 'text-yellow-400' : 'text-gray-400 hover:text-white'}`}
    onclick={(e) => { e.stopPropagation(); handleToggleFavorite(); }}
    onkeydown={handleFavoriteKeyDown}
    role="button"
    tabindex="0"
    aria-label="Toggle favorite"
  >
    <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="currentColor" viewBox="0 0 24 24">
      <path d="M11.48 3.499a.562.562 0 011.04 0l2.125 5.111a.563.563 0 00.475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 00-.182.557l1.285 5.385a.562.562 0 01-.84.61l-4.725-2.885a.563.563 0 00-.586 0L6.982 20.54a.563.563 0 01-.84-.61l1.285-5.386a.563.563 0 00-.182-.557l-4.204-3.602a.563.563 0 01.321-.988l5.518-.442a.563.563 0 00.475-.345L11.48 3.5z" />
    </svg>
  </div>
</button>
