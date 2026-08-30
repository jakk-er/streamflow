<script lang="ts">
  import { onMount } from 'svelte';
  import { settingsStore, playlistStore } from '$lib/stores';
  import SettingsSection from '$lib/components/settings/SettingsSection.svelte';
  import GeneralSettings from '$lib/components/settings/GeneralSettings.svelte';
  import PlaylistManager from '$lib/components/settings/PlaylistManager.svelte';
  import PlayerSettings from '$lib/components/settings/PlayerSettings.svelte';
  import ThemeToggle from '$lib/components/settings/ThemeToggle.svelte';
  import LanguageSelect from '$lib/components/settings/LanguageSelect.svelte';

  onMount(() => {
    (async () => {
      await settingsStore.load();
      await playlistStore.loadPlaylists();
    })();

    // A link like the dashboard's "Add Playlist" button
    // (`/settings?tab=playlists`) should land directly on that section.
    const tab = new URL(window.location.href).searchParams.get('tab');
    if (tab && sections.some((s) => s.id === tab)) {
      scrollToSection(tab);
    }
  });

  function scrollToSection(id: string) {
    const el = document.getElementById(id);
    if (el) {
      el.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
  }

  const sections = [
    { id: 'general', label: 'General' },
    { id: 'playlists', label: 'Playlists' },
    { id: 'player', label: 'Player' },
    { id: 'appearance', label: 'Appearance' },
    { id: 'about', label: 'About' },
  ];

  let activeSection = $state('general');

  function handleScroll() {
    const scrollPos = window.scrollY + 100;
    for (const section of sections) {
      const el = document.getElementById(section.id);
      if (el && el.offsetTop <= scrollPos) {
        activeSection = section.id;
      }
    }
  }
</script>

<svelte:window onscroll={handleScroll} />

<div class="flex h-full w-full overflow-hidden bg-gray-900 text-white">
  <div class="w-64 flex-shrink-0 border-r border-gray-700 overflow-y-auto hidden md:block">
    <div class="p-4">
      <h1 class="text-xl font-bold mb-6">Settings</h1>
      <nav class="space-y-1">
        {#each sections as section}
          <button
            class={`w-full text-left px-3 py-2 rounded-lg text-sm font-medium transition-colors ${
              activeSection === section.id
                ? 'bg-blue-600 text-white'
                : 'text-gray-300 hover:bg-gray-800'
            }`}
            onclick={() => scrollToSection(section.id)}
          >
            {section.label}
          </button>
        {/each}
      </nav>
    </div>
  </div>

  <div class="flex-1 overflow-y-auto">
    <div class="max-w-3xl mx-auto p-6 space-y-8">
      <SettingsSection id="general" title="General" description="General application settings">
        <GeneralSettings />
      </SettingsSection>

      <SettingsSection id="playlists" title="Playlists" description="Manage your IPTV playlists">
        <PlaylistManager />
      </SettingsSection>

      <SettingsSection id="player" title="Player" description="Video playback configuration">
        <PlayerSettings />
      </SettingsSection>

      <SettingsSection id="appearance" title="Appearance" description="Theme and language settings">
        <div class="space-y-6">
          <div>
            <div class="block text-sm font-medium text-white mb-2">Theme</div>
            <ThemeToggle />
          </div>
          <div>
            <div class="block text-sm font-medium text-white mb-2">Language</div>
            <LanguageSelect />
          </div>
        </div>
      </SettingsSection>

      <SettingsSection id="about" title="About" description="Application information">
        <div class="space-y-4">
          <div class="flex items-center gap-4">
            <img src="/streamflow-icon.svg" alt="" class="w-12 h-12 flex-shrink-0" />
            <div>
              <h3 class="text-lg font-semibold text-white">StreamFlow</h3>
              <p class="text-sm text-gray-400">Version 0.1.0</p>
            </div>
          </div>

          <p class="text-sm text-gray-400">
            StreamFlow is a modern IPTV player built with Tauri, SvelteKit, and Rust.
          </p>

          <div class="flex gap-3">
            <!-- TODO: replace with the real repo URL once StreamFlow is pushed to GitHub -->
            <a href="https://github.com/streamflow-app/streamflow" target="_blank" rel="noopener noreferrer" class="rounded-lg bg-gray-800 px-4 py-2 text-sm text-gray-300 hover:bg-gray-700 transition-colors">
              GitHub
            </a>
          </div>
        </div>
      </SettingsSection>
    </div>
  </div>
</div>
