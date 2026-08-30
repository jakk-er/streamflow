<script lang="ts">
  import { onMount } from 'svelte';
  import { playlistStore, settingsStore } from '$lib/stores';
  import { goto } from '$app/navigation';
  import { formatError } from '$lib/utils/errors';
  import StalkerLoginForm from '$lib/components/stalker/StalkerLoginForm.svelte';
  import { stalkerDeriveDeviceIds } from '$lib/api';
  import type { Playlist } from '$lib/types';

  let activeTab = $state<'m3u' | 'xtream' | 'stalker'>('m3u');
  let showAddForm = $state(false);

  // A link like the dashboard's "Add Playlist" button
  // (`/settings?tab=playlists&action=add`) should open the form directly,
  // not just land on the section and leave the user to click Add again.
  onMount(() => {
    if (new URL(window.location.href).searchParams.get('action') === 'add') {
      showAddForm = true;
    }
  });
  let error = $state<string | null>(null);
  /** Non-null while editing an existing playlist rather than adding a new
   * one - the same forms below are reused for both, keyed off this. */
  let editingPlaylistId = $state<string | null>(null);
  let isEditing = $derived(editingPlaylistId !== null);

  let m3uUrl = $state('');
  let m3uTitle = $state('');
  let m3uUserAgent = $state('');
  let m3uAutoRefresh = $state(false);

  let xtreamUrl = $state('');
  let xtreamUsername = $state('');
  let xtreamPassword = $state('');
  let xtreamTitle = $state('');
  let xtreamUserAgent = $state('');
  let xtreamAutoRefresh = $state(false);

  let stalkerUrl = $state('');
  let stalkerMac = $state('');
  let stalkerTitle = $state('');
  let stalkerUserAgent = $state('');
  let stalkerAutoRefresh = $state(false);
  let stalkerUsername = $state('');
  let stalkerPassword = $state('');
  let stalkerDeviceId1 = $state('');
  let stalkerDeviceId2 = $state('');
  let stalkerSerialNumber = $state('');
  let stalkerSignature1 = $state('');
  let stalkerSignature2 = $state('');
  let showStalkerAdvanced = $state(false);
  let pendingStalkerLoginPlaylistId = $state<string | null>(null);

  async function handleAddM3u() {
    error = null;
    try {
      if (editingPlaylistId) {
        await playlistStore.updateM3u(editingPlaylistId, m3uUrl, m3uTitle || 'M3U Playlist', m3uUserAgent || undefined, m3uAutoRefresh);
      } else {
        await playlistStore.importM3u(m3uUrl, m3uTitle || 'M3U Playlist', m3uUserAgent || undefined, m3uAutoRefresh);
      }
      resetForms();
    } catch (err) {
      error = formatError(err);
    }
  }

  async function handleAddXtream() {
    error = null;
    try {
      if (editingPlaylistId) {
        await playlistStore.updateXtream(editingPlaylistId, xtreamUrl, xtreamUsername, xtreamPassword, xtreamTitle || 'Xtream Playlist', xtreamUserAgent || undefined, xtreamAutoRefresh);
      } else {
        await playlistStore.addXtream(xtreamUrl, xtreamUsername, xtreamPassword, xtreamTitle || 'Xtream Playlist', xtreamUserAgent || undefined, xtreamAutoRefresh);
      }
      resetForms();
    } catch (err) {
      error = formatError(err);
    }
  }

  async function handleAddStalker() {
    error = null;
    try {
      const result = editingPlaylistId
        ? await playlistStore.updateStalker(
            editingPlaylistId,
            stalkerUrl,
            stalkerMac,
            stalkerTitle || 'Stalker Playlist',
            stalkerUserAgent || undefined,
            stalkerUsername || undefined,
            stalkerPassword || undefined,
            stalkerDeviceId1 || undefined,
            stalkerDeviceId2 || undefined,
            stalkerSerialNumber || undefined,
            stalkerSignature1 || undefined,
            stalkerSignature2 || undefined,
            stalkerAutoRefresh
          )
        : await playlistStore.addStalker(
            stalkerUrl,
            stalkerMac,
            stalkerTitle || 'Stalker Playlist',
            stalkerUserAgent || undefined,
            stalkerUsername || undefined,
            stalkerPassword || undefined,
            stalkerDeviceId1 || undefined,
            stalkerDeviceId2 || undefined,
            stalkerSerialNumber || undefined,
            stalkerSignature1 || undefined,
            stalkerSignature2 || undefined,
            stalkerAutoRefresh
          );
      if (result.outcome.kind === 'loginRequired') {
        // The playlist row exists; keep the form open and switch to asking
        // for credentials instead of treating this as a failure.
        pendingStalkerLoginPlaylistId = result.playlist._id;
      } else if (result.outcome.kind !== 'success') {
        error = result.outcome.message;
      } else {
        resetForms();
      }
    } catch (err) {
      error = formatError(err);
    }
  }

  let deviceIdDeriveError = $state<string | null>(null);

  /** Opt-in prefill only, never automatic - disabled once a device ID is typed by hand. */
  async function handleDeriveDeviceIds() {
    deviceIdDeriveError = null;
    const derived = await stalkerDeriveDeviceIds(stalkerMac);
    if (!derived) {
      deviceIdDeriveError = 'Enter a valid MAC address first.';
      return;
    }
    [stalkerDeviceId1, stalkerDeviceId2] = derived;
  }

  function handleEdit(playlist: Playlist) {
    error = null;
    pendingStalkerLoginPlaylistId = null;
    editingPlaylistId = playlist._id;
    activeTab = playlist.playlistType as 'm3u' | 'xtream' | 'stalker';
    if (playlist.playlistType === 'm3u') {
      m3uUrl = playlist.url ?? '';
      m3uTitle = playlist.title ?? '';
      m3uUserAgent = playlist.userAgent ?? '';
      m3uAutoRefresh = playlist.autoRefresh ?? false;
    } else if (playlist.playlistType === 'xtream') {
      xtreamUrl = playlist.serverUrl ?? '';
      xtreamUsername = playlist.username ?? '';
      xtreamPassword = playlist.password ?? '';
      xtreamTitle = playlist.title ?? '';
      xtreamUserAgent = playlist.userAgent ?? '';
      xtreamAutoRefresh = playlist.autoRefresh ?? false;
    } else if (playlist.playlistType === 'stalker') {
      stalkerUrl = playlist.portalUrl ?? '';
      stalkerMac = playlist.macAddress ?? '';
      stalkerTitle = playlist.title ?? '';
      stalkerUserAgent = playlist.userAgent ?? '';
      stalkerAutoRefresh = playlist.autoRefresh ?? false;
      stalkerUsername = playlist.username ?? '';
      stalkerPassword = playlist.password ?? '';
      stalkerDeviceId1 = playlist.stalkerDeviceId1 ?? '';
      stalkerDeviceId2 = playlist.stalkerDeviceId2 ?? '';
      stalkerSerialNumber = playlist.stalkerSerialNumber ?? '';
      stalkerSignature1 = playlist.stalkerSignature1 ?? '';
      stalkerSignature2 = playlist.stalkerSignature2 ?? '';
      showStalkerAdvanced = Boolean(
        stalkerDeviceId1 || stalkerDeviceId2 || stalkerSerialNumber || stalkerSignature1 || stalkerSignature2
      );
    }
    showAddForm = true;
  }

  async function handleStalkerLoginSuccess() {
    await playlistStore.loadPlaylists();
    resetForms();
  }

  async function handleRefresh(id: string) {
    await playlistStore.refreshPlaylist(id);
  }

  async function handleDelete(id: string) {
    await playlistStore.deletePlaylist(id);
  }

  /** Sets the playlist auto-activated on launch (see `+layout.svelte` `onMount`) - a single
   * settings field, so setting a new default implicitly clears the old one. */
  async function handleToggleDefault(id: string) {
    const isDefault = settingsStore.settings?.defaultPlaylistId === id;
    await settingsStore.update({ defaultPlaylistId: isDefault ? undefined : id });
  }

  function handlePlaylistClick(id: string) {
    playlistStore.setActive(id);
    goto('/live');
  }

  function resetForms() {
    m3uUrl = '';
    m3uTitle = '';
    m3uUserAgent = '';
    m3uAutoRefresh = false;
    xtreamUrl = '';
    xtreamUsername = '';
    xtreamPassword = '';
    xtreamTitle = '';
    xtreamUserAgent = '';
    xtreamAutoRefresh = false;
    stalkerUrl = '';
    stalkerMac = '';
    stalkerTitle = '';
    stalkerUserAgent = '';
    stalkerAutoRefresh = false;
    stalkerUsername = '';
    stalkerPassword = '';
    stalkerDeviceId1 = '';
    stalkerDeviceId2 = '';
    deviceIdDeriveError = null;
    stalkerSerialNumber = '';
    stalkerSignature1 = '';
    stalkerSignature2 = '';
    showStalkerAdvanced = false;
    pendingStalkerLoginPlaylistId = null;
    showAddForm = false;
    activeTab = 'm3u';
    editingPlaylistId = null;
  }

  function getPlaylistTypeLabel(type: string | undefined) {
    switch (type) {
      case 'm3u': return 'M3U';
      case 'xtream': return 'Xtream';
      case 'stalker': return 'Stalker';
      default: return type || 'Unknown';
    }
  }
</script>

<div class="space-y-4">
  <div class="flex items-center justify-between">
    <h3 class="text-lg font-semibold text-white">Your Playlists</h3>
    <button
      class="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors"
      onclick={() => {
        if (showAddForm) {
          resetForms();
        } else {
          editingPlaylistId = null;
          showAddForm = true;
          error = null;
        }
      }}
    >
      {showAddForm ? 'Cancel' : '+ Add Playlist'}
    </button>
  </div>

  {#if showAddForm}
    <div class="relative rounded-lg bg-gray-800 p-4 border border-gray-700">
      <div class="transition-opacity" class:opacity-40={playlistStore.loading} class:pointer-events-none={playlistStore.loading}>
      {#if error}
        <div class="mb-4 rounded-lg bg-red-900/50 border border-red-700 p-3 text-sm text-red-300">
          {error}
        </div>
      {/if}

      {#if isEditing}
        <p class="mb-4 text-sm font-medium text-white">
          Editing {getPlaylistTypeLabel(activeTab)} playlist
        </p>
      {:else}
        <div class="flex gap-2 mb-4">
          <button
            class={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
              activeTab === 'm3u'
                ? 'bg-blue-600 text-white'
                : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
            }`}
            onclick={() => { activeTab = 'm3u'; error = null; }}
          >
            M3U URL
          </button>
          <button
            class={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
              activeTab === 'xtream'
                ? 'bg-blue-600 text-white'
                : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
            }`}
            onclick={() => { activeTab = 'xtream'; error = null; }}
          >
            Xtream Codes
          </button>
          <button
            class={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
              activeTab === 'stalker'
                ? 'bg-blue-600 text-white'
                : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
            }`}
            onclick={() => { activeTab = 'stalker'; error = null; }}
          >
            Stalker Portal
          </button>
        </div>
      {/if}

      {#if activeTab === 'm3u'}
        <div class="space-y-4">
          <div>
            <label class="block text-sm font-medium text-white mb-1" for="m3u-url">Playlist URL *</label>
            <input
              type="url"
              id="m3u-url"
              placeholder="http://example.com/playlist.m3u"
              value={m3uUrl}
              oninput={(e) => (m3uUrl = (e.target as HTMLInputElement).value)}
              class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-white mb-1" for="m3u-title">Title</label>
            <input
              type="text"
              id="m3u-title"
              placeholder="My Playlist"
              value={m3uTitle}
              oninput={(e) => (m3uTitle = (e.target as HTMLInputElement).value)}
              class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-white mb-1" for="m3u-useragent">User-Agent (optional)</label>
            <input
              type="text"
              id="m3u-useragent"
              placeholder="Mozilla/5.0..."
              value={m3uUserAgent}
              oninput={(e) => (m3uUserAgent = (e.target as HTMLInputElement).value)}
              class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
            />
          </div>
          <label class="flex items-start gap-2 text-sm text-white" for="m3u-auto-refresh">
            <input
              type="checkbox"
              id="m3u-auto-refresh"
              checked={m3uAutoRefresh}
              onchange={(e) => (m3uAutoRefresh = (e.target as HTMLInputElement).checked)}
              class="mt-0.5 h-4 w-4 rounded border-gray-600 bg-gray-700 text-blue-600 focus:ring-blue-500"
            />
            <span>
              Auto-refresh daily
              <span class="block text-xs text-gray-400">Re-syncs once every 24 hours, at the same time of day this playlist was added.</span>
            </span>
          </label>
          <button
            class="w-full rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            onclick={handleAddM3u}
            disabled={!m3uUrl.trim()}
          >
            {isEditing ? 'Save Changes' : 'Import M3U Playlist'}
          </button>
        </div>
      {:else if activeTab === 'xtream'}
        <div class="space-y-4">
          <div>
            <label class="block text-sm font-medium text-white mb-1" for="xtream-url">Server URL *</label>
            <input
              type="url"
              id="xtream-url"
              placeholder="http://example.com:8080"
              value={xtreamUrl}
              oninput={(e) => (xtreamUrl = (e.target as HTMLInputElement).value)}
              class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
            />
          </div>
          <div class="grid grid-cols-2 gap-4">
            <div>
              <label class="block text-sm font-medium text-white mb-1" for="xtream-username">Username *</label>
              <input
                type="text"
                id="xtream-username"
                value={xtreamUsername}
                oninput={(e) => (xtreamUsername = (e.target as HTMLInputElement).value)}
                class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
              />
            </div>
            <div>
              <label class="block text-sm font-medium text-white mb-1" for="xtream-password">Password *</label>
              <input
                type="password"
                id="xtream-password"
                value={xtreamPassword}
                oninput={(e) => (xtreamPassword = (e.target as HTMLInputElement).value)}
                class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
              />
            </div>
          </div>
          <div>
            <label class="block text-sm font-medium text-white mb-1" for="xtream-title">Title</label>
            <input
              type="text"
              id="xtream-title"
              placeholder="My Xtream Playlist"
              value={xtreamTitle}
              oninput={(e) => (xtreamTitle = (e.target as HTMLInputElement).value)}
              class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-white mb-1" for="xtream-useragent">User-Agent (optional)</label>
            <input
              type="text"
              id="xtream-useragent"
              placeholder="Mozilla/5.0..."
              value={xtreamUserAgent}
              oninput={(e) => (xtreamUserAgent = (e.target as HTMLInputElement).value)}
              class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
            />
          </div>
          <label class="flex items-start gap-2 text-sm text-white" for="xtream-auto-refresh">
            <input
              type="checkbox"
              id="xtream-auto-refresh"
              checked={xtreamAutoRefresh}
              onchange={(e) => (xtreamAutoRefresh = (e.target as HTMLInputElement).checked)}
              class="mt-0.5 h-4 w-4 rounded border-gray-600 bg-gray-700 text-blue-600 focus:ring-blue-500"
            />
            <span>
              Auto-refresh daily
              <span class="block text-xs text-gray-400">Re-syncs channels and the VOD/series catalog once every 24 hours, at the same time of day this playlist was added.</span>
            </span>
          </label>
          <button
            class="w-full rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            onclick={handleAddXtream}
            disabled={!xtreamUrl.trim() || !xtreamUsername.trim() || !xtreamPassword.trim()}
          >
            {isEditing ? 'Save Changes' : 'Add Xtream Playlist'}
          </button>
        </div>
      {:else if activeTab === 'stalker'}
        {#if pendingStalkerLoginPlaylistId}
          <StalkerLoginForm
            playlistId={pendingStalkerLoginPlaylistId}
            onsuccess={handleStalkerLoginSuccess}
            oncancel={resetForms}
          />
        {:else}
          <div class="space-y-4">
            <div>
              <label class="block text-sm font-medium text-white mb-1" for="stalker-url">Server URL *</label>
              <input
                type="url"
                id="stalker-url"
                placeholder="http://example.com:8080"
                value={stalkerUrl}
                oninput={(e) => (stalkerUrl = (e.target as HTMLInputElement).value)}
                class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
              />
            </div>
            <div>
              <label class="block text-sm font-medium text-white mb-1" for="stalker-mac">MAC Address *</label>
              <input
                type="text"
                id="stalker-mac"
                placeholder="00:1A:79:XX:XX:XX"
                value={stalkerMac}
                oninput={(e) => (stalkerMac = (e.target as HTMLInputElement).value)}
                class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
              />
            </div>
            <div>
              <label class="block text-sm font-medium text-white mb-1" for="stalker-title">Title</label>
              <input
                type="text"
                id="stalker-title"
                placeholder="My Stalker Playlist"
                value={stalkerTitle}
                oninput={(e) => (stalkerTitle = (e.target as HTMLInputElement).value)}
                class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
              />
            </div>
            <div class="grid grid-cols-2 gap-4">
              <div>
                <label class="block text-sm font-medium text-white mb-1" for="stalker-username">Username (if required)</label>
                <input
                  type="text"
                  id="stalker-username"
                  value={stalkerUsername}
                  oninput={(e) => (stalkerUsername = (e.target as HTMLInputElement).value)}
                  class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
                />
              </div>
              <div>
                <label class="block text-sm font-medium text-white mb-1" for="stalker-password">Password (if required)</label>
                <input
                  type="password"
                  id="stalker-password"
                  value={stalkerPassword}
                  oninput={(e) => (stalkerPassword = (e.target as HTMLInputElement).value)}
                  class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
                />
              </div>
            </div>
            <div>
              <label class="block text-sm font-medium text-white mb-1" for="stalker-useragent">User-Agent (optional)</label>
              <input
                type="text"
                id="stalker-useragent"
                placeholder="Mozilla/5.0..."
                value={stalkerUserAgent}
                oninput={(e) => (stalkerUserAgent = (e.target as HTMLInputElement).value)}
                class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
              />
            </div>
            <label class="flex items-start gap-2 text-sm text-white" for="stalker-auto-refresh">
              <input
                type="checkbox"
                id="stalker-auto-refresh"
                checked={stalkerAutoRefresh}
                onchange={(e) => (stalkerAutoRefresh = (e.target as HTMLInputElement).checked)}
                class="mt-0.5 h-4 w-4 rounded border-gray-600 bg-gray-700 text-blue-600 focus:ring-blue-500"
              />
              <span>
                Auto-refresh daily
                <span class="block text-xs text-gray-400">Re-syncs channels and the VOD/series catalog once every 24 hours, at the same time of day this playlist was added.</span>
              </span>
            </label>

            <button
              type="button"
              class="text-sm text-blue-400 hover:text-blue-300 transition-colors"
              onclick={() => (showStalkerAdvanced = !showStalkerAdvanced)}
            >
              {showStalkerAdvanced ? '▾' : '▸'} Advanced device identity (optional)
            </button>
            {#if showStalkerAdvanced}
              <div class="space-y-4 rounded-lg border border-gray-700 p-3">
                <p class="text-xs text-gray-400">
                  Some portals bind an account to more than the MAC address. Leave these blank
                  unless your provider gave you specific values.
                </p>
                <div>
                  <button
                    type="button"
                    class="text-xs text-blue-400 hover:text-blue-300 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    onclick={handleDeriveDeviceIds}
                    disabled={!stalkerMac.trim() || Boolean(stalkerDeviceId1 || stalkerDeviceId2)}
                    title={stalkerDeviceId1 || stalkerDeviceId2 ? 'Clear Device ID 1/2 first to re-derive' : 'Derive Device ID 1 and 2 from the MAC address above (StbEmu-compatible) — only use this if your provider told you to'}
                  >
                    Derive Device ID 1/2 from MAC (StbEmu-compatible)
                  </button>
                  {#if deviceIdDeriveError}
                    <p class="text-xs text-red-400 mt-1">{deviceIdDeriveError}</p>
                  {/if}
                </div>
                <div class="grid grid-cols-2 gap-4">
                  <div>
                    <label class="block text-sm font-medium text-white mb-1" for="stalker-serial">Serial Number</label>
                    <input
                      type="text"
                      id="stalker-serial"
                      value={stalkerSerialNumber}
                      oninput={(e) => (stalkerSerialNumber = (e.target as HTMLInputElement).value)}
                      class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
                    />
                  </div>
                  <div>
                    <label class="block text-sm font-medium text-white mb-1" for="stalker-device-id2">Device ID 2</label>
                    <input
                      type="text"
                      id="stalker-device-id2"
                      value={stalkerDeviceId2}
                      oninput={(e) => (stalkerDeviceId2 = (e.target as HTMLInputElement).value)}
                      class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
                    />
                  </div>
                </div>
                <div class="grid grid-cols-2 gap-4">
                  <div>
                    <label class="block text-sm font-medium text-white mb-1" for="stalker-device-id1">Device ID 1</label>
                    <input
                      type="text"
                      id="stalker-device-id1"
                      value={stalkerDeviceId1}
                      oninput={(e) => (stalkerDeviceId1 = (e.target as HTMLInputElement).value)}
                      class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
                    />
                  </div>
                  <div>
                    <label class="block text-sm font-medium text-white mb-1" for="stalker-signature1">Signature 1</label>
                    <input
                      type="text"
                      id="stalker-signature1"
                      value={stalkerSignature1}
                      oninput={(e) => (stalkerSignature1 = (e.target as HTMLInputElement).value)}
                      class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
                    />
                  </div>
                </div>
                <div>
                  <label class="block text-sm font-medium text-white mb-1" for="stalker-signature2">Signature 2</label>
                  <input
                    type="text"
                    id="stalker-signature2"
                    value={stalkerSignature2}
                    oninput={(e) => (stalkerSignature2 = (e.target as HTMLInputElement).value)}
                    class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
                  />
                </div>
              </div>
            {/if}

            <button
              class="w-full rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              onclick={handleAddStalker}
              disabled={!stalkerUrl.trim() || !stalkerMac.trim()}
            >
              {isEditing ? 'Save Changes' : 'Add Stalker Playlist'}
            </button>
          </div>
        {/if}
      {/if}
      </div>

      {#if playlistStore.loading}
        <div class="absolute inset-0 z-10 flex items-center justify-center rounded-lg">
          <div class="h-8 w-8 animate-spin rounded-full border-4 border-gray-700 border-t-blue-500"></div>
        </div>
      {/if}
    </div>
  {/if}

  <div class="relative min-h-[72px]">
    {#if playlistStore.playlists.length === 0}
      {#if !playlistStore.loading}
        <div class="text-center py-8 text-gray-400 text-sm">
          No playlists added yet. Add your first playlist above.
        </div>
      {/if}
    {:else}
    <div class="space-y-2 transition-opacity" class:opacity-40={playlistStore.loading} class:pointer-events-none={playlistStore.loading}>
      {#each playlistStore.playlists as playlist}
        <div class="flex items-center gap-4 rounded-lg bg-gray-800 p-4 hover:bg-gray-750 transition-colors">
          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2">
              <h4 class="text-sm font-medium text-white truncate">{playlist.title}</h4>
              <span class="flex-shrink-0 rounded bg-gray-700 px-2 py-0.5 text-xs text-gray-300">
                {getPlaylistTypeLabel(playlist.playlistType)}
              </span>
              {#if settingsStore.settings?.defaultPlaylistId === playlist._id}
                <span class="flex-shrink-0 rounded bg-blue-900/50 px-2 py-0.5 text-xs text-blue-300">
                  Default
                </span>
              {/if}
            </div>
            <p class="mt-1 text-xs text-gray-400 truncate">
              {playlist.serverUrl || playlist.portalUrl || playlist.url || 'No URL'}
            </p>
          </div>

          <div class="flex items-center gap-2 flex-shrink-0">
            <button
              class={`p-2 rounded-lg transition-colors ${
                settingsStore.settings?.defaultPlaylistId === playlist._id
                  ? 'text-blue-400 hover:text-blue-300 hover:bg-gray-700'
                  : 'text-gray-400 hover:text-white hover:bg-gray-700'
              }`}
              onclick={() => handleToggleDefault(playlist._id)}
              aria-label={settingsStore.settings?.defaultPlaylistId === playlist._id ? 'Unset as default playlist' : 'Set as default playlist'}
              title={settingsStore.settings?.defaultPlaylistId === playlist._id ? 'Default playlist - click to unset' : 'Set as default playlist (auto-selected on launch)'}
            >
              <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" viewBox="0 0 24 24" fill={settingsStore.settings?.defaultPlaylistId === playlist._id ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M11.48 3.499a.562.562 0 011.04 0l2.125 5.111a.563.563 0 00.475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 00-.182.557l1.285 5.385a.562.562 0 01-.84.61l-4.725-2.885a.562.562 0 00-.586 0L6.982 21.54a.562.562 0 01-.84-.61l1.285-5.386a.562.562 0 00-.182-.557l-4.204-3.602a.563.563 0 01.321-.988l5.518-.442a.563.563 0 00.475-.345L11.48 3.5z" />
              </svg>
            </button>
            <button
              class="p-2 rounded-lg text-gray-400 hover:text-white hover:bg-gray-700 transition-colors"
              onclick={() => handleEdit(playlist)}
              aria-label="Edit playlist"
              title="Edit"
            >
              <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
              </svg>
            </button>
            <button
              class="p-2 rounded-lg text-gray-400 hover:text-white hover:bg-gray-700 transition-colors"
              onclick={() => handleRefresh(playlist._id)}
              aria-label="Refresh playlist"
              title="Refresh"
            >
              <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
            </button>
            <button
              class="p-2 rounded-lg text-gray-400 hover:text-red-400 hover:bg-gray-700 transition-colors"
              onclick={() => handleDelete(playlist._id)}
              aria-label="Delete playlist"
              title="Delete"
            >
              <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </button>
            <button
              class="px-3 py-1.5 rounded-lg text-xs font-medium bg-gray-700 text-gray-300 hover:bg-gray-600 transition-colors"
              onclick={() => handlePlaylistClick(playlist._id)}
            >
              Open
            </button>
          </div>
        </div>
      {/each}
    </div>
    {/if}

    {#if playlistStore.loading && !showAddForm}
      <div class="absolute inset-0 z-10 flex items-center justify-center">
        <div class="h-8 w-8 animate-spin rounded-full border-4 border-gray-700 border-t-blue-500"></div>
      </div>
    {/if}
  </div>
</div>
