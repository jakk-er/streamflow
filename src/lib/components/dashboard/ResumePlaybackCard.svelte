<script lang="ts">
  import { playerStore } from '$lib/stores';
  import { goto } from '$app/navigation';

  let { item = null }: { item?: import('$lib/types').WatchHistoryItem | null } = $props();

  function getProgress() {
    if (!item || !item.totalSeconds || item.totalSeconds <= 0) return 0;
    return Math.round((item.positionSeconds / item.totalSeconds) * 100);
  }

  function formatTime(seconds: number) {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  }

  async function handlePlay() {
    await goto('/live');
  }
</script>

{#if item}
  <button
    class="w-full flex items-center gap-4 rounded-xl bg-gray-800 p-3 text-left hover:bg-gray-750 transition-colors group"
    onclick={handlePlay}
  >
    <div class="flex-shrink-0 w-[160px] aspect-video rounded-lg bg-gray-700 overflow-hidden">
      <div class="flex h-full w-full items-center justify-center bg-gradient-to-br from-gray-700 to-gray-900">
        <svg xmlns="http://www.w3.org/2000/svg" class="h-8 w-8 text-gray-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          <path stroke-linecap="round" stroke-linejoin="round" d="M15.91 11.672a.375.375 0 010 .656l-5.603 3.113a.375.375 0 01-.557-.328V8.887c0-.286.307-.466.557-.327l5.603 3.112z" />
        </svg>
      </div>
    </div>

    <div class="flex-1 min-w-0">
      <h3 class="text-sm font-medium text-white truncate group-hover:text-blue-400 transition-colors">
        Channel {item.channelId?.slice(0, 8) || 'Unknown'}
      </h3>
      <p class="text-xs text-gray-400 mt-1">
        {formatTime(item.totalSeconds - item.positionSeconds)} left
      </p>
      <div class="mt-2 h-1 rounded-full bg-gray-700 overflow-hidden">
        <div class="h-full bg-blue-500" style="width: {getProgress()}%"></div>
      </div>
    </div>

    <div class="flex-shrink-0">
      <div class="rounded-full bg-blue-600 p-2 text-white group-hover:bg-blue-500 transition-colors">
        <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="currentColor" viewBox="0 0 24 24">
          <path d="M8 5v14l11-7z" />
        </svg>
      </div>
    </div>
  </button>
{/if}
