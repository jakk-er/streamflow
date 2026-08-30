<script lang="ts">
  import { channelStore } from '$lib/stores';
  import Skeleton from '$lib/components/ui/Skeleton.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
  import Icon from '$lib/components/ui/Icon.svelte';

  let { onselect }: { onselect?: (group: string) => void } = $props();

  let loading = $derived(channelStore.loading);
  let loadError = $derived(channelStore.error);

  let categories = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const channel of channelStore.channels) {
      const title = channel.group?.title;
      if (!title) continue;
      counts.set(title, (counts.get(title) ?? 0) + 1);
    }
    return channelStore.groups.map((group) => ({ name: group, count: counts.get(group) ?? 0 }));
  });
</script>

<div
  data-channel-list
  class="h-full overflow-y-auto outline-none"
  tabindex="0"
  role="listbox"
  aria-label="Channel categories"
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
  {:else if categories.length === 0}
    <EmptyState
      icon="Tv"
      title="No categories found"
      description="Try selecting a different playlist."
    />
  {:else}
    {#each categories as category (category.name)}
      <button
        class="flex w-full items-center gap-3 px-3 py-2 text-left transition-all duration-150 hover:bg-surface-hover"
        onclick={() => onselect?.(category.name)}
        role="option"
        aria-selected="false"
      >
        <div class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-lg bg-surface-elevated text-text-secondary">
          <Icon name="Folder" size={18} />
        </div>
        <div class="min-w-0 flex-1">
          <div class="truncate text-sm text-text-primary">{category.name}</div>
          <div class="text-xs text-text-muted">{category.count} {category.count === 1 ? 'channel' : 'channels'}</div>
        </div>
        <Icon name="ChevronRight" size={16} class="flex-shrink-0 text-text-muted" />
      </button>
    {/each}
  {/if}
</div>
