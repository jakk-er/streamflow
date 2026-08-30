import { invoke } from '@tauri-apps/api/core';
import type { FavoriteChannel, WatchHistoryItem } from '$lib/types';

/**
 * Toggle a channel's favorite status.
 * @param channelId - The UUID of the channel
 * @param playlistId - The UUID of the playlist
 * @param favoriteType - The type of favorite ('channel' or 'global')
 * @returns True if the channel is now favorited, false if unfavorited
 */
export async function toggleFavorite(
  channelId: string,
  playlistId: string,
  favoriteType: string
): Promise<boolean> {
  return await invoke<boolean>('toggle_favorite', {
    channelId,
    playlistId,
    favoriteType,
  });
}

/**
 * Get all favorite channels.
 * @param playlistId - Optional playlist ID to filter favorites by
 * @returns Array of FavoriteChannel objects
 */
export async function getFavorites(playlistId?: string): Promise<FavoriteChannel[]> {
  return await invoke<FavoriteChannel[]>('get_favorites', { playlistId });
}

/**
 * Persist a user drag-reorder of a playlist's favorites.
 * @param playlistId - The UUID of the playlist
 * @param orderedChannelIds - Favorited channel IDs in their new front-to-back order
 */
export async function reorderFavorites(playlistId: string, orderedChannelIds: string[]): Promise<void> {
  await invoke<void>('reorder_favorites', {
    playlistId,
    orderedChannelIds,
  });
}

/**
 * Get recently watched channels.
 * @param limit - Maximum number of items to return
 * @returns Array of WatchHistoryItem objects
 */
export async function getRecentlyWatched(limit: number): Promise<WatchHistoryItem[]> {
  return await invoke<WatchHistoryItem[]>('get_recently_watched', { limit });
}

/**
 * Save the playback position for a channel.
 * @param channelId - The UUID of the channel
 * @param playlistId - The UUID of the playlist
 * @param positionSeconds - The current playback position in seconds
 * @param totalSeconds - The total duration of the content in seconds
 */
export async function savePlaybackPosition(
  channelId: string,
  playlistId: string,
  positionSeconds: number,
  totalSeconds: number
): Promise<void> {
  await invoke<void>('save_playback_position', {
    channelId,
    playlistId,
    positionSeconds,
    totalSeconds,
  });
}

/**
 * Remove a single entry from watch history.
 * @param id - The UUID of the watch history row
 */
export async function removeWatchHistoryItem(id: string): Promise<void> {
  await invoke<void>('remove_watch_history_item', { id });
}

/**
 * Permanently delete all watch history entries.
 */
export async function clearWatchHistory(): Promise<void> {
  await invoke<void>('clear_watch_history');
}
