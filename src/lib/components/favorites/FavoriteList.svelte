<script lang="ts">
  import type { FavoriteChannel } from '$lib/types';
  import FavoriteCard from '$lib/components/favorites/FavoriteCard.svelte';

  let { items = [] }: { items?: FavoriteChannel[] } = $props();
</script>

<div class="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
  {#each items as item (item.id)}
    <FavoriteCard {item} />
  {/each}
</div>
