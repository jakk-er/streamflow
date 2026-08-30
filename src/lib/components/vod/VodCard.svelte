<script lang="ts">
  import { goto } from '$app/navigation';
  import type { VodItem, VodWatchProgress } from '$lib/types';

  let { item, progress }: { item: VodItem; progress?: VodWatchProgress } = $props();

  let progressPercent = $derived.by(() => {
    if (!progress || progress.totalSeconds <= 0) return null;
    return Math.min(100, Math.max(0, (progress.positionSeconds / progress.totalSeconds) * 100));
  });

  let coverFailed = $state(false);
  $effect(() => { item.cover; item.streamIcon; coverFailed = false; });

  function handleClick() {
    const type = item.streamType === 'series' ? 'series' : 'movie';
    goto(`/vod/${item.id}?type=${type}`);
  }

  function getInitial(name: string) {
    return name.charAt(0).toUpperCase();
  }

  function getYear(dateStr?: string) {
    if (!dateStr) return null;
    const date = new Date(dateStr);
    if (isNaN(date.getTime())) return null;
    return date.getFullYear();
  }
</script>

<button
  class="group text-left"
  onclick={handleClick}
>
  <div class="relative aspect-[2/3] overflow-hidden rounded-lg bg-gray-800">
    {#if (item.cover || item.streamIcon) && !coverFailed}
      <img
        src={item.cover || item.streamIcon}
        alt={item.name}
        loading="lazy"
        class="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
        onerror={() => coverFailed = true}
      />
    {:else}
      <div class="flex h-full w-full items-center justify-center bg-gradient-to-br from-gray-700 to-gray-900">
        <span class="text-4xl font-bold text-gray-500">{getInitial(item.name)}</span>
      </div>
    {/if}

    <div class="absolute inset-0 bg-black/0 transition-colors duration-300 group-hover:bg-black/20"></div>

    {#if item.rating}
      <span class="absolute top-2 right-2 flex items-center gap-1 rounded bg-black/70 px-2 py-0.5 text-xs text-yellow-400">
        <svg xmlns="http://www.w3.org/2000/svg" class="h-3 w-3" fill="currentColor" viewBox="0 0 24 24">
          <path d="M11.48 3.499a.562.562 0 011.04 0l2.125 5.111a.563.563 0 00.475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 00-.182.557l1.285 5.385a.562.562 0 01-.84.61l-4.725-2.885a.563.563 0 00-.586 0L6.982 20.54a.562.562 0 01-.84-.61l1.285-5.386a.563.563 0 00-.182-.557l-4.204-3.602a.563.563 0 01.321-.988l5.518-.442a.563.563 0 00.475-.345L11.48 3.5z" />
        </svg>
        {item.rating}
      </span>
    {/if}

    {#if getYear(item.releaseDate)}
      <span class="absolute bottom-2 left-2 rounded bg-black/70 px-2 py-0.5 text-xs text-gray-300">
        {getYear(item.releaseDate)}
      </span>
    {/if}

    <div class="absolute inset-0 flex items-center justify-center opacity-0 transition-opacity duration-300 group-hover:opacity-100">
      <div class="rounded-full bg-white/20 p-3 backdrop-blur-sm">
        <svg xmlns="http://www.w3.org/2000/svg" class="h-8 w-8 text-white" fill="currentColor" viewBox="0 0 24 24">
          <path d="M8 5v14l11-7z" />
        </svg>
      </div>
    </div>

    {#if progressPercent !== null}
      <div class="absolute bottom-0 left-0 right-0 h-1 bg-black/50">
        <div class="h-full bg-blue-500" style="width: {progressPercent}%"></div>
      </div>
    {/if}
  </div>

  <div class="mt-2">
    <h3 class="truncate text-sm font-medium text-white group-hover:text-blue-400 transition-colors">
      {item.name}
    </h3>
    {#if item.genre}
      <p class="mt-1 truncate text-xs text-gray-400">{item.genre}</p>
    {/if}
  </div>
</button>
