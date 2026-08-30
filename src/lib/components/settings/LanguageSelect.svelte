<script lang="ts">
  import { settingsStore } from '$lib/stores';

  const languages = [
    { value: 'en', label: 'English' },
    { value: 'de', label: 'Deutsch (German)' },
    { value: 'es', label: 'Espanol (Spanish)' },
    { value: 'fr', label: 'Francais (French)' },
    { value: 'it', label: 'Italiano (Italian)' },
    { value: 'pt', label: 'Portugues (Portuguese)' },
    { value: 'ru', label: 'Russian (Russian)' },
    { value: 'ar', label: 'Arabic (Arabic)' },
    { value: 'zh', label: 'Chinese (Chinese)' },
    { value: 'ja', label: 'Japanese (Japanese)' },
    { value: 'ko', label: 'Korean (Korean)' },
    { value: 'nl', label: 'Nederlands (Dutch)' },
    { value: 'pl', label: 'Polski (Polish)' },
    { value: 'tr', label: 'Turkce (Turkish)' },
    { value: 'uk', label: 'Ukrainian (Ukrainian)' },
  ];

  let selected = $derived(settingsStore.language);

  async function handleChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    await settingsStore.update({ language: target.value as any });
  }
</script>

<select
  value={selected}
  onchange={handleChange}
  class="rounded-lg bg-gray-800 px-4 py-2 text-sm text-white outline-none focus:ring-2 focus:ring-blue-500 border border-gray-700"
>
  {#each languages as lang}
    <option value={lang.value}>{lang.label}</option>
  {/each}
</select>
