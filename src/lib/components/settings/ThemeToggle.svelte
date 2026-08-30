<script lang="ts">
  import { settingsStore } from '$lib/stores';

  const themes = [
    { value: 'LIGHT_THEME', label: 'Light' },
    { value: 'DARK_THEME', label: 'Dark' },
    { value: 'SYSTEM_THEME', label: 'System' },
  ] as const;

  let selected = $derived(settingsStore.theme);

  async function handleChange(value: string) {
    await settingsStore.update({ theme: value as 'DARK_THEME' | 'LIGHT_THEME' | 'SYSTEM_THEME' });
  }
</script>

<div class="flex items-center gap-2">
  {#each themes as theme}
    <button
      class={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
        selected === theme.value
          ? 'bg-blue-600 text-white'
          : 'bg-gray-800 text-gray-300 hover:bg-gray-700'
      }`}
      onclick={() => handleChange(theme.value)}
    >
      {theme.label}
    </button>
  {/each}
</div>
