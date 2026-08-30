import { startDownload, getDownloadProgress, pauseDownload, resumeDownload, cancelDownload } from '$lib/api';
import type { DownloadMetadata } from '$lib/types';
import { formatError } from '$lib/utils/errors';

function createDownloadStore() {
  let downloads = $state<DownloadMetadata[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);

  let progressInterval: number | null = null;

  async function start(url: string, filePath: string, headers?: [string, string][]) {
    error = null;
    loading = true;
    try {
      const id = await startDownload(url, filePath, headers);
      await loadAll();
      syncProgressInterval();
      return id;
    } catch (err) {
      error = formatError(err);
      throw err;
    } finally {
      loading = false;
    }
  }

  async function loadAll() {
    error = null;
    loading = true;
    try {
      const results = await Promise.all(
        downloads.map(d => getDownloadProgress(d.id))
      );
      downloads = results;
    } catch (err) {
      error = formatError(err);
    } finally {
      loading = false;
      syncProgressInterval();
    }
  }

  async function pause(id: string) {
    error = null;
    loading = true;
    try {
      await pauseDownload(id);
      await refreshProgress(id);
      syncProgressInterval();
    } catch (err) {
      error = formatError(err);
      throw err;
    } finally {
      loading = false;
    }
  }

  async function resume(id: string) {
    error = null;
    loading = true;
    try {
      await resumeDownload(id);
      await refreshProgress(id);
      syncProgressInterval();
    } catch (err) {
      error = formatError(err);
      throw err;
    } finally {
      loading = false;
    }
  }

  async function cancel(id: string) {
    error = null;
    loading = true;
    try {
      await cancelDownload(id);
      await refreshProgress(id);
      syncProgressInterval();
    } catch (err) {
      error = formatError(err);
      throw err;
    } finally {
      loading = false;
    }
  }

  async function refreshProgress(id: string) {
    try {
      const updated = await getDownloadProgress(id);
      const index = downloads.findIndex(d => d.id === id);
      if (index >= 0) {
        downloads[index] = updated;
      } else {
        downloads.push(updated);
      }
      syncProgressInterval();
      return updated;
    } catch (err) {
      console.error('Failed to refresh download progress:', err);
    }
  }

  const activeDownloads = $derived(() => downloads.filter(d => d.status === 'downloading' || d.status === 'pending'));

  function syncProgressInterval() {
    const hasActive = activeDownloads().length > 0;
    if (hasActive && progressInterval === null) {
      progressInterval = window.setInterval(() => {
        loadAll();
      }, 2000);
    } else if (!hasActive && progressInterval !== null) {
      clearInterval(progressInterval);
      progressInterval = null;
    }
  }

  return {
    get downloads() { return downloads; },
    get loading() { return loading; },
    get error() { return error; },
    get activeDownloads() { return activeDownloads(); },
    start,
    loadAll,
    pause,
    resume,
    cancel,
    refreshProgress,
  };
}

export const downloadStore = createDownloadStore();
