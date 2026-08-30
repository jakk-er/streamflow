import { invoke } from '@tauri-apps/api/core';
import type { DownloadMetadata } from '$lib/types';

/**
 * Start downloading a file from a URL. `filePath` is a bare filename - the
 * backend resolves/de-dupes it against the app's downloads directory itself.
 * @param url - The URL of the file to download
 * @param filePath - Desired file name (not a full path)
 * @param headers - Extra headers some sources need (e.g. Stalker stream
 *   headers); most (Xtream, M3U, Stalker CDN links) already have the token in the URL
 * @returns A download ID string for tracking progress
 */
export async function startDownload(url: string, filePath: string, headers?: [string, string][]): Promise<string> {
  return await invoke<string>('start_download', { url, filePath, headers });
}

/**
 * Get the current progress of a download.
 * @param id - The download ID
 * @returns The DownloadMetadata object
 */
export async function getDownloadProgress(id: string): Promise<DownloadMetadata> {
  return await invoke<DownloadMetadata>('get_download_progress', { id });
}

/**
 * Pause a running download. The partial file is kept on disk so
 * `resumeDownload` can continue it later.
 * @param id - The download ID
 */
export async function pauseDownload(id: string): Promise<void> {
  await invoke<void>('pause_download', { id });
}

/**
 * Resume a paused/failed download. Checks the source's `ETag`/`Last-Modified`
 * first - if it changed since pausing, restarts from byte 0 instead of producing a corrupt file.
 * @param id - The download ID
 */
export async function resumeDownload(id: string): Promise<void> {
  await invoke<void>('resume_download', { id });
}

/**
 * Cancel a download (running or paused) and delete its partial file.
 * @param id - The download ID
 */
export async function cancelDownload(id: string): Promise<void> {
  await invoke<void>('cancel_download', { id });
}
