<script lang="ts">
  import { downloadStore } from '$lib/stores';

  let {
    url,
    title,
    fileName = undefined,
    resolveUrl = undefined,
  }: {
    url: string;
    title: string;
    fileName?: string;
    /**
     * Called right before the download starts instead of trusting the static
     * `url` prop - a Stalker `create_link` result is a temporary link that
     * can already be dead by the time this button is clicked (see
     * `VodDetail.svelte`'s `handlePlay`). Returning `null` aborts silently.
     */
    resolveUrl?: () => Promise<{ url: string; headers?: [string, string][] } | null>;
  } = $props();

  let status = $state<'idle' | 'downloading' | 'paused' | 'completed' | 'failed'>('idle');
  let progress = $state(0);
  let downloadId = $state<string | null>(null);

  function getFileName() {
    if (fileName) return fileName;
    const ext = url.split('.').pop() || 'mp4';
    return `${title.replace(/[^a-z0-9]/gi, '_')}.${ext}`;
  }

  async function handleClick() {
    if (status === 'idle' || status === 'failed') {
      try {
        const resolved = resolveUrl ? await resolveUrl() : { url };
        if (!resolved || !resolved.url) return;
        downloadId = await downloadStore.start(resolved.url, getFileName(), resolved.headers);
        status = 'downloading';
        progress = 0;
      } catch (err) {
        console.error('Download failed:', err);
        status = 'failed';
      }
    } else if (status === 'downloading' && downloadId) {
      await downloadStore.pause(downloadId);
      status = 'paused';
    } else if (status === 'paused' && downloadId) {
      try {
        await downloadStore.resume(downloadId);
        status = 'downloading';
      } catch (err) {
        console.error('Failed to resume download:', err);
      }
    }
  }

  async function handleCancel(e: MouseEvent) {
    e.stopPropagation();
    if (!downloadId) return;
    try {
      await downloadStore.cancel(downloadId);
    } catch (err) {
      console.error('Failed to cancel download:', err);
    } finally {
      status = 'idle';
      progress = 0;
      downloadId = null;
    }
  }

  // Poll download progress
  $effect(() => {
    if (!downloadId || status !== 'downloading') return;

    const interval = setInterval(async () => {
      const meta = await downloadStore.refreshProgress(downloadId!);
      if (!meta) return;
      if (meta.status === 'completed') {
        status = 'completed';
        clearInterval(interval);
      } else if (meta.status === 'paused') {
        status = 'paused';
        clearInterval(interval);
      } else if (meta.status === 'failed' || meta.status === 'canceled') {
        status = meta.status === 'canceled' ? 'idle' : 'failed';
        clearInterval(interval);
      } else if (meta.totalBytes && meta.totalBytes > 0) {
        progress = Math.round((meta.downloadedBytes / meta.totalBytes) * 100);
      }
    }, 1000);

    return () => clearInterval(interval);
  });
</script>

<div class="flex items-center gap-1">
  <button
    class={`flex items-center gap-2 rounded-lg px-4 py-2 font-medium transition-colors ${
      status === 'completed'
        ? 'bg-green-600 text-white cursor-default'
        : status === 'downloading'
        ? 'bg-orange-600 text-white'
        : status === 'failed'
        ? 'bg-red-900/60 text-red-200 hover:bg-red-900'
        : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
    }`}
    onclick={handleClick}
    disabled={status === 'completed'}
  >
    {#if status === 'idle'}
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
      </svg>
      Download
    {:else if status === 'downloading'}
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 animate-spin" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
      </svg>
      {progress}%
    {:else if status === 'paused'}
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
        <path stroke-linecap="round" stroke-linejoin="round" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
      </svg>
      Resume ({progress}%)
    {:else if status === 'failed'}
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
      </svg>
      Retry
    {:else if status === 'completed'}
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
      </svg>
      Saved
    {/if}
  </button>

  {#if status === 'downloading' || status === 'paused'}
    <button
      class="rounded-lg p-2 text-gray-400 transition-colors hover:bg-gray-700 hover:text-red-400"
      onclick={handleCancel}
      aria-label="Cancel download"
      title="Cancel download"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
      </svg>
    </button>
  {/if}
</div>
