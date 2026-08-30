<script lang="ts">
  import { favoritesStore, settingsStore, playlistStore } from '$lib/stores';
  import type { Channel } from '$lib/types';
  import Icon from '$lib/components/ui/Icon.svelte';
  import Card from '$lib/components/ui/Card.svelte';
  import { isKnownBadLogoUrl } from '$lib/utils/brokenLogoHosts';

  let { channel, isSelected = false, isFavorite = false, onclick, class: className = '' }: { channel: Channel; isSelected?: boolean; isFavorite?: boolean; onclick?: () => void; class?: string } = $props();

  let logoFailed = $state(false);
  $effect(() => { channel.tvg?.logo; logoFailed = false; });
  let logoUsable = $derived(!!channel.tvg?.logo && !logoFailed && !isKnownBadLogoUrl(channel.tvg.logo));

  let showNumber = $derived(settingsStore.settings?.showChannelNumber ?? false);
  let coverSize = $derived((settingsStore.settings?.coverSize ?? 'medium').toLowerCase());

  let sizeClasses = $derived({
    small: 'w-8 h-8 text-xs',
    medium: 'w-10 h-10 text-sm',
    large: 'w-12 h-12 text-base',
  }[coverSize] ?? 'w-10 h-10 text-sm');

  function handleFavoriteClick(event: MouseEvent) {
    event.stopPropagation();
    favoritesStore.toggle(channel.id, playlistStore.activePlaylistId ?? '');
  }

  function handleFavoriteKeyDown(event: KeyboardEvent) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      event.stopPropagation();
      favoritesStore.toggle(channel.id, playlistStore.activePlaylistId ?? '');
    }
  }
</script>

<button
  class={`flex w-full items-center gap-3 px-3 py-2 text-left transition-all duration-150 ${className}`}
  onclick={onclick}
  role="option"
  aria-selected={isSelected}
>
  <div class={`flex-shrink-0 overflow-hidden rounded-lg bg-surface-elevated flex items-center justify-center text-text-secondary ${sizeClasses} ${isSelected ? 'ring-2 ring-primary ring-offset-2 ring-offset-background' : ''}`}>
    {#if logoUsable}
      <img src={channel.tvg.logo} alt="" class="h-full w-full object-cover" onerror={() => logoFailed = true} />
    {:else}
      <span class="font-medium">{channel.name.charAt(0).toUpperCase()}</span>
    {/if}
  </div>

  <div class="flex-1 min-w-0">
    <div class="flex items-center gap-2">
      {#if showNumber && channel.channelNumber}
        <span class="text-xs text-text-muted font-mono w-6">{channel.channelNumber}</span>
      {/if}
      <span class="truncate text-sm text-text-primary">{channel.name}</span>
    </div>
    {#if channel.group?.title && channel.group.title !== 'All Channels'}
      <span class="text-xs text-text-muted truncate">{channel.group.title}</span>
    {/if}
  </div>

  <div
    class={`flex-shrink-0 p-1 rounded-lg transition-all duration-150 ${isFavorite ? 'text-primary' : 'text-text-muted hover:text-text-primary hover:bg-surface-hover'}`}
    onclick={handleFavoriteClick}
    onkeydown={handleFavoriteKeyDown}
    role="button"
    tabindex="0"
    aria-label={isFavorite ? 'Remove from favorites' : 'Add to favorites'}
  >
    <Icon name="Star" size={16} class={isFavorite ? 'fill-current' : ''} />
  </div>
</button>
