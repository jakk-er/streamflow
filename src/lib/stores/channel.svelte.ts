import { listen } from '@tauri-apps/api/event';
import { getChannelsByPlaylist, searchChannels } from '$lib/api';
import type { Channel } from '$lib/types';
import { formatError } from '$lib/utils/errors';

function createChannelStore() {
  let channels = $state<Channel[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let searchQuery = $state('');
  let selectedGroup = $state<string | null>(null);
  let activeChannel = $state<Channel | null>(null);
  let loadedPlaylistId: string | null = null;
  // 'none' until the user engages the sort toggle - preserves the existing
  // default ordering until they ask for name sort; then only toggles
  // direction.
  let sortBy = $state<'none' | 'name-asc' | 'name-desc'>('none');

  /**
   * Re-reads the currently loaded playlist's channels WITHOUT resetting the
   * user's group filter the way `loadChannels` does.
   */
  async function refreshLoaded() {
    if (!loadedPlaylistId) return;
    try {
      channels = await getChannelsByPlaylist(loadedPlaylistId);
    } catch (err) {
      console.error('[channelStore] Failed to refresh channels:', formatError(err));
    }
  }

  // Stalker's censored-category recovery (a background crawl of genres the
  // portal excludes from its bulk endpoint, typically adult) writes channels
  // to the DB after this store already loaded - re-read on this event or
  // those rows stay invisible until the next restart. `loadedPlaylistId`,
  // not `channels`, identifies the held list since search can replace
  // `channels` with a cross-playlist result set.
  listen<string>('channels-updated', (event) => {
    if (event.payload !== loadedPlaylistId) return;
    if (searchQuery.trim()) return;
    void refreshLoaded();
  }).catch(() => {
    // Not running under Tauri (a plain-browser `vite dev`) - the list simply
    // doesn't live-update there.
  });

  async function loadChannels(playlistId: string) {
    error = null;
    loading = true;
    try {
      channels = await getChannelsByPlaylist(playlistId);
      loadedPlaylistId = playlistId;
      searchQuery = '';
      selectedGroup = null;
    } catch (err) {
      error = formatError(err);
      console.error(`[channelStore] Failed to load channels:`, error);
    } finally {
      loading = false;
    }
  }

  async function search(query: string, playlistId?: string) {
    error = null;
    loading = true;
    try {
      channels = await searchChannels(query, playlistId);
      searchQuery = query;
      selectedGroup = null;
    } catch (err) {
      error = formatError(err);
    } finally {
      loading = false;
    }
  }

  function selectChannel(channel: Channel | null) {
    activeChannel = channel;
  }

  function setGroupFilter(group: string | null) {
    selectedGroup = group;
  }

  function toggleNameSort() {
    sortBy = sortBy === 'name-asc' ? 'name-desc' : 'name-asc';
  }

  const filteredChannels = $derived(() => {
    let result = channels;
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase().trim();
      result = result.filter(c => c.name.toLowerCase().includes(q));
    }
    if (selectedGroup) {
      result = result.filter(c => c.group.title === selectedGroup);
    }
    // The unfiltered "All" view otherwise falls back to the DB's global
    // `channel_number` order - some Stalker portals assign numbers *per
    // genre*, so several genres all start at 1-15 and collide, floating to
    // the front regardless of sync order. Grouping by title first keeps each
    // genre contiguous (stable sort preserves relative order within a group)
    // without touching the raw `channelNumber` used elsewhere for display/zapping.
    if (!searchQuery.trim() && !selectedGroup) {
      result = [...result].sort((a, b) => a.group.title.localeCompare(b.group.title));
    }
    // An explicit name sort always wins over the implicit group-first default.
    if (sortBy === 'name-asc') {
      result = [...result].sort((a, b) => a.name.localeCompare(b.name));
    } else if (sortBy === 'name-desc') {
      result = [...result].sort((a, b) => b.name.localeCompare(a.name));
    }
    return result;
  });

  const groups = $derived(() => {
    const set = new Set(channels.map(c => c.group.title).filter(Boolean));
    return Array.from(set).sort();
  });

  return {
    get channels() { return channels; },
    get loading() { return loading; },
    get error() { return error; },
    get searchQuery() { return searchQuery; },
    get selectedGroup() { return selectedGroup; },
    get activeChannel() { return activeChannel; },
    set activeChannel(channel: Channel | null) { activeChannel = channel; },
    get filteredChannels() { return filteredChannels(); },
    get groups() { return groups(); },
    get sortBy() { return sortBy; },
    loadChannels,
    search,
    selectChannel,
    setGroupFilter,
    toggleNameSort,
  };
}

export const channelStore = createChannelStore();
