<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { settingsStore } from '$lib/stores';

  let localSettings = $state({
    videoPlayer: settingsStore.settings?.videoPlayer ?? 'html5',
    mpvPath: settingsStore.settings?.mpvPath ?? '',
    vlcPath: settingsStore.settings?.vlcPath ?? '',
  });

  let debounceTimer: number | null = null;

  async function handleChange() {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = window.setTimeout(async () => {
      await settingsStore.update({
        videoPlayer: localSettings.videoPlayer as any,
        mpvPath: localSettings.mpvPath,
        vlcPath: localSettings.vlcPath,
      });
    }, 500);
  }

  $effect(() => {
    if (settingsStore.settings) {
      localSettings.videoPlayer = settingsStore.settings.videoPlayer;
      localSettings.mpvPath = settingsStore.settings.mpvPath ?? '';
      localSettings.vlcPath = settingsStore.settings.vlcPath ?? '';
    }
  });

  /** No extension filter - mpv/vlc have no `.exe` suffix outside Windows. */
  async function browseFor(target: 'mpv' | 'vlc') {
    const selected = await open({ multiple: false, directory: false });
    if (typeof selected !== 'string') return; // user cancelled
    if (target === 'mpv') {
      localSettings.mpvPath = selected;
    } else {
      localSettings.vlcPath = selected;
    }
    handleChange();
  }
</script>

<div class="space-y-6">
  <div>
    <label class="block text-sm font-medium text-white mb-2">
      Default Player
      <div class="flex flex-wrap gap-2">
      {#each ['html5', 'mpv', 'vlc'] as player}
        <button
          class={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
            localSettings.videoPlayer === player
              ? 'bg-blue-600 text-white'
              : 'bg-gray-800 text-gray-300 hover:bg-gray-700'
          }`}
          onclick={() => { localSettings.videoPlayer = player as any; handleChange(); }}
        >
          {player === 'html5' ? 'HTML5 Player' : player === 'mpv' ? 'MPV' : 'VLC'}
        </button>
      {/each}
      </div>
    </label>
    <p class="mt-2 text-xs text-gray-400">Choose your preferred video playback method.</p>
  </div>

  <div>
      <label class="block text-sm font-medium text-white mb-2" for="mpv-path">MPV Path</label>
    <div class="flex gap-2">
      <input
        type="text"
        id="mpv-path"
        placeholder="/usr/bin/mpv or C:\Program Files\mpv\mpv.exe"
        value={localSettings.mpvPath}
        oninput={(e) => { localSettings.mpvPath = (e.target as HTMLInputElement).value; handleChange(); }}
        class="flex-1 rounded-lg bg-gray-800 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-700"
      />
      <button class="rounded-lg bg-gray-700 px-4 py-2 text-sm text-gray-300 hover:bg-gray-600" aria-label="Browse MPV path" onclick={() => browseFor('mpv')}>
        Browse
      </button>
    </div>
    <p class="mt-2 text-xs text-gray-400">Path to the MPV executable. Leave empty to use system PATH.</p>
  </div>

  <div>
      <label class="block text-sm font-medium text-white mb-2" for="vlc-path">VLC Path</label>
    <div class="flex gap-2">
      <input
        type="text"
        id="vlc-path"
        placeholder="/usr/bin/vlc or C:\Program Files\VideoLAN\VLC\vlc.exe"
        value={localSettings.vlcPath}
        oninput={(e) => { localSettings.vlcPath = (e.target as HTMLInputElement).value; handleChange(); }}
        class="flex-1 rounded-lg bg-gray-800 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-700"
      />
      <button class="rounded-lg bg-gray-700 px-4 py-2 text-sm text-gray-300 hover:bg-gray-600" aria-label="Browse VLC path" onclick={() => browseFor('vlc')}>
        Browse
      </button>
    </div>
    <p class="mt-2 text-xs text-gray-400">Path to the VLC executable. Leave empty to use system PATH.</p>
  </div>
</div>
