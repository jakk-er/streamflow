<script lang="ts">
  let { digits = $bindable('') }: { digits?: string } = $props();

  let visible = $state(false);
  let timeout: number | null = null;

  function show(d: string) {
    digits = d;
    visible = true;
    if (timeout) clearTimeout(timeout);
    timeout = window.setTimeout(() => {
      visible = false;
      digits = '';
    }, 1500);
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      visible = false;
      digits = '';
      if (timeout) clearTimeout(timeout);
    }
    if (event.key === 'Enter') {
      visible = false;
      digits = '';
      if (timeout) clearTimeout(timeout);
    }
  }

  $effect(() => {
    if (visible) {
      window.addEventListener('keydown', handleKeydown);
      return () => window.removeEventListener('keydown', handleKeydown);
    }
  });
</script>

{#if visible}
  <div class="fixed bottom-8 left-1/2 z-50 -translate-x-1/2">
    <div class="flex items-center gap-2 rounded-lg bg-gray-900/90 px-6 py-3 shadow-xl backdrop-blur-sm border border-gray-700">
      <span class="text-3xl font-mono font-bold text-white tracking-wider">{digits}</span>
      <span class="text-sm text-gray-400">Channel</span>
    </div>
  </div>
{/if}
