<script lang="ts">
  import { onMount } from 'svelte';
  import type { VodItem, VodContentType, VodWatchProgress } from '$lib/types';
  import { vodGetProgressBulk } from '$lib/api';
  import VodCard from '$lib/components/vod/VodCard.svelte';

  // Default target: 7 columns in a full-screen window. The toolbar's
  // column-count control uses this same literal as its own default.
  const DEFAULT_PREFERRED_COLUMNS = 7;
  // Floor width below which a poster card looks cramped - keeps the grid
  // responsive to window width regardless of `preferredColumns`.
  const MIN_CARD_WIDTH_PX = 140;

  let {
    items = [],
    onLoadMore,
    preferredColumns = DEFAULT_PREFERRED_COLUMNS,
    initialScrollTop = 0,
    onScrollTopChange,
    playlistId,
    contentType,
  }: {
    items?: VodItem[];
    onLoadMore?: () => void;
    preferredColumns?: number;
    /** Applied once, the first time a real width measurement and at least
     * one item are available - restoring a raw pixel offset before row
     * height is known would land in the wrong place. Not bindable
     * deliberately: the caller owns this value and only needs the current
     * value back via `onScrollTopChange`. */
    initialScrollTop?: number;
    onScrollTopChange?: (scrollTop: number) => void;
    /** Both needed together to fetch progress bars - omitted entirely just
     * means no card shows a progress bar. */
    playlistId?: string;
    contentType?: VodContentType;
  } = $props();

  // Bulk-fetched for the whole loaded `items` batch (not just the windowed
  // subset) - one query per batch, re-run on every page appended. Simple and
  // cheap enough at typical VOD page sizes (14-28 items) to not bother
  // diffing just the newly added ids.
  let progressByItemId = $state<Map<string, VodWatchProgress>>(new Map());
  $effect(() => {
    const ids = items.map((item) => item.id);
    if (!playlistId || !contentType || ids.length === 0) {
      progressByItemId = new Map();
      return;
    }
    vodGetProgressBulk(playlistId, contentType, ids)
      .then((result) => {
        progressByItemId = new Map(Object.entries(result));
      })
      .catch((err) => {
        console.error('Failed to load VOD watch progress:', err);
      });
  });

  // Fires on every scroll tick within this distance of the bottom -
  // deliberately not debounced, since `vodStore.loadMoreLive` already no-ops
  // while a fetch is in flight or the category is exhausted.
  const LOAD_MORE_THRESHOLD_PX = 800;

  // Manual scroll-position virtualization (row-based, adapted from
  // `ChannelList.svelte`'s single-column version). Needed because
  // `vodStore` can hand back a provider's entire catalog - tens of thousands
  // of titles for a large reseller package - and mounting that many real
  // `<VodCard>` nodes at once previously crashed the app. Only rows near the
  // viewport are rendered.
  let containerRef: HTMLDivElement | null = $state(null);
  let scrollTop = $state(0);
  let containerWidth = $state(0);
  let viewportHeight = $state(0);

  const GAP = 16; // Tailwind `gap-4`
  // `VodCard`'s text block below the cover: mt-2 (8px) + title line (~20px)
  // + mt-1 (4px) + genre line (~16px, reserved even when absent so row
  // height stays constant regardless of a card's content).
  const TEXT_BLOCK_HEIGHT = 48;
  const BUFFER_ROWS = 2;

  // Hard ceiling on columns that fit the measured width at `MIN_CARD_WIDTH_PX`
  // - `columnCount` never exceeds this even when `preferredColumns` is
  // higher, which is what keeps the grid responsive as the window narrows.
  let maxColumnsForWidth = $derived(
    containerWidth > 0 ? Math.max(2, Math.floor((containerWidth + GAP) / (MIN_CARD_WIDTH_PX + GAP))) : 2
  );
  let columnCount = $derived(Math.max(2, Math.min(preferredColumns, maxColumnsForWidth)));
  let cardWidth = $derived(columnCount > 0 ? (containerWidth - GAP * (columnCount - 1)) / columnCount : 0);
  let rowHeight = $derived(cardWidth * 1.5 + TEXT_BLOCK_HEIGHT + GAP);
  let rowCount = $derived(columnCount > 0 ? Math.ceil(items.length / columnCount) : 0);
  let totalHeight = $derived(rowCount * rowHeight);
  let startRow = $derived(rowHeight > 0 ? Math.max(0, Math.floor(scrollTop / rowHeight) - BUFFER_ROWS) : 0);
  let endRow = $derived(
    rowHeight > 0 ? Math.min(rowCount, Math.ceil((scrollTop + viewportHeight) / rowHeight) + BUFFER_ROWS) : 0
  );
  let visibleItems = $derived(items.slice(startRow * columnCount, endRow * columnCount));
  let offsetY = $derived(startRow * rowHeight);

  function handleScroll() {
    if (!containerRef) return;
    scrollTop = containerRef.scrollTop;
    onScrollTopChange?.(scrollTop);
    const distanceToBottom = containerRef.scrollHeight - containerRef.scrollTop - containerRef.clientHeight;
    if (distanceToBottom < LOAD_MORE_THRESHOLD_PX) {
      onLoadMore?.();
    }
  }

  // Restores a saved scroll position once per mount, the first moment a real
  // width measurement and actual rows exist to scroll against.
  // `initialScrollTop` is 0 on a genuine category switch and only non-zero
  // when returning to the same category (see its doc comment).
  let hasRestoredScroll = false;
  $effect(() => {
    if (hasRestoredScroll || !containerRef || containerWidth <= 0 || items.length === 0) return;
    hasRestoredScroll = true;
    if (initialScrollTop > 0) {
      containerRef.scrollTop = initialScrollTop;
      scrollTop = initialScrollTop;
    }
  });

  // A `scroll` event only fires once content overflows the container - with
  // as few as ~14-28 items that's often shorter than the viewport, so
  // `handleScroll` would never run and the list would stall. Keeps
  // requesting more until content overflows or `onLoadMore`'s own guards
  // report the category exhausted (a safe no-op call otherwise).
  $effect(() => {
    if (items.length > 0 && viewportHeight > 0 && totalHeight <= viewportHeight) {
      onLoadMore?.();
    }
  });

  onMount(() => {
    if (!containerRef) return;
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      containerWidth = entry.contentRect.width;
      viewportHeight = entry.contentRect.height;
    });
    observer.observe(containerRef);
    return () => observer.disconnect();
  });
</script>

<div bind:this={containerRef} class="h-full w-full overflow-y-auto p-6" onscroll={handleScroll}>
  <!-- `containerWidth` is 0 until the ResizeObserver's first callback lands,
       during which `columnCount` would fall back to a wrong guess (2) -
       painting at that guess then snapping to the real count caused every
       poster to visibly reflow at once (mainly visible on Xtream, whose
       fast cache read can beat the first measurement). Wait for a real
       measurement before painting anything. -->
  {#if containerWidth > 0}
    <div style="height: {totalHeight}px; position: relative;">
      <div
        class="grid gap-4"
        style="transform: translateY({offsetY}px); grid-template-columns: repeat({columnCount}, minmax(0, 1fr));"
      >
        {#each visibleItems as item (item.id)}
          <VodCard {item} progress={progressByItemId.get(item.id)} />
        {/each}
      </div>
    </div>
  {/if}
</div>
