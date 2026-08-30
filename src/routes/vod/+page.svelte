<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { playlistStore, vodStore, settingsStore } from '$lib/stores';
  import { vodGetContinueWatching } from '$lib/api';
  import type { VodWatchProgress } from '$lib/types';
  import CategoryFilter from '$lib/components/vod/CategoryFilter.svelte';
  import VodGrid from '$lib/components/vod/VodGrid.svelte';
  import ContinueWatchingSection from '$lib/components/vod/ContinueWatchingSection.svelte';

  // A third mode alongside Movies/Series - not part of `vodStore.selectedType`
  // since Continue Watching shows both content types at once. Page-local.
  let viewMode = $state<'browse' | 'continue-watching'>('browse');
  let continueWatchingItems = $state<VodWatchProgress[]>([]);
  let continueWatchingLoading = $state(false);

  async function loadContinueWatching() {
    const playlistId = playlistStore.activePlaylistId;
    if (!playlistId) {
      continueWatchingItems = [];
      return;
    }
    continueWatchingLoading = true;
    try {
      // Fetched cross-playlist, filtered to the active playlist here - avoids
      // needing a playlist-scoped backend variant just for this tab.
      const all = await vodGetContinueWatching(50);
      continueWatchingItems = all.filter((item) => item.playlistId === playlistId);
    } catch (err) {
      console.error('Failed to load continue watching:', err);
      continueWatchingItems = [];
    } finally {
      continueWatchingLoading = false;
    }
  }

  function selectContinueWatchingTab() {
    viewMode = 'continue-watching';
    void loadContinueWatching();
  }

  let continueWatchingMovies = $derived(continueWatchingItems.filter((item) => item.contentType === 'movie'));
  let continueWatchingSeries = $derived(continueWatchingItems.filter((item) => item.contentType === 'series'));

  // Matches `VodGrid.svelte`'s default so this control shows the right
  // selected value before the user ever touches it.
  const DEFAULT_GRID_COLUMNS = 7;
  const GRID_COLUMN_OPTIONS = [3, 4, 5, 6, 7, 8, 9, 10];

  let loaded = $state(false);
  // Playlist id already loaded - distinct from `loaded` so the effect below
  // can tell "same playlist" apart from "switched, reload".
  let loadedForPlaylistId = $state<string | null>(null);

  onMount(async () => {
    const playlistId = playlistStore.activePlaylistId;
    if (!playlistId) {
      // No playlist yet - nothing to load, but `loaded` must still flip to
      // true, or the effect below (which picks up a playlist selected later)
      // never fires since it requires `loaded` already true.
      loaded = true;
      return;
    }

    // `vodStore` is a singleton that survives the `/vod` <-> `/vod/[id]`
    // remount untouched. If it already holds a settled category for this
    // playlist, this is a "Back" round trip - keep it as the user left it
    // (category, items, scroll position) instead of resetting.
    if (vodStore.loadedPlaylistId === playlistId && vodStore.selectedCategoryId !== null) {
      loadedForPlaylistId = playlistId;
      loaded = true;
      return;
    }

    await vodStore.loadCategories(playlistId);
    // No client-side "All Categories" any more - land on the provider's own
    // first category (see `vodStore.setType`'s identical comment).
    vodStore.setCategory(playlistId, vodStore.categories[0]?.categoryId ?? null);
    loadedForPlaylistId = playlistId;
    loaded = true;
  });

  // Reacts only to switching to a DIFFERENT playlist after initial load.
  //
  // Previously called `vodStore.loadItems(...)` directly inside this effect,
  // which reads/writes its own internal state (`loading`, `stalkerPage`,
  // etc.) - Svelte's tracking picked those up too, so every write re-triggered
  // this effect in a read-write feedback loop, firing bursts of duplicate
  // `get_ordered_list` requests (most refused by single-request-at-a-time
  // Stalker portals). `untrack` now scopes tracking to just `playlistId`/
  // `loadedForPlaylistId`; type/category switches trigger their own reload
  // explicitly via `setType`/`setCategory` instead.
  $effect(() => {
    const playlistId = playlistStore.activePlaylistId;
    if (playlistId && loaded && playlistId !== loadedForPlaylistId) {
      loadedForPlaylistId = playlistId;
      // Sequential, not concurrent - two requests in flight at once is
      // exactly what this whole fix exists to avoid.
      untrack(() => {
        void (async () => {
          await vodStore.loadCategories(playlistId);
          vodStore.setCategory(playlistId, vodStore.categories[0]?.categoryId ?? null);
        })();
      });
    }
  });
</script>

<div class="flex h-full w-full flex-col overflow-hidden bg-gray-900 text-white">
  {#if !playlistStore.activePlaylistId}
    <div class="flex flex-1 items-center justify-center">
      <div class="text-center">
        <svg xmlns="http://www.w3.org/2000/svg" class="mx-auto h-16 w-16 mb-4 text-gray-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          <path stroke-linecap="round" stroke-linejoin="round" d="M15.91 11.672a.375.375 0 010 .656l-5.603 3.113a.375.375 0 01-.557-.328V8.887c0-.286.307-.466.557-.327l5.603 3.112z" />
        </svg>
        <p class="text-lg text-gray-400">Select a playlist with VOD content</p>
        <p class="mt-2 text-sm text-gray-500">Choose a playlist from the sidebar</p>
      </div>
    </div>
  {:else}
    <div class="flex-shrink-0 space-y-4 border-b border-gray-800 bg-gray-900/95 px-6 py-4">
      <div class="flex items-center gap-1 rounded-lg bg-gray-800 p-1 w-fit">
        <button
          class={`rounded-md px-4 py-1.5 text-sm font-medium transition-colors ${
            viewMode === 'browse' && vodStore.selectedType === 'movie'
              ? 'bg-blue-600 text-white'
              : 'text-gray-300 hover:text-white'
          }`}
          onclick={() => {
            viewMode = 'browse';
            vodStore.setType(playlistStore.activePlaylistId ?? undefined, 'movie');
          }}
        >
          Movies
        </button>
        <button
          class={`rounded-md px-4 py-1.5 text-sm font-medium transition-colors ${
            viewMode === 'browse' && vodStore.selectedType === 'series'
              ? 'bg-blue-600 text-white'
              : 'text-gray-300 hover:text-white'
          }`}
          onclick={() => {
            viewMode = 'browse';
            vodStore.setType(playlistStore.activePlaylistId ?? undefined, 'series');
          }}
        >
          Series
        </button>
        <button
          class={`rounded-md px-4 py-1.5 text-sm font-medium transition-colors ${
            viewMode === 'continue-watching'
              ? 'bg-blue-600 text-white'
              : 'text-gray-300 hover:text-white'
          }`}
          onclick={selectContinueWatchingTab}
        >
          Continue Watching
        </button>
      </div>

      {#if viewMode === 'browse'}
      <div class="flex flex-wrap items-center gap-3">
        <div class="relative min-w-[220px] flex-1">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-500"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            stroke-width="2"
          >
            <path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-4.35-4.35M17 11a6 6 0 11-12 0 6 6 0 0112 0z" />
          </svg>
          <input
            type="text"
            placeholder={`Search ${vodStore.selectedType === 'movie' ? 'movies' : 'series'}...`}
            value={vodStore.searchQuery}
            oninput={(e) => vodStore.setSearch((e.target as HTMLInputElement).value)}
            class="w-full rounded-lg border border-gray-700 bg-gray-800 py-2 pl-9 pr-3 text-sm text-white placeholder-gray-500 outline-none transition-colors focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
          />
        </div>

        <CategoryFilter />

        <div class="relative">
          <select
            value={vodStore.sortBy}
            onchange={(e) => vodStore.setSort((e.target as HTMLSelectElement).value as any)}
            class="appearance-none rounded-lg border border-gray-700 bg-gray-800 py-2 pl-3 pr-9 text-sm text-white outline-none transition-colors hover:bg-gray-750 focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
            aria-label="Sort by"
          >
            <option value="default">Sort: Default</option>
            <option value="name-az">Name (A-Z)</option>
            <option value="name-za">Name (Z-A)</option>
            <option value="year-desc">Year (Newest)</option>
            <option value="year-asc">Year (Oldest)</option>
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

        <div class="relative">
          <select
            value={settingsStore.settings?.vodGridColumns ?? DEFAULT_GRID_COLUMNS}
            onchange={(e) => settingsStore.update({ vodGridColumns: Number((e.target as HTMLSelectElement).value) })}
            class="appearance-none rounded-lg border border-gray-700 bg-gray-800 py-2 pl-3 pr-9 text-sm text-white outline-none transition-colors hover:bg-gray-750 focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
            aria-label="Movies per row"
          >
            {#each GRID_COLUMN_OPTIONS as count}
              <option value={count}>{count} per row</option>
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
      </div>
      {/if}
    </div>

    <!-- Not `overflow-y-auto` itself when showing the grid - `VodGrid` must
         be the sole scroll container so its scroll-position virtualization
         reflects reality, not a second outer container racing it. Other
         branches (short, bounded content) scroll independently. -->
    <div class="flex-1 overflow-hidden">
      {#if viewMode === 'continue-watching'}
        <div class="h-full overflow-y-auto p-6 space-y-8">
          {#if continueWatchingLoading && continueWatchingItems.length === 0}
            <p class="text-gray-400">Loading…</p>
          {:else if continueWatchingMovies.length === 0 && continueWatchingSeries.length === 0}
            <div class="flex items-center justify-center py-16">
              <p class="text-gray-400">Nothing in progress yet - start watching a movie or series and it'll show up here.</p>
            </div>
          {:else}
            <ContinueWatchingSection title="Movies" items={continueWatchingMovies} />
            <ContinueWatchingSection title="Series" items={continueWatchingSeries} />
          {/if}
        </div>
      {:else if vodStore.error}
        <div class="h-full overflow-y-auto p-6">
          <div class="flex items-center justify-center py-16">
            <div class="text-center">
              <p class="text-red-400">Failed to load {vodStore.selectedType === 'movie' ? 'movies' : 'series'}: {vodStore.error}</p>
              <button
                class="mt-4 rounded-lg bg-gray-800 px-4 py-2 text-sm font-medium text-gray-200 hover:bg-gray-700"
                onclick={() => vodStore.retry(playlistStore.activePlaylistId ?? '')}
              >
                Retry
              </button>
            </div>
          </div>
        </div>
      {:else if vodStore.loading && vodStore.vodItems.length === 0}
        <div class="h-full overflow-y-auto p-6">
          <div class="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6">
            {#each Array(12) as _}
              <div class="animate-pulse">
                <div class="aspect-[2/3] rounded-lg bg-gray-800"></div>
                <div class="mt-2 h-4 rounded bg-gray-800"></div>
                <div class="mt-1 h-3 w-2/3 rounded bg-gray-800"></div>
              </div>
            {/each}
          </div>
        </div>
      {:else if vodStore.filteredItems.length === 0}
        <div class="h-full overflow-y-auto p-6">
          <div class="flex items-center justify-center py-16">
            <p class="text-gray-400">
              {#if vodStore.searchQuery.trim()}
                No {vodStore.selectedType === 'movie' ? 'movies' : 'series'} found matching "{vodStore.searchQuery}".
              {:else}
                No {vodStore.selectedType === 'movie' ? 'movies' : 'series'} found in this category.
              {/if}
            </p>
          </div>
        </div>
      {:else}
        <!-- Keyed on category: forces a fresh `VodGrid` instance (fresh
             scrollTop/virtualization) on a real category switch.
             `initialScrollTop` then starts at 0 for a switch, or the
             preserved position on a return from detail. -->
        {#key vodStore.selectedCategoryId}
          <VodGrid
            items={vodStore.filteredItems}
            playlistId={playlistStore.activePlaylistId ?? undefined}
            contentType={vodStore.selectedType}
            preferredColumns={settingsStore.settings?.vodGridColumns ?? DEFAULT_GRID_COLUMNS}
            initialScrollTop={vodStore.scrollPosition}
            onScrollTopChange={(pos) => vodStore.setScrollPosition(pos)}
            onLoadMore={() => {
              const playlistId = playlistStore.activePlaylistId;
              if (playlistId && vodStore.selectedCategoryId) {
                void vodStore.loadMoreLive(playlistId, vodStore.selectedCategoryId);
              }
            }}
          />
        {/key}
      {/if}
    </div>
  {/if}
</div>
