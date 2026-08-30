<script lang="ts">
  import { settingsStore } from '$lib/stores';

  let localSettings = $state({
    showEpg: settingsStore.settings?.showEpg ?? true,
    showChannelNumber: settingsStore.settings?.showChannelNumber ?? false,
    hideCategoryNames: settingsStore.settings?.hideCategoryNames ?? false,
    sortChannelsBy: settingsStore.settings?.sortChannelsBy ?? 'default',
    coverSize: settingsStore.settings?.coverSize ?? 'medium',
    enableAnalytics: settingsStore.settings?.enableAnalytics ?? false,
    trackWatchHistory: settingsStore.settings?.trackWatchHistory ?? true,
  });

  let debounceTimer: number | null = null;

  async function handleChange() {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = window.setTimeout(async () => {
      await settingsStore.update({
        showEpg: localSettings.showEpg,
        showChannelNumber: localSettings.showChannelNumber,
        hideCategoryNames: localSettings.hideCategoryNames,
        sortChannelsBy: localSettings.sortChannelsBy as any,
        coverSize: localSettings.coverSize as any,
        enableAnalytics: localSettings.enableAnalytics,
        trackWatchHistory: localSettings.trackWatchHistory,
      });
    }, 300);
  }

  $effect(() => {
    if (settingsStore.settings) {
      localSettings.showEpg = settingsStore.settings.showEpg;
      localSettings.showChannelNumber = settingsStore.settings.showChannelNumber;
      localSettings.hideCategoryNames = settingsStore.settings.hideCategoryNames;
      localSettings.sortChannelsBy = settingsStore.settings.sortChannelsBy;
      localSettings.coverSize = settingsStore.settings.coverSize;
      localSettings.enableAnalytics = settingsStore.settings.enableAnalytics;
      localSettings.trackWatchHistory = settingsStore.settings.trackWatchHistory;
    }
  });
</script>

<div class="space-y-6">
  <label class="flex cursor-pointer items-center justify-between">
    <div>
      <span class="block text-sm font-medium text-white">Show EPG</span>
      <span class="text-xs text-gray-400">Display electronic program guide data for channels.</span>
    </div>
    <button
      type="button"
      class="relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 focus:ring-offset-gray-900 {localSettings.showEpg ? 'bg-blue-600' : 'bg-gray-700'}"
      onclick={() => { localSettings.showEpg = !localSettings.showEpg; handleChange(); }}
      aria-label="Toggle show EPG"
    >
      <span class="inline-block h-4 w-4 transform rounded-full bg-white transition-transform {localSettings.showEpg ? 'translate-x-6' : 'translate-x-1'}"></span>
    </button>
  </label>

  {#if localSettings.showEpg}
    <div>
      <label class="block text-sm font-medium text-white mb-1" for="epg-source">EPG Source URL</label>
      <input
        type="url"
        id="epg-source"
        placeholder="http://example.com/epg.xml"
        value={settingsStore.settings?.epgSource?.[0] ?? ''}
        oninput={(e) => {
          const url = (e.target as HTMLInputElement).value;
          settingsStore.update({ epgSource: url ? [url] : [] });
        }}
        class="w-full rounded-lg bg-gray-800 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-700"
      />
      <p class="mt-2 text-xs text-gray-400">XMLTV format EPG source URL.</p>
    </div>
  {/if}

  <label class="flex cursor-pointer items-center justify-between">
    <div>
      <span class="block text-sm font-medium text-white">Show Channel Numbers</span>
      <span class="text-xs text-gray-400">Display channel numbers in the channel list.</span>
    </div>
    <button
      type="button"
      class="relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 focus:ring-offset-gray-900 {localSettings.showChannelNumber ? 'bg-blue-600' : 'bg-gray-700'}"
      onclick={() => { localSettings.showChannelNumber = !localSettings.showChannelNumber; handleChange(); }}
      aria-label="Toggle show channel numbers"
    >
      <span class="inline-block h-4 w-4 transform rounded-full bg-white transition-transform {localSettings.showChannelNumber ? 'translate-x-6' : 'translate-x-1'}"></span>
    </button>
  </label>

  <label class="flex cursor-pointer items-center justify-between">
    <div>
      <span class="block text-sm font-medium text-white">Hide Category Names</span>
      <span class="text-xs text-gray-400">Hide category/group names in the channel list.</span>
    </div>
    <button
      type="button"
      class="relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 focus:ring-offset-gray-900 {localSettings.hideCategoryNames ? 'bg-blue-600' : 'bg-gray-700'}"
      onclick={() => { localSettings.hideCategoryNames = !localSettings.hideCategoryNames; handleChange(); }}
      aria-label="Toggle hide category names"
    >
      <span class="inline-block h-4 w-4 transform rounded-full bg-white transition-transform {localSettings.hideCategoryNames ? 'translate-x-6' : 'translate-x-1'}"></span>
    </button>
  </label>

  <label class="flex cursor-pointer items-center justify-between">
    <div>
      <span class="block text-sm font-medium text-white">Track Watch History</span>
      <span class="text-xs text-gray-400">Remember recently watched channels and playback position.</span>
    </div>
    <button
      type="button"
      class="relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 focus:ring-offset-gray-900 {localSettings.trackWatchHistory ? 'bg-blue-600' : 'bg-gray-700'}"
      onclick={() => { localSettings.trackWatchHistory = !localSettings.trackWatchHistory; handleChange(); }}
      aria-label="Toggle track watch history"
    >
      <span class="inline-block h-4 w-4 transform rounded-full bg-white transition-transform {localSettings.trackWatchHistory ? 'translate-x-6' : 'translate-x-1'}"></span>
    </button>
  </label>

  <div>
    <label class="block text-sm font-medium text-white mb-1" for="sort-channels">Sort Channels By</label>
    <select
      id="sort-channels"
      value={localSettings.sortChannelsBy}
      onchange={(e) => { localSettings.sortChannelsBy = (e.target as HTMLSelectElement).value as any; handleChange(); }}
      class="rounded-lg bg-gray-800 px-4 py-2 text-sm text-white outline-none focus:ring-2 focus:ring-blue-500 border border-gray-700"
    >
      <option value="default">Default</option>
      <option value="name-az">Name (A-Z)</option>
      <option value="name-za">Name (Z-A)</option>
    </select>
  </div>

  <div>
    <label class="block text-sm font-medium text-white mb-1" for="cover-size">Cover Size</label>
    <select
      id="cover-size"
      value={localSettings.coverSize}
      onchange={(e) => { localSettings.coverSize = (e.target as HTMLSelectElement).value as any; handleChange(); }}
      class="rounded-lg bg-gray-800 px-4 py-2 text-sm text-white outline-none focus:ring-2 focus:ring-blue-500 border border-gray-700"
    >
      <option value="small">Small</option>
      <option value="medium">Medium</option>
      <option value="large">Large</option>
    </select>
  </div>

  <label class="flex cursor-pointer items-center justify-between">
    <div>
      <span class="block text-sm font-medium text-white">Enable Analytics</span>
      <span class="text-xs text-gray-400">Help improve StreamFlow by sending anonymous usage data.</span>
    </div>
    <button
      type="button"
      class="relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 focus:ring-offset-gray-900 {localSettings.enableAnalytics ? 'bg-blue-600' : 'bg-gray-700'}"
      onclick={() => { localSettings.enableAnalytics = !localSettings.enableAnalytics; handleChange(); }}
      aria-label="Toggle enable analytics"
    >
      <span class="inline-block h-4 w-4 transform rounded-full bg-white transition-transform {localSettings.enableAnalytics ? 'translate-x-6' : 'translate-x-1'}"></span>
    </button>
  </label>
</div>
