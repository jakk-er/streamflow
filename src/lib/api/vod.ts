import { invoke } from '@tauri-apps/api/core';
import type { VodCatalogItem, VodContentType, VodLivePage, VodWatchProgress, XtreamCategory } from '$lib/types';

/**
 * Reads the VOD/series category list. Xtream: syncs once (if never synced)
 * then reads local cache. Stalker: always a live, remote-first fetch. See
 * `commands::vod::vod_get_categories` (Rust) for the full provider split.
 */
export async function vodGetCategories(playlistId: string, contentType: VodContentType): Promise<XtreamCategory[]> {
  return await invoke<XtreamCategory[]>('vod_get_categories', { playlistId, contentType });
}

/**
 * Reads the local VOD/series item cache (optionally scoped to one category).
 * Xtream: syncs once if never synced. Stalker: pure cache read, never
 * fetches - with no category selected this is a view over whatever's been
 * cached via `vodGetItemsLive`, not the full catalog.
 */
export async function vodGetItems(
  playlistId: string,
  contentType: VodContentType,
  categoryId?: string
): Promise<VodCatalogItem[]> {
  return await invoke<VodCatalogItem[]>('vod_get_items', { playlistId, contentType, categoryId });
}

/**
 * Dashboard "Trending" rail data - top-rated movies already cached locally
 * for this playlist (no TMDB or other external API). Never triggers a sync;
 * see `commands::vod::vod_get_top_rated` (Rust) for why.
 */
export async function vodGetTopRated(playlistId: string, limit: number): Promise<VodCatalogItem[]> {
  return await invoke<VodCatalogItem[]>('vod_get_top_rated', { playlistId, limit });
}

/**
 * Live, remote-first single-page fetch for one Stalker category - renders
 * immediately, no local sync/wait. Stalker only; opportunistically caches
 * what it fetches but never requires a prior sync. `search`, when set,
 * forwards as a title search instead of a category listing - pass
 * `categoryId: '*'` with it to search the whole catalog.
 */
export async function vodGetItemsLive(
  playlistId: string,
  contentType: VodContentType,
  categoryId: string,
  page: number,
  search?: string
): Promise<VodLivePage> {
  return await invoke<VodLivePage>('vod_get_items_live', { playlistId, contentType, categoryId, page, search });
}

/**
 * Pure local-cache read by item id, never a network call - fallback for a
 * Stalker item whose in-memory session cache (`stalkerRawItems`) is empty,
 * e.g. after an app restart. `null` means never cached by this playlist.
 */
export async function vodGetCachedItem(
  playlistId: string,
  contentType: VodContentType,
  itemId: string
): Promise<VodCatalogItem | null> {
  return await invoke<VodCatalogItem | null>('vod_get_cached_item', { playlistId, contentType, itemId });
}

/** Explicit resync - forces a fresh live fetch and replaces the local cache. */
export async function vodSync(playlistId: string): Promise<void> {
  return await invoke<void>('vod_sync', { playlistId });
}

/**
 * Saves playback position for a movie/episode - silently a no-op below the
 * 60s/2% threshold, when watch-history tracking is off, or for adult/18+/XXX
 * content (all enforced server-side, see `vod_save_progress` in Rust).
 */
export async function vodSaveProgress(params: {
  playlistId: string;
  contentType: VodContentType;
  vodItemId: string;
  episodeId?: string;
  seasonNumber?: number;
  episodeNumber?: number;
  episodeTitle?: string;
  positionSeconds: number;
  totalSeconds: number;
  title: string;
  cover?: string;
}): Promise<void> {
  return await invoke<void>('vod_save_progress', params);
}

/** Clears saved progress entirely - called on completion (a finished movie,
 * or a series with no next episode left), never just "marked done". */
export async function vodClearProgress(playlistId: string, contentType: VodContentType, vodItemId: string): Promise<void> {
  return await invoke<void>('vod_clear_progress', { playlistId, contentType, vodItemId });
}

export async function vodGetProgress(
  playlistId: string,
  contentType: VodContentType,
  vodItemId: string
): Promise<VodWatchProgress | null> {
  return await invoke<VodWatchProgress | null>('vod_get_progress', { playlistId, contentType, vodItemId });
}

/** One call for however many cards are currently rendered, not one round
 * trip per poster. */
export async function vodGetProgressBulk(
  playlistId: string,
  contentType: VodContentType,
  vodItemIds: string[]
): Promise<Record<string, VodWatchProgress>> {
  return await invoke<Record<string, VodWatchProgress>>('vod_get_progress_bulk', { playlistId, contentType, vodItemIds });
}

export async function vodGetContinueWatching(limit: number): Promise<VodWatchProgress[]> {
  return await invoke<VodWatchProgress[]>('vod_get_continue_watching', { limit });
}
