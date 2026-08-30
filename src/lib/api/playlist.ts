import { invoke } from '@tauri-apps/api/core';
import type { Playlist, StalkerAuthOutcome } from '$lib/types';

export interface StalkerAddResult {
  playlist: Playlist;
  outcome: StalkerAuthOutcome;
}

/**
 * Import an M3U playlist from a remote URL.
 * @param url - The URL of the M3U playlist file
 * @param title - The title for the new playlist
 * @param userAgent - Optional custom user agent for the HTTP request
 * @returns The created Playlist object
 */
export async function importM3uPlaylist(
  url: string,
  title: string,
  userAgent?: string,
  autoRefresh?: boolean
): Promise<Playlist> {
  return await invoke<Playlist>('import_m3u_playlist', { url, title, userAgent, autoRefresh });
}

/**
 * Edit an existing M3U playlist in place. Re-fetches and re-validates the
 * (possibly changed) URL before writing anything, so a typo being fixed
 * can't silently overwrite a working playlist with a broken one.
 * @param id - The UUID of the playlist to edit
 * @param url - The URL of the M3U playlist file
 * @param title - The playlist's title
 * @param userAgent - Optional custom user agent for the HTTP request
 * @returns The updated Playlist object
 */
export async function updateM3uPlaylist(
  id: string,
  url: string,
  title: string,
  userAgent?: string,
  autoRefresh?: boolean
): Promise<Playlist> {
  return await invoke<Playlist>('update_m3u_playlist', { id, url, title, userAgent, autoRefresh });
}

/**
 * Add a new Xtream API playlist.
 * @param baseUrl - The base URL of the Xtream API server
 * @param username - The Xtream username
 * @param password - The Xtream password
 * @param title - The title for the new playlist
 * @param userAgent - Optional custom user agent for the HTTP request
 * @returns The created Playlist object
 */
export async function addXtreamPlaylist(
  baseUrl: string,
  username: string,
  password: string,
  title: string,
  userAgent?: string,
  autoRefresh?: boolean
): Promise<Playlist> {
  return await invoke<Playlist>('add_xtream_playlist', {
    baseUrl,
    username,
    password,
    title,
    userAgent,
    autoRefresh,
  });
}

/**
 * Edits an Xtream playlist in place - re-authenticates and re-fetches
 * against the new URL/credentials first, so a typo fix can't silently
 * overwrite a working playlist with a broken one.
 * @param id - The UUID of the playlist to edit
 * @param baseUrl - The base URL of the Xtream API server
 * @param username - The Xtream username
 * @param password - The Xtream password
 * @param title - The playlist's title
 * @param userAgent - Optional custom user agent for the HTTP request
 * @returns The updated Playlist object
 */
export async function updateXtreamPlaylist(
  id: string,
  baseUrl: string,
  username: string,
  password: string,
  title: string,
  userAgent?: string,
  autoRefresh?: boolean
): Promise<Playlist> {
  return await invoke<Playlist>('update_xtream_playlist', {
    id,
    baseUrl,
    username,
    password,
    title,
    userAgent,
    autoRefresh,
  });
}

/**
 * Add a new Stalker portal playlist. `username`/`password` are only used if
 * the portal requires login (`status: 2`) - if omitted, `outcome` comes back
 * `{ kind: 'loginRequired' }` so the caller can prompt and retry via
 * `stalkerDoAuth`. `deviceId1`/`deviceId2`/`serialNumber`/`signature1`/
 * `signature2` bind the account alongside the MAC on some portals - leave
 * blank unless the provider gave specific values.
 * @param serverUrl - The URL of the Stalker portal server
 * @param macAddress - The MAC address for authentication
 * @param title - The title for the new playlist
 * @param userAgent - Optional custom user agent for the HTTP request
 * @returns The created Playlist row plus the auth outcome
 */
export async function addStalkerPlaylist(
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
  return await invoke<StalkerAddResult>('add_stalker_playlist', {
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
    autoRefresh,
  });
}

/**
 * Edits a Stalker playlist in place - re-discovers the endpoint and
 * re-authenticates against the new URL/MAC/credentials before writing, so a
 * typo fix can't overwrite a working playlist. Otherwise mirrors
 * {@link addStalkerPlaylist}, including the `loginRequired` outcome.
 * @param id - The UUID of the playlist to edit
 * @param serverUrl - The URL of the Stalker portal server
 * @param macAddress - The MAC address for authentication
 * @param title - The playlist's title
 * @param userAgent - Optional custom user agent for the HTTP request
 * @returns The updated Playlist row plus the auth outcome
 */
export async function updateStalkerPlaylist(
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
  return await invoke<StalkerAddResult>('update_stalker_playlist', {
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
    autoRefresh,
  });
}

/**
 * Get all playlists from the database.
 * @returns Array of all Playlist objects
 */
export async function getPlaylists(): Promise<Playlist[]> {
  return await invoke<Playlist[]>('get_playlists');
}

/**
 * Delete a playlist and its associated data.
 * @param id - The UUID of the playlist to delete
 */
export async function deletePlaylist(id: string): Promise<void> {
  await invoke<void>('delete_playlist', { id });
}

/**
 * Refresh an existing playlist by re-fetching its content.
 * @param id - The UUID of the playlist to refresh
 * @returns The updated Playlist object
 */
export async function refreshPlaylist(id: string): Promise<Playlist> {
  return await invoke<Playlist>('refresh_playlist', { id });
}
