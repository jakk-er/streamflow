<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { playlistStore, channelStore, playerStore, epgStore, favoritesStore, settingsStore, uiPrefsStore } from '$lib/stores';
  import { stalkerResolvePlayback } from '$lib/api';
  import { wrapUrlThroughStreamProxy } from '$lib/utils/streamProxy';
  import { registerShortcut, registerChannelNumber } from '$lib/utils/keyboard';
  import ChannelSearch from '$lib/components/channel/ChannelSearch.svelte';
  import ChannelList from '$lib/components/channel/ChannelList.svelte';
  import CategoryList from '$lib/components/channel/CategoryList.svelte';
  import ChannelNumberInput from '$lib/components/channel/ChannelNumberInput.svelte';
  import VideoPlayer from '$lib/components/player/VideoPlayer.svelte';
  import EpgRibbon from '$lib/components/epg/EpgRibbon.svelte';
  import Icon from '$lib/components/ui/Icon.svelte';

  let showNumberInput = $state(false);
  let numberBuffer = $state('');

  let unregisterShortcuts: (() => void)[] = [];

  /**
   * `sync_channels` stashes the portal's `use_http_tmp_link`/
   * `use_load_balancing` flags as JSON in `channel.raw` for Stalker channels
   * - evidence for whether `create_link` is actually needed. Some portals'
   * `create_link` silently drops the stream id from its returned URL, while
   * the row's own static `cmd` (what these flags say is already playable)
   * works correctly - so skip the round trip when the flags say we can.
   */
  function parseLinkFlags(raw: string | undefined): { useHttpTmpLink?: string; useLoadBalancing?: string } {
    if (!raw) return {};
    try {
      const parsed = JSON.parse(raw);
      return {
        useHttpTmpLink: parsed.use_http_tmp_link ?? undefined,
        useLoadBalancing: parsed.use_load_balancing ?? undefined,
      };
    } catch {
      return {};
    }
  }

  /**
   * Stalker channels store the portal's raw, unresolved `cmd` - only
   * `create_link` turns that into a fetchable URL, resolved at play time
   * since temporary links expire. M3U/Xtream channels already store a
   * playable URL. Either way the result is routed through the local proxy
   * (`wrapUrlThroughStreamProxy`) - see its doc comment for why.
   */
  async function resolvePlaybackUrl(
    channel: import('$lib/types').Channel
  ): Promise<{ url: string; extension?: string }> {
    const playlist = playlistStore.activePlaylist;
    if (!playlist) {
      return { url: channel.url };
    }
    try {
      let resolvedUrl = channel.url;
      if (playlist.playlistType === 'stalker') {
        const contentType = channel.radio === '1' ? 'radio' : 'itv';
        const { useHttpTmpLink, useLoadBalancing } = parseLinkFlags(channel.raw);
        resolvedUrl = await stalkerResolvePlayback(playlist._id, contentType, channel.url, useHttpTmpLink, useLoadBalancing);
      }
      return await wrapUrlThroughStreamProxy(playlist._id, resolvedUrl);
    } catch (err) {
      console.error('Failed to resolve playback URL:', err);
      return { url: channel.url };
    }
  }

  async function playChannel(channel: import('$lib/types').Channel) {
    channelStore.selectChannel(channel);
    const { url, extension } = await resolvePlaybackUrl(channel);
    playerStore.play(url, channel.name, undefined, extension, 'live');

    // Just logs "watched this channel" for history (0/0 = no progress bar,
    // see WatchHistoryItem's getProgress()). Failure is ignored - must never
    // block playback.
    const playlistId = playlistStore.activePlaylistId;
    if (playlistId) {
      favoritesStore.savePosition(channel.id, playlistId, 0, 0).catch(() => {});
    }
  }

  onMount(() => {
    (async () => {
      await playlistStore.loadPlaylists();
    })();

    const unregister1 = registerShortcut('arrowup', () => {
      const list = document.querySelector('[data-channel-list]');
      if (list) (list as HTMLElement).focus();
    });
    const unregister2 = registerShortcut('arrowdown', () => {
      const list = document.querySelector('[data-channel-list]');
      if (list) (list as HTMLElement).focus();
    });
    const unregister3 = registerShortcut('enter', () => {
      const active = channelStore.activeChannel;
      if (active) {
        playChannel(active);
      }
    });
    const unregister4 = registerShortcut('f', () => {
      playerStore.toggleFullscreen();
    });
    const unregister5 = registerShortcut('m', () => {
      // Placeholder: mute toggle not implemented yet.
    });
    const unregister6 = registerShortcut('space', () => {
      if (playerStore.currentUrl) {
        playerStore.stop();
      }
    });
    const unregister7 = registerShortcut('k', () => {
      if (playerStore.currentUrl) {
        playerStore.stop();
      }
    });

    unregisterShortcuts = [unregister1, unregister2, unregister3, unregister4, unregister5, unregister6, unregister7];

    const unregisterNumber = registerChannelNumber((num) => {
      numberBuffer = String(num);
      showNumberInput = true;
      setTimeout(() => {
        showNumberInput = false;
        const channel = channelStore.filteredChannels.find(c => c.channelNumber === num);
        if (channel) {
          playChannel(channel);
        }
      }, 500);
    });

    return () => {
      unregisterShortcuts.forEach(fn => fn());
      unregisterNumber();
      playerStore.stop();
    };
  });

  // On a playlist switch, a still-selected/playing channel belongs to the
  // OLD playlist and must be cleared/stopped, not just left running under a
  // now-different channel list. A background VOD movie is left alone here
  // (see `NowPlayingOverlay`'s `isLiveContent` check) - unrelated to live-TV
  // playlist switches.
  let previousPlaylistId: string | null = null;
  $effect(() => {
    const id = playlistStore.activePlaylistId;
    if (!id || id === previousPlaylistId) return;
    const isSwitch = previousPlaylistId !== null;
    previousPlaylistId = id;
    if (isSwitch) {
      if (playerStore.sourceKind === 'live') {
        playerStore.stop();
      }
      channelStore.selectChannel(null);
    }
    channelStore.loadChannels(id);
  });

  function handleChannelClick(channel: import('$lib/types').Channel) {
    playChannel(channel);
  }

  // Matches `NowPlayingOverlay.svelte`'s identical guard. Without checking
  // `playerType` too, the inline `<VideoPlayer>` stayed mounted and playing
  // even while an external mpv/VLC window was active - so when that window
  // closed, it looked like playback "resumed inline" when really the inline
  // player had never stopped.
  const isExternalSession = $derived(
    playerStore.playerType === 'mpv' || playerStore.playerType === 'vlc' || playerStore.playerType === 'external'
  );
</script>

<div class="flex h-full w-full flex-col lg:flex-row">
  <div class="flex w-full flex-col border-r border-border bg-surface lg:w-[300px] lg:min-w-[240px] lg:max-w-[400px]">
    <ChannelSearch />
      <div data-channel-list class="flex-1 overflow-hidden outline-none">
        {#if uiPrefsStore.browseLayout === 'categories'}
          <!-- Kept mounted (just hidden) rather than destroyed while drilled into a
               category or searching - unmounting would reset its scroll position, so
               "back" would always land at the top of the category list instead of
               wherever the user had scrolled to. -->
          <div class="h-full" class:hidden={!!channelStore.selectedGroup || !!channelStore.searchQuery.trim()}>
            <CategoryList onselect={(group) => channelStore.setGroupFilter(group)} />
          </div>
          {#if channelStore.selectedGroup}
            <div class="flex h-full flex-col">
              <button
                class="flex items-center gap-1.5 border-b border-border px-3 py-2 text-sm text-text-secondary hover:text-text-primary transition-all duration-200"
                onclick={() => channelStore.setGroupFilter(null)}
              >
                <Icon name="ChevronLeft" size={16} />
                <span class="truncate">{channelStore.selectedGroup}</span>
              </button>
              <div class="min-h-0 flex-1">
                <ChannelList onchannelclick={(channel) => handleChannelClick(channel)} />
              </div>
            </div>
          {:else if channelStore.searchQuery.trim()}
            <ChannelList onchannelclick={(channel) => handleChannelClick(channel)} />
          {/if}
        {:else}
          <ChannelList onchannelclick={(channel) => handleChannelClick(channel)} />
        {/if}
      </div>
  </div>

  <div class="flex flex-1 flex-col bg-background">
    <div class="relative min-h-0 flex-[2]">
      {#if playerStore.currentUrl && playerStore.sourceKind === 'live' && !isExternalSession}
        <!-- `PlayerControls` is rendered inside `VideoPlayer` itself now (see
             its own comment) - not here as a sibling - so it's still part of
             the DOM subtree that actually goes fullscreen. -->
        <VideoPlayer />
      {:else if playerStore.currentUrl && playerStore.sourceKind === 'live' && isExternalSession}
        <!-- Playing in mpv/VLC as a real separate window - showing the
             "select a channel" placeholder here would be misleading, since
             a channel IS actively playing (just not in this pane). -->
        <div class="flex h-full items-center justify-center text-text-muted">
          <div class="text-center">
            <div class="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-surface-elevated">
              <Icon name="ExternalLink" size={32} class="text-text-muted" />
            </div>
            <p class="text-lg font-medium text-text-primary">Playing in {playerStore.playerType === 'vlc' ? 'VLC' : 'mpv'}</p>
            <p class="mt-2 text-sm text-text-muted">{playerStore.currentTitle ?? 'This channel'} is playing in an external window.</p>
          </div>
        </div>
      {:else}
        <div class="flex h-full items-center justify-center text-text-muted">
          <div class="text-center">
            <div class="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-surface-elevated">
              <Icon name="Play" size={32} class="text-text-muted" />
            </div>
            <p class="text-lg font-medium text-text-primary">Select a channel to start watching</p>
            <p class="mt-2 text-sm text-text-muted">Choose a playlist from the sidebar</p>
          </div>
        </div>
      {/if}
    </div>

    {#if playlistStore.activePlaylistId}
      <div class="min-h-0 flex-1">
        <EpgRibbon />
      </div>
    {/if}
  </div>

  {#if showNumberInput}
    <ChannelNumberInput digits={numberBuffer} />
  {/if}
</div>
