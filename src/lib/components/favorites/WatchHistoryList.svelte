<script lang="ts">
  import type { WatchHistoryItem as WatchHistoryItemType } from '$lib/types';
  import WatchHistoryItem from '$lib/components/favorites/WatchHistoryItem.svelte';

  let { items = [] }: { items?: WatchHistoryItemType[] } = $props();

  function getDateLabel(dateStr: string): string {
    const date = new Date(dateStr);
    const now = new Date();
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const yesterday = new Date(today.getTime() - 86400000);
    const itemDate = new Date(date.getFullYear(), date.getMonth(), date.getDate());

    if (itemDate.getTime() === today.getTime()) return 'Today';
    if (itemDate.getTime() === yesterday.getTime()) return 'Yesterday';
    const diffDays = Math.floor((today.getTime() - itemDate.getTime()) / 86400000);
    if (diffDays < 7) return 'This Week';
    return 'Earlier';
  }

  let grouped = $derived(items.reduce((acc, item) => {
    const label = getDateLabel(item.watchedAt);
    if (!acc[label]) acc[label] = [];
    acc[label].push(item);
    return acc;
  }, {} as Record<string, WatchHistoryItemType[]>));

  let groupOrder = $derived(['Today', 'Yesterday', 'This Week', 'Earlier']);
</script>

<div class="space-y-6">
  {#each groupOrder as groupLabel}
    {#if grouped[groupLabel] && grouped[groupLabel].length > 0}
      <div>
        <h3 class="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-3 sticky top-0 bg-gray-900 py-2">
          {groupLabel}
        </h3>
        <div class="space-y-2">
          {#each grouped[groupLabel] as item (item.id)}
            <WatchHistoryItem {item} />
          {/each}
        </div>
      </div>
    {/if}
  {/each}
</div>
