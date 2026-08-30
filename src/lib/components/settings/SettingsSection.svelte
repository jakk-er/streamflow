<script lang="ts">
  import type { Snippet } from 'svelte';

  let { title, description, id, children }: { title: string; description?: string; id: string; children: Snippet } = $props();
</script>

<div {id} class="border-b border-gray-700 pb-6 mb-6">
  <div class="flex items-center gap-3 mb-1">
    <h2 class="text-xl font-semibold text-white">{title}</h2>
  </div>
  {#if description}
    <p class="text-sm text-gray-400 mt-1">{description}</p>
  {/if}
  {@render children()}
</div>
