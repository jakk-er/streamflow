<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { playlistStore, vodStore } from '$lib/stores';
  import { goto } from '$app/navigation';
  import VodDetail from '$lib/components/vod/VodDetail.svelte';

  let { data }: { data: any } = $props();

  let id = $state('');
  let type = $state<'movie' | 'series'>('movie');
  let autoplayAction = $state<'resume' | 'startover' | null>(null);
  // `id` already loaded. SvelteKit reuses this component instance across
  // `/vod/:id` navigations, so this lets the effect below tell "same title"
  // apart from "navigated, reload".
  let loadedDetailFor = $state<string | null>(null);

  onMount(async () => {
    // Access page params via SvelteKit page store if available
    // For now, extract from URL as fallback
    const path = window.location.pathname;
    const match = path.match(/\/vod\/([^/]+)/);
    if (match) {
      id = match[1];
    }

    const url = new URL(window.location.href);
    const urlType = url.searchParams.get('type');
    if (urlType === 'movie' || urlType === 'series') {
      type = urlType;
    }
    const urlAutoplay = url.searchParams.get('autoplay');
    if (urlAutoplay === 'resume' || urlAutoplay === 'startover') {
      autoplayAction = urlAutoplay;
    }

    if (playlistStore.activePlaylistId && id) {
      await vodStore.loadDetail(playlistStore.activePlaylistId, id, type);
      loadedDetailFor = id;
    }
  });

  // Reacts only to `id` changing to a DIFFERENT title after initial load.
  //
  // Used to compare `id` against `vodStore.currentDetail`, which on a fresh
  // navigation still holds the PREVIOUS title (singleton store, not cleared
  // between navigations) - causing a spurious `loadDetail` call racing
  // `onMount`'s, plus the same read-write feedback risk as the VOD list
  // page's effect (`routes/vod/+page.svelte`). `loadDetail` also guards
  // against this race now, but fixing the trigger avoids the redundant
  // request outright.
  $effect(() => {
    const playlistId = playlistStore.activePlaylistId;
    if (playlistId && id && id !== loadedDetailFor) {
      loadedDetailFor = id;
      untrack(() => {
        void vodStore.loadDetail(playlistId, id, type);
      });
    }
  });
</script>

<div class="flex h-full w-full flex-col overflow-hidden bg-gray-900 text-white">
  {#if vodStore.currentDetail}
    <VodDetail detail={vodStore.currentDetail} {type} {autoplayAction} />
  {:else if vodStore.loading}
    <div class="flex flex-1 items-center justify-center">
      <div class="h-8 w-8 animate-spin rounded-full border-4 border-gray-700 border-t-blue-500"></div>
    </div>
  {:else}
    <div class="flex flex-1 items-center justify-center">
      <div class="text-center">
        <p class="text-gray-400">Failed to load details</p>
        {#if vodStore.error}
          <p class="mt-2 text-sm text-red-400">{vodStore.error}</p>
        {/if}
        <button class="mt-4 rounded bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-500" onclick={() => goto('/vod')}>
          Back to {type === 'series' ? 'Series' : 'Movies'}
        </button>
      </div>
    </div>
  {/if}
</div>
