import {
  getPlaylists,
  importM3uPlaylist,
  updateM3uPlaylist,
  addXtreamPlaylist,
  updateXtreamPlaylist,
  addStalkerPlaylist,
  updateStalkerPlaylist,
  deletePlaylist,
  refreshPlaylist,
} from '$lib/api';
import type { StalkerAddResult } from '$lib/api/playlist';
import type { Playlist } from '$lib/types';
import { formatError } from '$lib/utils/errors';

function createPlaylistStore() {
  let playlists = $state<Playlist[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let activePlaylistId = $state<string | null>(null);

  async function loadPlaylists() {
    error = null;
    loading = true;
    try {
      playlists = await getPlaylists();
    } catch (err) {
      error = formatError(err);
    } finally {
      loading = false;
    }
  }

  async function importM3u(url: string, title: string, userAgent?: string, autoRefresh?: boolean) {
    error = null;
    loading = true;
    try {
      await importM3uPlaylist(url, title, userAgent, autoRefresh);
      await loadPlaylists();
    } catch (err) {
      error = formatError(err);
      throw err;
    } finally {
      loading = false;
    }
  }

  async function updateM3u(id: string, url: string, title: string, userAgent?: string, autoRefresh?: boolean) {
    error = null;
    loading = true;
    try {
      await updateM3uPlaylist(id, url, title, userAgent, autoRefresh);
      await loadPlaylists();
    } catch (err) {
      error = formatError(err);
      throw err;
    } finally {
      loading = false;
    }
  }

  async function addXtream(
    baseUrl: string,
    username: string,
    password: string,
    title: string,
    userAgent?: string,
    autoRefresh?: boolean
  ) {
    error = null;
    loading = true;
    try {
      await addXtreamPlaylist(baseUrl, username, password, title, userAgent, autoRefresh);
      await loadPlaylists();
    } catch (err) {
      error = formatError(err);
      throw err;
    } finally {
      loading = false;
    }
  }

  async function updateXtream(
    id: string,
    baseUrl: string,
    username: string,
    password: string,
    title: string,
    userAgent?: string,
    autoRefresh?: boolean
  ) {
    error = null;
    loading = true;
    try {
      await updateXtreamPlaylist(id, baseUrl, username, password, title, userAgent, autoRefresh);
      await loadPlaylists();
    } catch (err) {
      error = formatError(err);
      throw err;
    } finally {
      loading = false;
    }
  }

  /**
   * Returns the full add result (not just the playlist) so the caller can
   * branch on `outcome.kind === 'loginRequired'` and show `StalkerLoginForm`
   * instead of treating it as a failure - the row is created either way.
   */
  async function addStalker(
    serverUrl: string,
    macAddress: string,
    title: string,
    userAgent?: string,
    username?: string,
    password?: string,
    deviceId1?: string,
    deviceId2?: string,
    serialNumber?: string,
    signature1?: string,
    signature2?: string,
    autoRefresh?: boolean
  ): Promise<StalkerAddResult> {
    error = null;
    loading = true;
    try {
      const result = await addStalkerPlaylist(
        serverUrl,
        macAddress,
        title,
        userAgent,
        username,
        password,
        deviceId1,
        deviceId2,
        serialNumber,
        signature1,
        signature2,
        autoRefresh
      );
      await loadPlaylists();
      return result;
    } catch (err) {
      error = formatError(err);
      console.error('[playlistStore] Failed to add Stalker playlist:', error);
      throw err;
    } finally {
      loading = false;
    }
  }

  /**
   * Mirrors `addStalker`'s `loginRequired`-outcome handling exactly, against
   * an existing playlist id instead of creating a new row.
   */
  async function updateStalker(
    id: string,
    serverUrl: string,
    macAddress: string,
    title: string,
    userAgent?: string,
    username?: string,
    password?: string,
    deviceId1?: string,
    deviceId2?: string,
    serialNumber?: string,
    signature1?: string,
    signature2?: string,
    autoRefresh?: boolean
  ): Promise<StalkerAddResult> {
    error = null;
    loading = true;
    try {
      const result = await updateStalkerPlaylist(
        id,
        serverUrl,
        macAddress,
        title,
        userAgent,
        username,
        password,
        deviceId1,
        deviceId2,
        serialNumber,
        signature1,
        signature2,
        autoRefresh
      );
      await loadPlaylists();
      return result;
    } catch (err) {
      error = formatError(err);
      console.error('[playlistStore] Failed to update Stalker playlist:', error);
      throw err;
    } finally {
      loading = false;
    }
  }

  async function deletePlaylistById(id: string) {
    error = null;
    loading = true;
    try {
      await deletePlaylist(id);
      if (activePlaylistId === id) {
        activePlaylistId = null;
      }
      await loadPlaylists();
    } catch (err) {
      error = formatError(err);
      throw err;
    } finally {
      loading = false;
    }
  }

  async function refreshPlaylistById(id: string) {
    error = null;
    loading = true;
    try {
      await refreshPlaylist(id);
      await loadPlaylists();
    } catch (err) {
      error = formatError(err);
      throw err;
    } finally {
      loading = false;
    }
  }

  function setActive(id: string | null) {
    activePlaylistId = id;
  }

  const activePlaylist = $derived(() => playlists.find(p => p._id === activePlaylistId) ?? null);

  return {
    get playlists() { return playlists; },
    get loading() { return loading; },
    get error() { return error; },
    get activePlaylistId() { return activePlaylistId; },
    set activePlaylistId(id: string | null) { activePlaylistId = id; },
    get activePlaylist() { return activePlaylist(); },
    loadPlaylists,
    importM3u,
    updateM3u,
    addXtream,
    updateXtream,
    addStalker,
    updateStalker,
    deletePlaylist: deletePlaylistById,
    refreshPlaylist: refreshPlaylistById,
    setActive,
  };
}

export const playlistStore = createPlaylistStore();
