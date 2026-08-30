import { invoke } from '@tauri-apps/api/core';

/**
 * Port of the local proxy Stalker playback routes through, so cross-origin
 * CORS/forbidden-header restrictions don't block hls.js/mpegts.js.
 */
export async function getStreamProxyPort(): Promise<number> {
  return await invoke<number>('get_stream_proxy_port');
}

/**
 * Spawn an external player process for a stream URL.
 * @param playerType - The type of player ('mpv', 'vlc', 'iina')
 * @param url - The stream URL to play
 * @param title - Optional title for the stream
 * @returns A session ID string for controlling the player
 */
export async function spawnExternalPlayer(
  playerType: string,
  url: string,
  title?: string
): Promise<string> {
  return await invoke<string>('spawn_external_player', {
    playerType,
    url,
    title,
  });
}

/**
 * Kill a running external player session.
 * @param sessionId - The session ID of the player to kill
 */
export async function killPlayer(sessionId: string): Promise<void> {
  await invoke<void>('kill_player', { sessionId });
}

/**
 * Check if an external player session is still running.
 * @param sessionId - The session ID of the player to check
 * @returns True if the player is running, false otherwise
 */
export async function getPlayerStatus(sessionId: string): Promise<boolean> {
  return await invoke<boolean>('get_player_status', { sessionId });
}
