import { listen } from '@tauri-apps/api/event';
import type { VodItem, VodDetails, SeriesDetails, XtreamCategory, VodCatalogItem, StalkerContentItem } from '$lib/types';
import { formatError } from '$lib/utils/errors';
import { vodGetCategories, vodGetItems, vodGetItemsLive, vodGetCachedItem, stalkerGetVodInfo, stalkerGetSeriesInfo, xtreamGetVodInfo, xtreamGetSeriesInfo } from '$lib/api';
import { playlistStore } from './playlist.svelte';

/** Xtream and Stalker both come back through the same DB-shaped
 * `VodCatalogItem` response, so one mapper replaces the old per-provider ones. */
function mapVodCatalogItemToVodItem(item: VodCatalogItem): VodItem {
  return {
    id: item.id,
    name: item.name,
    streamType: item.contentType,
    containerExtension: item.containerExtension,
    cover: item.cover,
    rating: item.rating,
    genre: item.genre,
    releaseDate: item.releaseDate,
  };
}

function mapStalkerItemToVodItem(item: StalkerContentItem, contentType: 'vod' | 'series'): VodItem {
  return {
    id: item.id,
    name: item.name,
    streamType: contentType === 'series' || item.isSeries ? 'series' : 'movie',
    containerExtension: undefined,
    directSource: undefined,
    seriesId: undefined,
    cover: item.screenshotUri || item.cover,
    streamIcon: item.screenshotUri,
    streamUrl: undefined,
    rating: item.ratingImdb,
    genre: item.genresStr,
    releaseDate: item.year,
    seasonNumber: undefined,
    episodeNumber: undefined,
  };
}

function providerFor(playlistId: string): string | undefined {
  return playlistStore.playlists.find((p) => p._id === playlistId)?.playlistType;
}

// M3U has no separate VOD API - a movie is just a channel with a
// `group-title`, already browsable from Live TV. Without this check an M3U
// playlist fell into the Xtream branch and failed with a confusing "Missing
// server_url" error. Show a clear explanation instead, not a retryable failure.
const M3U_VOD_UNSUPPORTED_MESSAGE =
  'M3U playlists don’t have a separate movies/series catalog — browse them from Live TV instead.';

function createVodStore() {
  let vodItems = $state<VodItem[]>([]);
  let categories = $state<XtreamCategory[]>([]);
  let selectedCategoryId = $state<string | null>(null);
  let selectedType = $state<'movie' | 'series'>('movie');
  let loading = $state(false);
  let error = $state<string | null>(null);
  let currentDetail = $state<VodDetails | SeriesDetails | null>(null);
  let searchQuery = $state('');
  let sortBy = $state<'default' | 'name-az' | 'name-za' | 'year-desc' | 'year-asc'>('default');
  // The playlist whose catalog is currently held - see the 'vod-updated'
  // listener below.
  let loadedPlaylistId: string | null = null;

  // How far into the current category's grid the user has scrolled -
  // updated by `VodGrid`'s `onScrollTopChange`, read back as its
  // `initialScrollTop` when the route remounts (e.g. returning from a
  // detail page) with the SAME category still selected. `setCategory`
  // resets this to 0 on a genuine switch, since it only matters across a
  // detail-page round trip.
  let scrollPosition = $state(0);
  function setScrollPosition(pos: number) {
    scrollPosition = pos;
  }

  // Pagination state for a Stalker specific-category live browse
  // (`loadCategoryLive`/`loadMoreLive`) - meaningless for Xtream/M3U or the
  // "All Categories" cache view, which never paginate.
  let livePage = $state(1);
  let liveTotalPages = $state(1);
  let liveLoadingMore = $state(false);
  // Non-null while a Stalker live search (`searchLive`) is driving
  // `vodItems` instead of a plain category browse - `loadMoreLive` reads
  // this to know whether the next page continues the search (category '*')
  // or normal category listing. Reset to null by anything starting a
  // genuine category browse so a stale search can't keep steering pagination.
  let liveSearchQuery = $state<string | null>(null);
  let searchDebounceTimer: number | null = null;
  // Plain variable, not $state - compared inside async callbacks after an
  // await to check a resolved fetch is still relevant. Bumped by every
  // load*-style function so a newer selection can discard a stale
  // in-flight/queued fetch instead of waiting through it via `enqueue`'s
  // ordering alone.
  let loadGeneration = 0;

  // Stalker has no id-based "get one item" endpoint - the catalog row IS the
  // detail. Populated from the last `loadItems()` call's `stalkerItem` field
  // so `loadDetail()` can look it up instead of re-fetching.
  const stalkerRawItems = new Map<string, { item: StalkerContentItem; contentType: 'vod' | 'series' }>();

  // Serializes every backend call in this store (loadCategories, loadItems,
  // loadDetail) to one at a time, in request order. Replaces two separate
  // ad-hoc pending-slots that didn't cover each other's calls: a
  // `loadDetail` arriving mid-`loadItems` used to park and then never get
  // drained, silently vanishing ("Failed to load details" on a still-
  // settling list, or an empty category after switching Movies/Series). One
  // shared queue removes that mismatch by construction.
  let requestQueue: Promise<unknown> = Promise.resolve();
  function enqueue<T>(task: () => Promise<T>): Promise<T> {
    const run = requestQueue.then(task, task);
    requestQueue = run.then(
      () => undefined,
      () => undefined
    );
    return run;
  }

  // Stalker's censored-category recovery (background crawl of categories
  // excluded from the wildcard crawl, typically adult) appends items after
  // this store already loaded - re-read when it commits a batch for the
  // playlist/type currently on screen.
  listen<[string, string]>('vod-updated', (event) => {
    const [playlistId, contentType] = event.payload;
    const type = contentType === 'series' ? 'series' : 'movie';
    if (playlistId !== loadedPlaylistId || type !== selectedType) return;
    void loadItems(playlistId);
  }).catch(() => {
    // Not running under Tauri - the catalog simply doesn't live-update.
  });

  async function loadCategories(playlistId: string): Promise<void> {
    return enqueue(async () => {
    error = null;
    loading = true;
    try {
      const provider = providerFor(playlistId);
      if (provider === 'm3u') {
        categories = [];
        error = M3U_VOD_UNSUPPORTED_MESSAGE;
      } else {
        categories = await vodGetCategories(playlistId, selectedType);
      }
    } catch (err) {
      error = formatError(err);
      console.error('[vodStore] Failed to load categories:', error);
    } finally {
      loading = false;
    }
    });
  }

  async function loadItems(playlistId: string): Promise<void> {
    const myGeneration = ++loadGeneration;
    return enqueue(async () => {
    if (myGeneration !== loadGeneration) return; // superseded while queued
    error = null;
    loading = true;
    try {
      const provider = providerFor(playlistId);

      if (provider === 'm3u') {
        vodItems = [];
        error = M3U_VOD_UNSUPPORTED_MESSAGE;
      } else {
        const results = await vodGetItems(playlistId, selectedType, selectedCategoryId ?? undefined);
        if (myGeneration !== loadGeneration) return; // superseded while in flight
        loadedPlaylistId = playlistId;
        if (provider === 'stalker') {
          stalkerRawItems.clear();
          const contentType = selectedType === 'movie' ? 'vod' : 'series';
          for (const item of results) {
            if (item.stalkerItem) {
              stalkerRawItems.set(item.id, { item: item.stalkerItem, contentType });
            }
          }
        }
        vodItems = results.map(mapVodCatalogItemToVodItem);
      }
    } catch (err) {
      if (myGeneration !== loadGeneration) return;
      error = formatError(err);
      console.error('[vodStore] Failed to load items:', error);
    } finally {
      if (myGeneration === loadGeneration) loading = false;
    }
    });
  }

  // One page per batch - 2 caused duplicate-id crashes (`get_ordered_list`
  // is sorted by `added`; a category gaining items mid-fetch can shift
  // between two close-together page requests, returning the same item on
  // both pages). `fetchLiveBatch`'s id-dedup is a second safeguard.
  const PORTAL_PAGES_PER_BATCH = 1;

  /**
   * Fetches up to `PORTAL_PAGES_PER_BATCH` sequential pages starting after
   * `livePage`, updating `livePage`/`liveTotalPages` and stopping early if
   * the category is exhausted mid-batch. Shared by `loadCategoryLive`
   * (`mode: 'replace'`) and `loadMoreLive` (`mode: 'append'`).
   */
  async function fetchLiveBatch(
    playlistId: string,
    categoryId: string,
    myGeneration: number,
    mode: 'replace' | 'append',
    search?: string
  ): Promise<void> {
    const combined: VodCatalogItem[] = [];
    for (let i = 0; i < PORTAL_PAGES_PER_BATCH; i++) {
      const nextPage = livePage + 1;
      const result = await vodGetItemsLive(playlistId, selectedType, categoryId, nextPage, search);
      if (myGeneration !== loadGeneration) return; // superseded mid-batch
      combined.push(...result.items);
      livePage = result.page;
      liveTotalPages = result.totalPages;
      if (livePage >= liveTotalPages) break; // exhausted - don't request a page past the end
    }

    // Guards against the portal handing back an id we already have (see
    // `PORTAL_PAGES_PER_BATCH`). Seeded from the CURRENT `vodItems` on
    // append, not just this batch, since a shift can reintroduce an item
    // from an earlier batch too.
    const seenIds = new Set<string>(mode === 'append' ? vodItems.map((item) => item.id) : []);
    const deduped: VodCatalogItem[] = [];
    for (const item of combined) {
      if (seenIds.has(item.id)) continue;
      seenIds.add(item.id);
      deduped.push(item);
    }

    const contentType = selectedType === 'movie' ? 'vod' : 'series';
    if (mode === 'replace') stalkerRawItems.clear();
    for (const item of deduped) {
      if (item.stalkerItem) {
        stalkerRawItems.set(item.id, { item: item.stalkerItem, contentType });
      }
    }
    const mapped = deduped.map(mapVodCatalogItemToVodItem);
    vodItems = mode === 'replace' ? mapped : [...vodItems, ...mapped];
  }

  /**
   * Remote-first, paginated: fetches the first batch live from the portal
   * and replaces `vodItems`. Stalker + a specific category only - "All
   * Categories" and non-Stalker playlists still use `loadItems`'s cache read.
   */
  async function loadCategoryLive(playlistId: string, categoryId: string): Promise<void> {
    const myGeneration = ++loadGeneration;
    return enqueue(async () => {
      if (myGeneration !== loadGeneration) return;
      error = null;
      loading = true;
      liveSearchQuery = null;
      livePage = 0; // fetchLiveBatch starts at livePage + 1 = portal page 1
      liveTotalPages = 1;
      try {
        await fetchLiveBatch(playlistId, categoryId, myGeneration, 'replace');
        if (myGeneration !== loadGeneration) return;
        loadedPlaylistId = playlistId;
      } catch (err) {
        if (myGeneration !== loadGeneration) return;
        error = formatError(err);
        console.error('[vodStore] Failed to load category (live):', error);
      } finally {
        if (myGeneration === loadGeneration) loading = false;
      }
    });
  }

  /**
   * Portal-wide (`category: '*'`) live title search for Stalker, paginated
   * like `loadCategoryLive`. `filteredItems`' client-side filter still
   * applies on top, so a portal that ignores `search` just degrades to
   * "search only what's fetched so far" rather than wrong results.
   */
  async function searchLive(playlistId: string, query: string): Promise<void> {
    const myGeneration = ++loadGeneration;
    return enqueue(async () => {
      if (myGeneration !== loadGeneration) return;
      error = null;
      loading = true;
      liveSearchQuery = query;
      livePage = 0;
      liveTotalPages = 1;
      try {
        await fetchLiveBatch(playlistId, '*', myGeneration, 'replace', query);
        if (myGeneration !== loadGeneration) return;
        loadedPlaylistId = playlistId;
      } catch (err) {
        if (myGeneration !== loadGeneration) return;
        error = formatError(err);
        console.error('[vodStore] Failed to search (live):', error);
      } finally {
        if (myGeneration === loadGeneration) loading = false;
      }
    });
  }

  /**
   * Appends the next batch to `vodItems` - called by `VodGrid` on its
   * near-bottom scroll trigger and proactively when loaded content doesn't
   * fill the viewport (see its fill-viewport effect).
   *
   * `liveLoadingMore` is set synchronously here, before `enqueue`, so a
   * rapid double-call (two scroll ticks, or scroll + fill-viewport both
   * firing) actually collides with the guard at the top of this function -
   * setting it only once the queued task starts would let both calls see
   * `false` and each enqueue a duplicate fetch.
   */
  async function loadMoreLive(playlistId: string, categoryId: string): Promise<void> {
    // Remote-first pagination only exists for Stalker - `livePage`/
    // `liveTotalPages` reset to 0/1 on category switch but stay unset for
    // Xtream/M3U, so the page-counter check alone (0 >= 1 is false) never
    // blocks those providers. `VodGrid`'s fill-viewport effect calls this
    // speculatively without knowing the provider (a smaller Series catalog
    // can trigger it even for Xtream), so this explicit check is what
    // actually stops it.
    if (providerFor(playlistId) !== 'stalker') return;
    if (liveLoadingMore || livePage >= liveTotalPages) return;
    liveLoadingMore = true;
    const myGeneration = loadGeneration; // continues the current selection, doesn't start a new one
    // A live search in progress owns `vodItems`/pagination, not whatever
    // category the UI has selected underneath it - continue THAT search's
    // next page rather than silently switching back to category browsing.
    const search = liveSearchQuery;
    const effectiveCategoryId = search !== null ? '*' : categoryId;
    return enqueue(async () => {
      if (myGeneration !== loadGeneration) {
        liveLoadingMore = false;
        return;
      }
      try {
        await fetchLiveBatch(playlistId, effectiveCategoryId, myGeneration, 'append', search ?? undefined);
      } catch (err) {
        if (myGeneration !== loadGeneration) return;
        error = formatError(err);
        console.error('[vodStore] Failed to load more (live):', error);
      } finally {
        if (myGeneration === loadGeneration) liveLoadingMore = false;
      }
    });
  }

  async function loadDetail(playlistId: string, id: string, type: 'movie' | 'series'): Promise<void> {
    // The detail page calls this from both `onMount` and a `$effect`, which
    // can fire close together on navigation. `enqueue` makes this wait its
    // turn - notably, if `loadItems` is still filling `stalkerRawItems`,
    // this now waits for it instead of running against a stale cache.
    return enqueue(async () => {
    error = null;
    loading = true;
    // Cleared up front, not just on failure: otherwise the previous title's
    // detail stays rendered under the NEW `type` prop until this resolves -
    // movie-to-series briefly showed a mismatched object under
    // `type==='series'` (see `VodDetail.svelte`'s `isSeries` guard).
    currentDetail = null;
    try {
      const provider = providerFor(playlistId);

      if (provider === 'stalker') {
        let cached = stalkerRawItems.get(id);
        if (!cached) {
          // Not in this session's in-memory cache (e.g. a fresh app launch,
          // before re-browsing to wherever this item lives) - fall back to
          // the persisted DB cache, which survives restarts. This is what
          // makes "Resume" from Continue Watching work right after opening
          // the app, not just after re-browsing to its category.
          const dbItem = await vodGetCachedItem(playlistId, type, id);
          if (dbItem?.stalkerItem) {
            cached = { item: dbItem.stalkerItem, contentType: dbItem.contentType === 'series' ? 'series' : 'vod' };
            stalkerRawItems.set(id, cached);
          }
        }
        if (!cached) {
          error = 'Open this title from the list to view details.';
          currentDetail = null;
          return;
        }
        const isSeries = cached.contentType === 'series' || cached.item.isSeries;
        currentDetail = isSeries
          ? await stalkerGetSeriesInfo(playlistId, cached.contentType, cached.item)
          : await stalkerGetVodInfo(playlistId, cached.contentType, cached.item);
      } else if (provider === 'm3u') {
        error = M3U_VOD_UNSUPPORTED_MESSAGE;
        currentDetail = null;
      } else if (type === 'movie') {
        currentDetail = await xtreamGetVodInfo(playlistId, id);
      } else {
        currentDetail = await xtreamGetSeriesInfo(playlistId, id);
      }
    } catch (err) {
      error = formatError(err);
      console.error('[vodStore] Failed to load detail:', error);
    } finally {
      loading = false;
    }
    });
  }

  // `setType`/`setCategory` trigger their own reload explicitly rather than
  // relying on a caller's `$effect` - an effect calling `loadItems` would
  // also pick up this store's own loading/error/page writes as accidental
  // dependencies (Svelte tracks every reactive read in an effect's call
  // stack), causing a read-write feedback loop that fired duplicate network
  // requests. Calling `loadItems` directly keeps this store the single owner
  // of when it reloads.
  function setType(playlistId: string | undefined, type: 'movie' | 'series') {
    selectedType = type;
    vodItems = [];
    categories = [];
    selectedCategoryId = null;
    searchQuery = '';
    liveSearchQuery = null;
    if (searchDebounceTimer !== null) {
      clearTimeout(searchDebounceTimer);
      searchDebounceTimer = null;
    }
    livePage = 0;
    liveTotalPages = 1;
    if (playlistId) {
      // Sequential, not concurrent - avoids the same two-requests collision
      // this store's queuing exists to prevent.
      void (async () => {
        await loadCategories(playlistId);
        // No client-side "All Categories" any more - the provider's own
        // category list is authoritative. Auto-select the first one to
        // avoid landing on a blank "pick a category" screen.
        setCategory(playlistId, categories[0]?.categoryId ?? null);
      })();
    }
  }

  function setCategory(playlistId: string | undefined, id: string | null) {
    selectedCategoryId = id;
    vodItems = [];
    livePage = 0;
    liveTotalPages = 1;
    // A pending debounced search must not fire AFTER this switch and
    // silently overwrite what was just loaded - without this, switching
    // category within `SEARCH_DEBOUNCE_MS` of typing left the stale search
    // armed, snapping the view back a moment later.
    if (searchDebounceTimer !== null) {
      clearTimeout(searchDebounceTimer);
      searchDebounceTimer = null;
    }
    // A genuine category switch always starts at the top - `scrollPosition`
    // surviving is only meaningful for "returned from detail page to the
    // SAME category", which never calls `setCategory`.
    scrollPosition = 0;
    if (!playlistId || id === null) return;
    if (providerFor(playlistId) === 'stalker') {
      void loadCategoryLive(playlistId, id);
    } else {
      void loadItems(playlistId); // Xtream: local-cache read for this category
    }
  }

  /** Re-runs whatever fetch fits the current selection, for the toolbar's
   * "Retry" action - a Stalker retry re-fetches live rather than falling
   * back to `loadItems`'s cache read (stale/incomplete by design there). */
  function retry(playlistId: string) {
    if (providerFor(playlistId) === 'stalker' && selectedCategoryId !== null) {
      void loadCategoryLive(playlistId, selectedCategoryId);
    } else {
      void loadItems(playlistId);
    }
  }

  function itemYear(item: VodItem): number {
    if (!item.releaseDate) return 0;
    const parsed = new Date(item.releaseDate).getFullYear();
    return Number.isNaN(parsed) ? 0 : parsed;
  }

  // Client-side only, over whatever's currently in `vodItems` - correct and
  // instant whether that's a full local-cache read or just the pages
  // fetched so far of a live-paginated Stalker category.
  const filteredItems = $derived(() => {
    let result = vodItems;
    const query = searchQuery.trim().toLowerCase();
    if (query) {
      result = result.filter((item) => item.name.toLowerCase().includes(query));
    }
    switch (sortBy) {
      case 'name-az':
        result = [...result].sort((a, b) => a.name.localeCompare(b.name));
        break;
      case 'name-za':
        result = [...result].sort((a, b) => b.name.localeCompare(a.name));
        break;
      case 'year-desc':
        result = [...result].sort((a, b) => itemYear(b) - itemYear(a));
        break;
      case 'year-asc':
        result = [...result].sort((a, b) => itemYear(a) - itemYear(b));
        break;
      default:
        break;
    }
    return result;
  });

  const SEARCH_DEBOUNCE_MS = 350;

  /**
   * `searchQuery` updates immediately (input and the client-side filter stay
   * responsive every keystroke), but for Stalker this also debounces a live
   * portal search (`searchLive`) - needed because browsing there is
   * lazy/paginated, so a title outside what's fetched so far otherwise never
   * shows up. Xtream/M3U's catalog is already fully local, so the immediate
   * client-side filter alone is enough.
   */
  function setSearch(query: string) {
    searchQuery = query;

    const playlistId = playlistStore.activePlaylistId;
    if (!playlistId || providerFor(playlistId) !== 'stalker') return;

    if (searchDebounceTimer !== null) clearTimeout(searchDebounceTimer);
    const trimmed = query.trim();
    searchDebounceTimer = window.setTimeout(() => {
      searchDebounceTimer = null;
      if (trimmed) {
        void searchLive(playlistId, trimmed);
      } else if (selectedCategoryId) {
        // Search cleared - back to plainly browsing whatever category is
        // selected (`loadCategoryLive` resets `liveSearchQuery` itself).
        void loadCategoryLive(playlistId, selectedCategoryId);
      }
    }, SEARCH_DEBOUNCE_MS);
  }

  function setSort(sort: typeof sortBy) {
    sortBy = sort;
  }

  return {
    get vodItems() { return vodItems; },
    get categories() { return categories; },
    get selectedCategoryId() { return selectedCategoryId; },
    get selectedType() { return selectedType; },
    get loading() { return loading; },
    get error() { return error; },
    get currentDetail() { return currentDetail; },
    get filteredItems() { return filteredItems(); },
    get searchQuery() { return searchQuery; },
    get sortBy() { return sortBy; },
    get livePage() { return livePage; },
    get liveTotalPages() { return liveTotalPages; },
    get liveLoadingMore() { return liveLoadingMore; },
    get scrollPosition() { return scrollPosition; },
    get loadedPlaylistId() { return loadedPlaylistId; },
    loadCategories,
    loadItems,
    loadDetail,
    loadMoreLive,
    retry,
    setType,
    setCategory,
    setSearch,
    setSort,
    setScrollPosition,
  };
}

export const vodStore = createVodStore();
