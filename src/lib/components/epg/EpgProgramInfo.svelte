<script lang="ts">
  import type { EpgProgram } from '$lib/types';

  let { program = null }: { program?: import('$lib/types').EpgProgram | null } = $props();

  function formatTimeRange(start: string, stop: string) {
    const s = new Date(start);
    const e = new Date(stop);
    const opts: Intl.DateTimeFormatOptions = { hour: '2-digit', minute: '2-digit' };
    return `${s.toLocaleTimeString([], opts)} - ${e.toLocaleTimeString([], opts)}`;
  }
</script>

{#if program}
  <div class="max-w-sm rounded-lg bg-gray-800 p-4 shadow-xl border border-gray-700">
    <div class="flex items-start justify-between gap-2">
      <h4 class="font-semibold text-white">{program.title}</h4>
      {#if program.icon}
        <img src={program.icon} alt="" class="h-10 w-10 rounded object-cover flex-shrink-0" />
      {/if}
    </div>

    <div class="mt-2 text-xs text-gray-400">
      {formatTimeRange(program.start, program.stop)}
    </div>

    {#if program.category}
      <span class="mt-2 inline-block rounded bg-gray-700 px-2 py-0.5 text-xs text-gray-300">
        {program.category}
      </span>
    {/if}

    {#if program.description}
      <p class="mt-2 text-sm text-gray-300 line-clamp-3">{program.description}</p>
    {/if}
  </div>
{/if}
