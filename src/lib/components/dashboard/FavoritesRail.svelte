<script lang="ts">
  import { goto } from '$app/navigation';
  import { reorderFavorites } from '$lib/api';
  import type { FavoriteChannel } from '$lib/types';
  import { isKnownBadLogoUrl } from '$lib/utils/brokenLogoHosts';

  let { items = [], playlistId = null }: { items?: FavoriteChannel[]; playlistId?: string | null } = $props();

  // Local, freely-reorderable copy of `items` - re-synced whenever the
  // active playlist's favorite set actually changes (new prop reference),
  // but never overwritten mid-drag by a stale re-render of the same data.
  let order = $state<FavoriteChannel[]>([]);
  $effect(() => {
    order = items;
  });

  let failedLogoIds = $state(new Set<string>());
  let draggedId = $state<string | null>(null);
  let dragOverId = $state<string | null>(null);

  function handleClick(event: MouseEvent, item: FavoriteChannel) {
    if (draggedId) {
      event.preventDefault();
      return;
    }
    goto('/live');
  }

  function handleDragStart(event: DragEvent, item: FavoriteChannel) {
    draggedId = item.channelId;
    event.dataTransfer?.setData('text/plain', item.channelId);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
  }

  function handleDragOver(event: DragEvent, item: FavoriteChannel) {
    event.preventDefault();
    dragOverId = item.channelId;
  }

  function handleDragEnd() {
    draggedId = null;
    dragOverId = null;
  }

  function handleDrop(event: DragEvent, target: FavoriteChannel) {
    event.preventDefault();
    dragOverId = null;
    const sourceId = draggedId;
    draggedId = null;
    if (!sourceId || sourceId === target.channelId || !playlistId) return;

    const sourceIndex = order.findIndex((f) => f.channelId === sourceId);
    const targetIndex = order.findIndex((f) => f.channelId === target.channelId);
    if (sourceIndex === -1 || targetIndex === -1) return;

    const next = [...order];
    const [moved] = next.splice(sourceIndex, 1);
    next.splice(targetIndex, 0, moved);
    order = next;

    reorderFavorites(playlistId, next.map((f) => f.channelId)).catch((err) => {
      console.error('Failed to save favorites order:', err);
    });
  }
</script>

{#if order.length > 0}
  <div class="space-y-3">
    <div class="flex items-center justify-between">
      <h2 class="text-lg font-semibold text-white">Your Favorites</h2>
      <a href="/favorites" class="text-sm text-blue-400 hover:text-blue-300"> See All </a>
    </div>

    <div class="flex gap-3 overflow-x-auto pb-2 snap-x snap-mandatory" style="scrollbar-width: none;">
      {#each order as item (item.channelId)}
        <button
          class={`flex-shrink-0 w-[200px] snap-start text-left group cursor-grab active:cursor-grabbing ${draggedId === item.channelId ? 'opacity-40' : ''} ${dragOverId === item.channelId && draggedId && draggedId !== item.channelId ? 'ring-2 ring-blue-500 rounded-lg' : ''}`}
          draggable="true"
          onclick={(e) => handleClick(e, item)}
          ondragstart={(e) => handleDragStart(e, item)}
          ondragover={(e) => handleDragOver(e, item)}
          ondragend={handleDragEnd}
          ondrop={(e) => handleDrop(e, item)}
        >
          <div class="relative aspect-video rounded-lg bg-gray-800 overflow-hidden">
            <div class="flex h-full w-full items-center justify-center bg-gradient-to-br from-gray-700 to-gray-900">
              {#if item.channelLogo && !failedLogoIds.has(item.channelId) && !isKnownBadLogoUrl(item.channelLogo)}
                <img
                  src={item.channelLogo}
                  alt=""
                  class="h-full w-full object-cover"
                  onerror={() => (failedLogoIds = new Set(failedLogoIds).add(item.channelId))}
                />
              {:else}
                <span class="text-2xl font-bold text-gray-500">
                  {(item.channelName ?? '?').charAt(0).toUpperCase()}
                </span>
              {/if}
            </div>

            <div class="absolute inset-0 bg-black/0 group-hover:bg-black/20 transition-colors duration-300"></div>
          </div>

          <div class="mt-2">
            <p class="text-sm text-white truncate group-hover:text-blue-400 transition-colors">
              {item.channelName ?? 'Favorite'}
            </p>
          </div>
        </button>
      {/each}
    </div>
  </div>
{/if}
