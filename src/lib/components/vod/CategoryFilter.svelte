<script lang="ts">
  import { playlistStore, vodStore } from '$lib/stores';

  function handleChange(event: Event) {
    const value = (event.target as HTMLSelectElement).value;
    vodStore.setCategory(playlistStore.activePlaylistId ?? undefined, value);
  }
</script>

<div class="relative">
  <select
    value={vodStore.selectedCategoryId ?? ''}
    onchange={handleChange}
    class="appearance-none rounded-lg border border-gray-700 bg-gray-800 py-2 pl-3 pr-9 text-sm text-white outline-none transition-colors hover:bg-gray-750 focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
    aria-label="Filter by category"
  >
    {#each vodStore.categories as category}
      <option value={String(category.categoryId)}>{category.categoryName}</option>
    {/each}
  </select>
  <svg
    xmlns="http://www.w3.org/2000/svg"
    class="pointer-events-none absolute right-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-400"
    fill="none"
    viewBox="0 0 24 24"
    stroke="currentColor"
    stroke-width="2"
  >
    <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
  </svg>
</div>
