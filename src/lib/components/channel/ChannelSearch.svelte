<script lang="ts">
  import { channelStore, favoritesStore, playlistStore, uiPrefsStore } from '$lib/stores';
  import Icon from '$lib/components/ui/Icon.svelte';
  import Input from '$lib/components/ui/Input.svelte';
  import MarqueeSelect from '$lib/components/ui/MarqueeSelect.svelte';

  function handleSearchInput(event: Event) {
    const target = event.target as HTMLInputElement;
    channelStore.search(target.value, playlistStore.activePlaylistId ?? undefined);
  }

  let debounceTimer: number | null = null;
  function handleSearchDebounce(event: Event) {
    const target = event.target as HTMLInputElement;
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = window.setTimeout(() => {
      channelStore.search(target.value, playlistStore.activePlaylistId ?? undefined);
    }, 300);
  }

  function handleGroupChange(newValue: string) {
    channelStore.setGroupFilter(newValue === 'all' ? null : newValue);
  }

  let groupOptions = $derived([
    { value: 'all', label: 'All Groups' },
    ...channelStore.groups.map((group) => ({ value: group, label: group })),
  ]);

  async function handleToggleFavorite() {
    const active = channelStore.activeChannel;
    if (!active) return;
    const playlistId = playlistStore.activePlaylistId;
    if (!playlistId) return;
    await favoritesStore.toggle(active.id, playlistId);
  }

  function clearSearch() {
    channelStore.search('', playlistStore.activePlaylistId ?? undefined);
  }

  let isFav = $derived(channelStore.activeChannel ? favoritesStore.isFavorite(channelStore.activeChannel.id) : false);
</script>

<div class="border-b border-border p-3 space-y-2">
  <div class="relative">
    <div class="absolute left-3 top-2.5 text-text-muted">
      <Icon name="Search" size={16} />
    </div>
    <Input
      type="text"
      placeholder="Search channels..."
      value={channelStore.searchQuery}
      oninput={handleSearchDebounce}
      class="pl-9 pr-9"
    />
    {#if channelStore.searchQuery}
      <button
        class="absolute right-2 top-2 rounded p-1 text-text-muted hover:text-text-primary transition-all duration-200"
        onclick={clearSearch}
        aria-label="Clear search"
      >
        <Icon name="X" size={14} />
      </button>
    {/if}
  </div>

  <div class="flex items-center gap-2">
    {#if uiPrefsStore.browseLayout === 'list'}
      <MarqueeSelect
        class="flex-1"
        options={groupOptions}
        value={channelStore.selectedGroup ?? 'all'}
        onchange={handleGroupChange}
        ariaLabel="Filter by group"
      />
    {/if}

    <div class="ml-auto flex items-center rounded-lg border border-border p-0.5">
      <button
        class="rounded-md p-1.5 transition-all duration-200 {uiPrefsStore.browseLayout === 'list'
          ? 'bg-surface-elevated text-text-primary'
          : 'text-text-muted hover:text-text-primary'}"
        onclick={() => uiPrefsStore.setBrowseLayout('list')}
        aria-label="List view with group filter"
        aria-pressed={uiPrefsStore.browseLayout === 'list'}
        title="List view"
      >
        <Icon name="LayoutList" size={16} />
      </button>
      <button
        class="rounded-md p-1.5 transition-all duration-200 {uiPrefsStore.browseLayout === 'categories'
          ? 'bg-surface-elevated text-text-primary'
          : 'text-text-muted hover:text-text-primary'}"
        onclick={() => uiPrefsStore.setBrowseLayout('categories')}
        aria-label="Browse by category"
        aria-pressed={uiPrefsStore.browseLayout === 'categories'}
        title="Category view"
      >
        <Icon name="LayoutGrid" size={16} />
      </button>
    </div>

    <button
      class="rounded-lg p-2 transition-all duration-200 {channelStore.sortBy === 'none'
        ? 'text-text-secondary hover:text-text-primary hover:bg-surface-hover'
        : 'text-primary hover:bg-surface-hover'}"
      onclick={channelStore.toggleNameSort}
      aria-label={channelStore.sortBy === 'name-desc' ? 'Sorted Z to A - click to sort A to Z' : 'Sort by name A to Z'}
      title={channelStore.sortBy === 'name-desc' ? 'Sorted Z → A' : channelStore.sortBy === 'name-asc' ? 'Sorted A → Z' : 'Sort by name'}
    >
      <Icon name={channelStore.sortBy === 'name-desc' ? 'ArrowDownAZ' : 'ArrowUpAZ'} size={18} />
    </button>

    <button
      class="rounded-lg p-2 text-text-secondary hover:text-text-primary hover:bg-surface-hover transition-all duration-200 {isFav ? 'text-primary' : ''}"
      onclick={handleToggleFavorite}
      aria-label="Toggle favorite"
      disabled={!channelStore.activeChannel}
    >
      <Icon name="Star" size={18} class={isFav ? 'fill-current' : ''} />
    </button>
  </div>
</div>
