<script lang="ts">
  import type { VodWatchProgress } from '$lib/types';
  import ContinueWatchingCard from '$lib/components/vod/ContinueWatchingCard.svelte';

  let { title, items }: { title: string; items: VodWatchProgress[] } = $props();
</script>

{#if items.length > 0}
  <div class="space-y-3">
    <h2 class="text-lg font-semibold text-white">{title}</h2>
    <div class="flex gap-4 overflow-x-auto pb-2 snap-x snap-mandatory" style="scrollbar-width: thin;">
      {#each items as item (item.id)}
        <ContinueWatchingCard {item} />
      {/each}
    </div>
  </div>
{/if}
