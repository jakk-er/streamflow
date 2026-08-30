import { toggleFavorite, getFavorites, getRecentlyWatched, savePlaybackPosition, removeWatchHistoryItem, clearWatchHistory } from '$lib/api';
import type { FavoriteChannel, WatchHistoryItem } from '$lib/types';
import { formatError } from '$lib/utils/errors';

function createFavoritesStore() {
  let favorites = $state<FavoriteChannel[]>([]);
  let recentlyWatched = $state<WatchHistoryItem[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);

  async function loadFavorites(playlistId?: string) {
    error = null;
    loading = true;
    try {
      favorites = await getFavorites(playlistId);
    } catch (err) {
      error = formatError(err);
    } finally {
      loading = false;
    }
  }

  async function toggle(channelId: string, playlistId: string, favoriteType?: string) {
    error = null;
    loading = true;
    try {
      await toggleFavorite(channelId, playlistId, favoriteType ?? 'channel');
      await loadFavorites();
    } catch (err) {
      error = formatError(err);
      throw err;
    } finally {
      loading = false;
    }
  }

  async function loadRecentlyWatched(limit: number) {
    error = null;
    loading = true;
    try {
      recentlyWatched = await getRecentlyWatched(limit);
    } catch (err) {
      error = formatError(err);
    } finally {
      loading = false;
    }
  }

  async function savePosition(channelId: string, playlistId: string, positionSeconds: number, totalSeconds: number) {
    error = null;
    loading = true;
    try {
      await savePlaybackPosition(channelId, playlistId, positionSeconds, totalSeconds);
    } catch (err) {
      error = formatError(err);
      throw err;
    } finally {
      loading = false;
    }
  }

  async function removeHistoryItem(id: string) {
    error = null;
    try {
      await removeWatchHistoryItem(id);
      recentlyWatched = recentlyWatched.filter(item => item.id !== id);
    } catch (err) {
      error = formatError(err);
      throw err;
    }
  }

  async function clearHistory() {
    error = null;
    try {
      await clearWatchHistory();
      recentlyWatched = [];
    } catch (err) {
      error = formatError(err);
      throw err;
    }
  }

  const isFavorite = (channelId: string) => favorites.some(f => f.channelId === channelId);

  return {
    get favorites() { return favorites; },
    get recentlyWatched() { return recentlyWatched; },
    get loading() { return loading; },
    get error() { return error; },
    get isFavorite() { return isFavorite; },
    loadFavorites,
    toggle,
    loadRecentlyWatched,
    savePosition,
    removeHistoryItem,
    clearHistory,
  };
}

export const favoritesStore = createFavoritesStore();
