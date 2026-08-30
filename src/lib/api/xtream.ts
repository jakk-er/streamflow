import { invoke } from '@tauri-apps/api/core';
import type { XtreamCategory, XtreamStream, XtreamUserInfo, VodDetails, SeriesDetails } from '$lib/types';

/**
 * Authenticate with an Xtream API server.
 * @param playlistId - The UUID of the Xtream playlist
 * @returns The XtreamUserInfo object
 */
export async function xtreamAuth(playlistId: string): Promise<XtreamUserInfo> {
  return await invoke<XtreamUserInfo>('xtream_auth', { playlistId });
}

/**
 * Get categories from an Xtream API server.
 * @param playlistId - The UUID of the Xtream playlist
 * @param streamType - The type of streams ('live', 'vod', 'series')
 * @returns Array of XtreamCategory objects
 */
export async function xtreamGetCategories(
  playlistId: string,
  streamType: string
): Promise<XtreamCategory[]> {
  return await invoke<XtreamCategory[]>('xtream_get_categories', {
    playlistId,
    streamType,
  });
}

/**
 * Get streams from an Xtream API server.
 * @param playlistId - The UUID of the Xtream playlist
 * @param streamType - The type of streams ('live', 'vod', 'series')
 * @param categoryId - Optional category ID to filter streams
 * @returns Array of XtreamStream objects
 */
export async function xtreamGetStreams(
  playlistId: string,
  streamType: string,
  categoryId?: string
): Promise<XtreamStream[]> {
  return await invoke<XtreamStream[]>('xtream_get_streams', {
    playlistId,
    streamType,
    categoryId,
  });
}

/**
 * Get VOD (movie) details from an Xtream API server.
 * @param playlistId - The UUID of the Xtream playlist
 * @param vodId - The VOD stream id
 * @returns The VodDetails object
 */
export async function xtreamGetVodInfo(playlistId: string, vodId: string): Promise<VodDetails> {
  return await invoke<VodDetails>('xtream_get_vod_info', { playlistId, vodId });
}

/**
 * Get series details (seasons/episodes) from an Xtream API server.
 * @param playlistId - The UUID of the Xtream playlist
 * @param seriesId - The series id
 * @returns The SeriesDetails object
 */
export async function xtreamGetSeriesInfo(playlistId: string, seriesId: string): Promise<SeriesDetails> {
  return await invoke<SeriesDetails>('xtream_get_series_info', { playlistId, seriesId });
}

/**
 * Builds a catch-up playback URL for a live stream at an EPG program's
 * `[startTimestamp, stopTimestamp)` window (Unix seconds) - only meaningful
 * when `tvArchive === 1` and `tvArchiveDuration > 0` (see `XtreamStream`).
 */
export async function xtreamResolveCatchupUrl(
  playlistId: string,
  streamId: number,
  startTimestamp: number,
  stopTimestamp: number
): Promise<string> {
  return await invoke<string>('xtream_resolve_catchup_url', {
    playlistId,
    streamId,
    startTimestamp,
    stopTimestamp,
  });
}

/** Whether a stored channel (by its unified id) supports Xtream catch-up. */
export async function xtreamChannelCatchupAvailable(channelId: string): Promise<boolean> {
  return await invoke<boolean>('xtream_channel_catchup_available', { channelId });
}

/**
 * Resolves a catch-up URL for a stored channel at an EPG program's window
 * (Unix seconds) - looks up the stream id/playlist itself. Returns null if
 * the channel isn't a catch-up-eligible Xtream channel.
 */
export async function xtreamResolveCatchupUrlForChannel(
  channelId: string,
  startTimestamp: number,
  stopTimestamp: number
): Promise<string | null> {
  return await invoke<string | null>('xtream_resolve_catchup_url_for_channel', {
    channelId,
    startTimestamp,
    stopTimestamp,
  });
}
