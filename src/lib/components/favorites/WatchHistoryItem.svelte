<script lang="ts">
  import { favoritesStore, playlistStore, playerStore } from '$lib/stores';
  import { goto } from '$app/navigation';
  import type { WatchHistoryItem } from '$lib/types';

  let { item }: { item: WatchHistoryItem } = $props();

  let playlist = $derived(playlistStore.playlists.find(p => p._id === item.playlistId));
  let displayName = $derived(item.channelName ?? `Channel ${item.channelId?.slice(0, 8) ?? 'Unknown'}`);

  function getInitial(name: string) {
    return name.charAt(0).toUpperCase();
  }

  function formatWatchedAt(dateStr: string) {
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) return 'Just now';
    if (diffMins < 60) return `${diffMins} minute${diffMins > 1 ? 's' : ''} ago`;
    if (diffHours < 24) return `${diffHours} hour${diffHours > 1 ? 's' : ''} ago`;
    if (diffDays === 1) return 'Yesterday';
    if (diffDays < 7) return `${diffDays} days ago`;
    return date.toLocaleDateString();
  }

  function getProgress() {
    if (!item.totalSeconds || item.totalSeconds <= 0) return 0;
    return Math.round((item.positionSeconds / item.totalSeconds) * 100);
  }

  async function handlePlay() {
    // Placeholder: would need to look up the actual channel/VOD item
    await goto('/live');
  }

  let removing = $state(false);

  async function handleRemove() {
    if (removing) return;
    removing = true;
    try {
      await favoritesStore.removeHistoryItem(item.id);
    } finally {
      removing = false;
    }
  }
</script>

<div class="flex items-center gap-4 rounded-lg bg-gray-800 p-3 hover:bg-gray-700 transition-colors">
  <div class="flex-shrink-0 w-[120px] aspect-video rounded bg-gray-700 overflow-hidden">
    {#if item.channelLogo}
      <img src={item.channelLogo} alt="" class="h-full w-full object-cover" />
    {:else}
      <div class="flex h-full w-full items-center justify-center bg-gradient-to-br from-gray-700 to-gray-900">
        <span class="text-2xl font-bold text-gray-500">{getInitial(displayName)}</span>
      </div>
    {/if}
  </div>

  <div class="flex-1 min-w-0">
    <div class="flex items-center gap-2">
      <h3 class="truncate text-sm font-medium text-white">
        {displayName}
      </h3>
      {#if playlist}
        <span class="text-xs text-gray-400 truncate">• {playlist.title}</span>
      {/if}
    </div>

    <p class="mt-1 text-xs text-gray-400">{formatWatchedAt(item.watchedAt)}</p>

    {#if getProgress() > 0}
      <div class="mt-2 flex items-center gap-2">
        <div class="h-1 flex-1 rounded-full bg-gray-700 overflow-hidden">
          <div class="h-full bg-blue-500" style="width: {getProgress()}%"></div>
        </div>
        <span class="text-xs text-gray-400">{getProgress()}%</span>
      </div>
    {/if}
  </div>

  <div class="flex items-center gap-2 flex-shrink-0">
    <button
      class="rounded-full bg-blue-600 p-2 text-white hover:bg-blue-500 transition-colors"
      onclick={handlePlay}
      aria-label="Play"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="currentColor" viewBox="0 0 24 24">
        <path d="M8 5v14l11-7z" />
      </svg>
    </button>

    <button
      class="rounded-full bg-gray-700 p-2 text-gray-300 hover:text-white hover:bg-gray-600 transition-colors disabled:opacity-50"
      onclick={handleRemove}
      disabled={removing}
      aria-label="Remove from history"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
      </svg>
    </button>
  </div>
</div>
