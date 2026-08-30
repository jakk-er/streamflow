<script lang="ts">
  import { channelStore, settingsStore, favoritesStore } from '$lib/stores';
  import ChannelCard from '$lib/components/channel/ChannelCard.svelte';
  import Skeleton from '$lib/components/ui/Skeleton.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
  import Icon from '$lib/components/ui/Icon.svelte';
  import type { Channel } from '$lib/types';

  let { onchannelclick }: { onchannelclick?: (channel: Channel) => void } = $props();

  let containerRef: HTMLDivElement | null = $state(null);
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  const ITEM_HEIGHT = 56;
  const BUFFER = 5;

  let items = $derived(channelStore.filteredChannels);
  let loading = $derived(channelStore.loading);
  let loadError = $derived(channelStore.error);
  let activeId = $derived(channelStore.activeChannel?.id ?? null);
  let showNumber = $derived(settingsStore.settings?.showChannelNumber ?? false);

  $effect(() => {
    if (containerRef) {
      viewportHeight = containerRef.clientHeight;
    }
  });

  let totalHeight = $derived(items.length * ITEM_HEIGHT);
  let startIndex = $derived(Math.max(0, Math.floor(scrollTop / ITEM_HEIGHT) - BUFFER));
  let endIndex = $derived(Math.min(items.length, Math.ceil((scrollTop + viewportHeight) / ITEM_HEIGHT) + BUFFER));
  let visibleItems = $derived(items.slice(startIndex, endIndex));
  let offsetY = $derived(startIndex * ITEM_HEIGHT);

  function handleScroll() {
    if (containerRef) {
      scrollTop = containerRef.scrollTop;
    }
  }

  function handleClick(channel: Channel) {
    channelStore.selectChannel(channel);
    onchannelclick?.(channel);
  }

  function scrollToActive() {
    if (!activeId || !containerRef) return;
    const index = items.findIndex(c => c.id === activeId);
    if (index >= 0) {
      const top = index * ITEM_HEIGHT;
      const bottom = top + ITEM_HEIGHT;
      const viewTop = containerRef.scrollTop;
      const viewBottom = viewTop + containerRef.clientHeight;
      if (top < viewTop || bottom > viewBottom) {
        containerRef.scrollTop = top - containerRef.clientHeight / 2 + ITEM_HEIGHT / 2;
      }
    }
  }

  $effect(() => {
    if (activeId) scrollToActive();
  });
</script>

<div
  bind:this={containerRef}
  data-channel-list
  class="h-full overflow-y-auto"
   onscroll={handleScroll}
  tabindex="0"
  role="listbox"
  aria-label="Channel list"
>
  {#if loading}
    <div class="space-y-1 p-2">
      {#each Array(6) as _}
        <Skeleton class="h-14 w-full" />
      {/each}
    </div>
  {:else if loadError}
    <EmptyState
      icon="AlertCircle"
      title="Failed to load channels"
      description={loadError}
    />
  {:else if items.length === 0}
    <EmptyState
      icon="Tv"
      title="No channels found"
      description="Try adjusting your search or select a different playlist."
    />
  {:else}
    <div style="height: {totalHeight}px; position: relative;">
      <div style="transform: translateY({offsetY}px);">
        {#each visibleItems as channel (channel.id)}
          <div style="height: {ITEM_HEIGHT}px;" role="option" aria-selected={channel.id === activeId}>
            <ChannelCard
              {channel}
              isSelected={channel.id === activeId}
              isFavorite={favoritesStore.isFavorite(channel.id)}
              onclick={() => handleClick(channel)}
            />
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>
