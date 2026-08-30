import { invoke } from '@tauri-apps/api/core';
import type { Channel } from '$lib/types';

/**
 * Get all channels for a specific playlist.
 * @param playlistId - The UUID of the playlist
 * @returns Array of Channel objects
 */
export async function getChannelsByPlaylist(playlistId: string): Promise<Channel[]> {
  return await invoke<Channel[]>('get_channels_by_playlist', { playlistId });
}

/**
 * Search channels by name across playlists.
 * @param query - The search query string
 * @param playlistId - Optional playlist ID to limit search to
 * @returns Array of matching Channel objects
 */
export async function searchChannels(
  query: string,
  playlistId?: string
): Promise<Channel[]> {
  return await invoke<Channel[]>('search_channels', { query, playlistId });
}

/**
 * Get a single channel by its ID.
 * @param id - The UUID of the channel
 * @returns The Channel object
 */
export async function getChannelById(id: string): Promise<Channel> {
  return await invoke<Channel>('get_channel_by_id', { id });
}

/** Whether an M3U/Xtream-sourced channel supports catch-up (archive replay) playback at all. */
export async function m3uIsCatchupSupported(channelId: string): Promise<boolean> {
  return await invoke<boolean>('m3u_is_catchup_supported', { channelId });
}

/**
 * Builds a playable catch-up URL for a channel at an EPG program's start.
 * @param programStart - ISO or XMLTV-formatted date string for the program's start
 * @param programStartTimestamp - Unix seconds, when known - takes precedence over `programStart`
 * @returns The playable URL, or null if this channel/program doesn't support catch-up
 */
export async function m3uResolveCatchupUrl(
  channelId: string,
  programStart: string,
  programStartTimestamp?: number
): Promise<string | null> {
  return await invoke<string | null>('m3u_resolve_catchup_url', {
    channelId,
    programStart,
    programStartTimestamp,
  });
}
