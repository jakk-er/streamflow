<script lang="ts">
  import { goto } from '$app/navigation';
  import { settingsStore, playlistStore } from '$lib/stores';

  let isOpen = $state(false);
  let query = $state('');
  let selectedIndex = $state(0);

  interface Command {
    label: string;
    action: () => void;
    keywords?: string[];
  }

  const allCommands: Command[] = [
    {
      label: 'Go to Live TV',
      action: () => goto('/live'),
      keywords: ['live', 'tv', 'television'],
    },
    {
      label: 'Go to VOD',
      action: () => goto('/vod'),
      keywords: ['vod', 'movies', 'video'],
    },
    {
      label: 'Go to Favorites',
      action: () => goto('/favorites'),
      keywords: ['favorites', 'liked'],
    },
    {
      label: 'Go to History',
      action: () => goto('/history'),
      keywords: ['history', 'recent', 'watched'],
    },
    {
      label: 'Go to Settings',
      action: () => goto('/settings'),
      keywords: ['settings', 'preferences', 'config'],
    },
    {
      label: 'Toggle Theme',
      action: () => {
        const current = settingsStore.theme;
        const next = current === 'DARK_THEME' ? 'LIGHT_THEME' : 'DARK_THEME';
        settingsStore.update({ theme: next });
      },
      keywords: ['theme', 'dark', 'light', 'mode'],
    },
    {
      label: 'Refresh Playlists',
      action: () => playlistStore.loadPlaylists(),
      keywords: ['refresh', 'playlists', 'reload'],
    },
    {
      label: 'Search Channels...',
      action: () => {
        // Placeholder: focus search input
      },
      keywords: ['search', 'channels', 'find'],
    },
  ];

  let inputRef: HTMLInputElement | null = $state(null);

  $effect(() => {
    if (inputRef) inputRef.focus();
  });

  let filteredCommands = $derived(() => {
    const q = query.toLowerCase().trim();
    if (!q) return allCommands;
    return allCommands.filter(cmd => {
      const haystack = `${cmd.label} ${(cmd.keywords ?? []).join(' ')}`.toLowerCase();
      return haystack.includes(q);
    });
  });

  function open() {
    isOpen = true;
    query = '';
    selectedIndex = 0;
  }

  function close() {
    isOpen = false;
    query = '';
    selectedIndex = 0;
  }

  function selectNext() {
    const total = filteredCommands().length;
    if (total === 0) return;
    selectedIndex = (selectedIndex + 1) % total;
  }

  function selectPrev() {
    const total = filteredCommands().length;
    if (total === 0) return;
    selectedIndex = (selectedIndex - 1 + total) % total;
  }

  function executeSelected() {
    const commands = filteredCommands();
    if (commands.length === 0) return;
    const cmd = commands[selectedIndex];
    close();
    cmd.action();
  }

  // Keyboard shortcuts
  import { registerShortcut } from '$lib/utils/keyboard';

  const unregisterOpen = registerShortcut('ctrl+k', open);
  const unregisterCmdK = registerShortcut('cmd+k', open);
  const unregisterEscape = registerShortcut('escape', close);
  const unregisterArrowDown = registerShortcut('arrowdown', () => {
    if (isOpen) {
      selectNext();
    }
  });
  const unregisterArrowUp = registerShortcut('arrowup', () => {
    if (isOpen) {
      selectPrev();
    }
  });
  const unregisterEnter = registerShortcut('enter', () => {
    if (isOpen) {
      executeSelected();
    }
  });

  // Cleanup
  import { onMount } from 'svelte';
  onMount(() => {
    return () => {
      unregisterOpen();
      unregisterCmdK();
      unregisterEscape();
      unregisterArrowDown();
      unregisterArrowUp();
      unregisterEnter();
    };
  });
</script>

{#if isOpen}
  <div
    class="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-[20vh]"
    onclick={(e) => { if (e.target === e.currentTarget) close(); }}
    onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') close(); }}
    role="button"
    tabindex="0"
    aria-label="Close command palette"
  >
    <div
      class="w-full max-w-lg rounded-lg bg-gray-800 shadow-xl"
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
    >
      <div class="border-b border-gray-700 px-4 py-3">
        <input
          type="text"
          placeholder="Type a command..."
          class="w-full bg-transparent text-white outline-none placeholder-gray-400"
          bind:value={query}
          bind:this={inputRef}
        />
      </div>
      <div class="max-h-80 overflow-y-auto py-2">
        {#each filteredCommands() as cmd, i (cmd.label)}
          <button
            class={`w-full px-4 py-2 text-left text-sm transition-colors ${
              i === selectedIndex
                ? 'bg-blue-600 text-white'
                : 'text-gray-300 hover:bg-gray-700'
            }`}
            onclick={() => {
              selectedIndex = i;
              executeSelected();
            }}
            onmouseenter={() => (selectedIndex = i)}
          >
            {cmd.label}
          </button>
        {/each}
        {#if filteredCommands().length === 0}
          <div class="px-4 py-4 text-center text-sm text-gray-400">No commands found</div>
        {/if}
      </div>
      <div class="border-t border-gray-700 px-4 py-2 text-xs text-gray-400">
        <span class="mr-4">↑↓ Navigate</span>
        <span class="mr-4">Enter Select</span>
        <span>Esc Close</span>
      </div>
    </div>
  </div>
{/if}
