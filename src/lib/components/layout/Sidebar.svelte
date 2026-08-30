<script lang="ts">
  import { playlistStore, favoritesStore } from '$lib/stores';
  import { goto } from '$app/navigation';
  import ResizablePane from './ResizablePane.svelte';
  import Icon from '$lib/components/ui/Icon.svelte';
  import Card from '$lib/components/ui/Card.svelte';
  import Badge from '$lib/components/ui/Badge.svelte';
  import Button from '$lib/components/ui/Button.svelte';

  let { visible = $bindable(true) }: { visible?: boolean } = $props();

  const playlists = $derived(() => playlistStore.playlists);
  const activeId = $derived(() => playlistStore.activePlaylistId);
  const favoritesCount = $derived(() => favoritesStore.favorites?.length ?? 0);

  function selectPlaylist(id: string) {
    playlistStore.setActive(id);
  }

  function navigate(path: string) {
    goto(path);
  }
</script>

{#if visible}
  <ResizablePane side="left" defaultWidth={280}>
    <div class="glass glass-surface flex h-full w-full flex-col border-r border-border text-text-primary">
      <div class="flex items-center justify-between border-b border-border px-4 py-3">
        <div class="flex items-center gap-2">
          <img src="/streamflow-icon.svg" alt="" class="h-9 w-9 flex-shrink-0" />
          <h2 class="text-lg font-semibold tracking-tight">StreamFlow</h2>
        </div>
        <button
          class="rounded-lg p-1.5 text-text-secondary hover:text-text-primary hover:bg-surface-hover transition-all duration-200"
          onclick={() => (visible = false)}
          aria-label="Close sidebar"
        >
          <Icon name="X" size={18} />
        </button>
      </div>

      <div class="flex-1 overflow-y-auto px-3 py-3">
        <div class="mb-5">
          <h3 class="mb-2 px-2 text-xs font-semibold uppercase tracking-wider text-text-muted">Playlists</h3>
          <ul class="space-y-1">
            {#each playlists().filter(p => p._id) as playlist (playlist._id)}
              <li>
                <button
                  class={`w-full rounded-lg px-3 py-2 text-left text-sm transition-all duration-200 ${
                    activeId() === playlist._id
                      ? 'border-l-2 border-primary bg-primary/5 text-text-primary'
                      : 'border-l-2 border-transparent text-text-secondary hover:bg-surface-hover hover:text-text-primary'
                  }`}
                  onclick={() => selectPlaylist(playlist._id)}
                >
                  <div class="flex items-center gap-2">
                    <Icon name="Tv" size={16} class="shrink-0" />
                    <span class="truncate">{playlist.title}</span>
                    <span class="ml-auto text-xs text-text-muted">
                      {playlist.playlistType === 'm3u' ? 'M3U' : playlist.playlistType === 'xtream' ? 'XC' : 'ST'}
                    </span>
                  </div>
                </button>
              </li>
            {/each}
            {#if playlists().length === 0}
              <li class="px-3 py-2 text-sm text-text-muted">No playlists yet</li>
            {/if}
          </ul>
        </div>

        <div class="mb-5">
          <h3 class="mb-2 px-2 text-xs font-semibold uppercase tracking-wider text-text-muted">Navigation</h3>
          <ul class="space-y-1">
            <li>
              <a href="/" class="flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm text-text-secondary hover:bg-surface-hover hover:text-text-primary transition-all duration-200">
                <Icon name="Home" size={16} />
                <span>Home</span>
              </a>
            </li>
            <li>
              <a href="/live" class="flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm text-text-secondary hover:bg-surface-hover hover:text-text-primary transition-all duration-200">
                <Icon name="Tv" size={16} />
                <span>Live TV</span>
              </a>
            </li>
            <li>
              <a href="/vod" class="flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm text-text-secondary hover:bg-surface-hover hover:text-text-primary transition-all duration-200">
                <Icon name="Film" size={16} />
                <span>VOD</span>
              </a>
            </li>
            <li>
              <a href="/favorites" class="flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm text-text-secondary hover:bg-surface-hover hover:text-text-primary transition-all duration-200">
                <Icon name="Heart" size={16} />
                <span>Favorites</span>
                {#if favoritesCount() > 0}
                  <Badge variant="primary" class="ml-auto">{favoritesCount()}</Badge>
                {/if}
              </a>
            </li>
            <li>
              <a href="/history" class="flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm text-text-secondary hover:bg-surface-hover hover:text-text-primary transition-all duration-200">
                <Icon name="History" size={16} />
                <span>History</span>
              </a>
            </li>
            <li>
              <a href="/settings" class="flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm text-text-secondary hover:bg-surface-hover hover:text-text-primary transition-all duration-200">
                <Icon name="Settings" size={16} />
                <span>Settings</span>
              </a>
            </li>
          </ul>
        </div>
      </div>

      <div class="border-t border-border px-4 py-3">
        <Button size="md" class="w-full" onclick={() => {}}>
          <Icon name="Plus" size={16} />
          Add Playlist
        </Button>
      </div>
    </div>
  </ResizablePane>
{/if}

{#if !visible}
  <button
    class="fixed left-0 top-4 z-40 rounded-r-xl glass glass-surface border border-border border-l-0 px-2 py-3 text-text-secondary hover:text-text-primary transition-all duration-200"
    onclick={() => (visible = true)}
    aria-label="Open sidebar"
  >
    <Icon name="Menu" size={20} />
  </button>
{/if}
