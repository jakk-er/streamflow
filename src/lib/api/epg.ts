import { invoke } from '@tauri-apps/api/core';
import type { EpgProgram } from '$lib/types';

/**
 * Fetch and parse EPG data from a remote URL.
 * @param playlistId - The UUID of the playlist to associate EPG with
 * @param epgUrl - The URL of the XMLTV EPG file
 */
export async function fetchEpg(playlistId: string, epgUrl: string): Promise<void> {
  await invoke<void>('fetch_epg', { playlistId, epgUrl });
}

/**
 * Get EPG programs for a specific channel within a time range.
 * @param channelId - The UUID of the channel
 * @param start - Start time in ISO-8601 format (YYYY-MM-DDTHH:MM:SS)
 * @param end - End time in ISO-8601 format (YYYY-MM-DDTHH:MM:SS)
 * @returns Array of EpgProgram objects
 */
export async function getEpgForChannel(
  channelId: string,
  start: string,
  end: string
): Promise<EpgProgram[]> {
  return await invoke<EpgProgram[]>('get_epg_for_channel', {
    channelId,
    start,
    end,
  });
}

/**
 * Get the currently playing EPG program for a channel.
 * @param channelId - The UUID of the channel
 * @returns The current EpgProgram or null if none is playing
 */
export async function getCurrentProgram(channelId: string): Promise<EpgProgram | null> {
  return await invoke<EpgProgram | null>('get_current_program', { channelId });
}
