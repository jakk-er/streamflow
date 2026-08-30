import { invoke } from '@tauri-apps/api/core';
import type {
  Channel,
  SeriesDetails,
  StalkerAuthOutcome,
  StalkerCategory,
  StalkerChannel,
  StalkerContentItem,
  StalkerContentPage,
  StalkerContentType,
  VodDetails,
} from '$lib/types';

/**
 * Authenticate (or re-authenticate) with a Stalker portal playlist.
 * @param playlistId - The UUID of the Stalker playlist
 * @param username - Optional login, only used if the portal answers status 2
 * @param password - Optional password, only used if the portal answers status 2
 */
export async function stalkerAuth(
  playlistId: string,
  username?: string,
  password?: string
): Promise<StalkerAuthOutcome> {
  return await invoke<StalkerAuthOutcome>('stalker_auth', { playlistId, username, password });
}

/**
 * Completes a login-required (`status: 2`) portal's auth flow. The playlist
 * must already carry a pending endpoint/token from a prior `stalkerAuth`
 * call that returned `loginRequired`.
 */
export async function stalkerDoAuth(
  playlistId: string,
  username: string,
  password: string
): Promise<StalkerAuthOutcome> {
  return await invoke<StalkerAuthOutcome>('stalker_do_auth', { playlistId, username, password });
}

export async function stalkerRefreshToken(playlistId: string): Promise<StalkerAuthOutcome> {
  return await invoke<StalkerAuthOutcome>('stalker_refresh_token', { playlistId });
}

/**
 * Derives `[deviceId1, deviceId2]` from a MAC (StbEmu/`stalker-to-m3u`-
 * compatible SHA-256), for opt-in form prefill only - never called
 * automatically. Returns `null` for a non-12-hex-digit MAC.
 */
export async function stalkerDeriveDeviceIds(macAddress: string): Promise<[string, string] | null> {
  return await invoke<[string, string] | null>('stalker_derive_device_ids', { macAddress });
}

/** Lightweight keep-alive ping - failures are non-fatal on the portal side. */
export async function stalkerWatchdogPing(playlistId: string, init?: boolean): Promise<void> {
  await invoke<void>('stalker_watchdog_ping', { playlistId, init });
}

export async function stalkerGetCategories(
  playlistId: string,
  contentType: StalkerContentType
): Promise<StalkerCategory[]> {
  return await invoke<StalkerCategory[]>('stalker_get_categories', { playlistId, contentType });
}

export async function stalkerGetContent(
  playlistId: string,
  contentType: StalkerContentType,
  categoryId?: string,
  page = 1
): Promise<StalkerContentPage> {
  return await invoke<StalkerContentPage>('stalker_get_content', {
    playlistId,
    contentType,
    categoryId,
    page,
  });
}

/**
 * Resolves a single movie's playable URL. `item` should be the exact row
 * previously returned by `stalkerGetContent` - Stalker has no separate
 * "get one item" endpoint, the catalog row already carries the detail
 * fields.
 */
export async function stalkerGetVodInfo(
  playlistId: string,
  contentType: StalkerContentType,
  item: StalkerContentItem
): Promise<VodDetails> {
  return await invoke<VodDetails>('stalker_get_vod_info', { playlistId, contentType, item });
}

/**
 * Walks seasons/episodes for a series-shaped item (`type=series` categories,
 * or a VOD row with `isSeries`/embedded `series[]`). Episode URLs are
 * resolved eagerly server-side, matching the Xtream contract `SeriesDetail` expects.
 */
export async function stalkerGetSeriesInfo(
  playlistId: string,
  contentType: StalkerContentType,
  item: StalkerContentItem
): Promise<SeriesDetails> {
  return await invoke<SeriesDetails>('stalker_get_series_info', { playlistId, contentType, item });
}

/**
 * Resolves a playable URL for ITV/radio rows and downloads, applying the
 * same call-or-don't `create_link` decision a real portal client makes.
 */
export async function stalkerResolvePlayback(
  playlistId: string,
  contentType: StalkerContentType,
  cmd: string,
  useHttpTmpLink?: string,
  useLoadBalancing?: string,
  series?: string
): Promise<string> {
  return await invoke<string>('stalker_resolve_playback', {
    playlistId,
    contentType,
    cmd,
    useHttpTmpLink,
    useLoadBalancing,
    series,
  });
}

/**
 * Re-resolves one episode's playback URL fresh right before playing -
 * `create_link` links can go dead while the user lingers on a detail page,
 * so the eagerly-cached `directSource` from `stalkerGetSeriesInfo` isn't reused.
 */
export async function stalkerResolveVodEpisode(
  playlistId: string,
  contentType: StalkerContentType,
  cmd: string,
  series?: string
): Promise<string> {
  return await invoke<string>('stalker_resolve_vod_episode', { playlistId, contentType, cmd, series });
}

/**
 * Headers a resolved stream URL needs on its own fetch, not just the JSON
 * API calls - some portals gate stream/segment requests on the same MAC
 * cookie/session token. The player attaches these itself since it fetches
 * the URL directly, bypassing the app's authenticated HTTP client.
 */
export async function stalkerStreamHeaders(playlistId: string): Promise<[string, string][]> {
  return await invoke<[string, string][]>('stalker_stream_headers', { playlistId });
}

/** Fetches and persists the full ITV + radio channel list into the shared `channels` table. */
export async function stalkerGetChannels(playlistId: string): Promise<Channel[]> {
  return await invoke<Channel[]>('stalker_get_channels', { playlistId });
}

/** Bulk-syncs 7 days of EPG into the shared `epg_programs` table. */
export async function stalkerSyncEpg(playlistId: string): Promise<void> {
  await invoke<void>('stalker_sync_epg', { playlistId });
}

export type { StalkerChannel };
