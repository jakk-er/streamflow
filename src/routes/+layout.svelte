<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { navigating } from '$app/stores';
  import { playlistStore, settingsStore, favoritesStore } from '$lib/stores';
  import TitleBar from '$lib/components/layout/TitleBar.svelte';
  import Sidebar from '$lib/components/layout/Sidebar.svelte';
  import CommandPalette from '$lib/components/layout/CommandPalette.svelte';
  import ToastProvider from '$lib/components/layout/ToastProvider.svelte';
  import NowPlayingOverlay from '$lib/components/player/NowPlayingOverlay.svelte';
  import { registerShortcut } from '$lib/utils/keyboard';
  import type { Snippet } from 'svelte';

  let { children }: { children: Snippet } = $props();

  let sidebarVisible = $state(true);
  let titleBarHeight = $state(48);

  onMount(() => {
    (async () => {
      await playlistStore.loadPlaylists();
      await settingsStore.load();
      await favoritesStore.loadFavorites();

      // Auto-activate the user's designated default playlist (star toggle in
      // Settings > Playlists), only if nothing's active yet and it still
      // exists (silently no-op if it was since deleted).
      const defaultId = settingsStore.settings?.defaultPlaylistId;
      if (defaultId && !playlistStore.activePlaylistId && playlistStore.playlists.some((p) => p._id === defaultId)) {
        playlistStore.setActive(defaultId);
      }
    })();

    const unregister = registerShortcut('ctrl+b', () => {
      sidebarVisible = !sidebarVisible;
    });

    return () => {
      unregister();
    };
  });

  function toggleSidebar() {
    sidebarVisible = !sidebarVisible;
  }
</script>

<div class="flex flex-col h-screen w-screen overflow-hidden bg-background text-foreground dark" data-theme={settingsStore.theme}>
  <TitleBar onToggleSidebar={toggleSidebar} />
  <div class="flex flex-1 overflow-hidden">
    <Sidebar visible={sidebarVisible} />
    <main class="relative flex-1 overflow-auto bg-background">
      {@render children?.()}
      <NowPlayingOverlay />
      {#if $navigating}
        <div class="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm">
          <div class="h-12 w-12 animate-spin rounded-full border-4 border-primary border-t-transparent"></div>
        </div>
      {/if}
    </main>
  </div>
  <CommandPalette />
  <ToastProvider />
</div>
