<script lang="ts">
  import { channelStore, epgStore, playlistStore, playerStore } from '$lib/stores';
  import { m3uIsCatchupSupported, m3uResolveCatchupUrl, xtreamChannelCatchupAvailable, xtreamResolveCatchupUrlForChannel } from '$lib/api';
  import { wrapUrlThroughStreamProxy } from '$lib/utils/streamProxy';
  import Badge from '$lib/components/ui/Badge.svelte';
  import Icon from '$lib/components/ui/Icon.svelte';

  let activeChannel = $derived(channelStore.activeChannel);
  let currentProgram = $derived(epgStore.currentProgram);

  let catchupSupported = $state(false);
  let resolvingCatchup = $state(false);

  $effect(() => {
    if (activeChannel) {
      epgStore.loadCurrent(activeChannel.id);
    }
  });

  // M3U and Xtream are separate catch-up protocols (see `catchup::resolve_m3u_catchup_url`/
  // `resolve_xtream_catchup_url`); both are checked, either saying yes is enough. Stalker
  // channels have no catch-up path wired up yet, so they never qualify.
  $effect(() => {
    const channel = activeChannel;
    if (!channel) {
      catchupSupported = false;
      return;
    }
    let cancelled = false;
    (async () => {
      const [m3uSupported, xtreamSupported] = await Promise.all([
        m3uIsCatchupSupported(channel.id).catch(() => false),
        xtreamChannelCatchupAvailable(channel.id).catch(() => false),
      ]);
      if (!cancelled) {
        catchupSupported = m3uSupported || xtreamSupported;
      }
    })();
    return () => {
      cancelled = true;
    };
  });

  function formatTime(dateStr: string) {
    const date = new Date(dateStr);
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }

  function getProgress(start: string, stop: string) {
    const startMs = new Date(start).getTime();
    const stopMs = new Date(stop).getTime();
    const nowMs = Date.now();
    if (stopMs <= startMs) return 0;
    return Math.min(100, Math.max(0, ((nowMs - startMs) / (stopMs - startMs)) * 100));
  }

  async function watchFromStart() {
    const channel = activeChannel;
    const program = currentProgram;
    const playlist = playlistStore.activePlaylist;
    if (!channel || !program || !playlist || resolvingCatchup) return;

    resolvingCatchup = true;
    try {
      let resolvedUrl: string | null = null;
      if (playlist.playlistType === 'xtream') {
        const startTimestamp = Math.floor(new Date(program.start).getTime() / 1000);
        const stopTimestamp = Math.floor(new Date(program.stop).getTime() / 1000);
        resolvedUrl = await xtreamResolveCatchupUrlForChannel(channel.id, startTimestamp, stopTimestamp);
      }
      // Falls through to the M3U resolver if Xtream didn't apply or returned null (not
      // every Xtream channel has archive metadata); it's a no-op for channels with no
      // catchup/timeshift/tvg-rec attributes, so trying costs nothing.
      if (!resolvedUrl) {
        resolvedUrl = await m3uResolveCatchupUrl(channel.id, program.start);
      }
      if (!resolvedUrl) {
        return;
      }
      const { url, extension } = await wrapUrlThroughStreamProxy(playlist._id, resolvedUrl, true);
      // `kind: 'vod'` (not 'live') so the seek bar shows - catch-up is a bounded replay,
      // not an open-ended feed, even though it plays through the live-page player.
      playerStore.play(url, `${channel.name} — ${program.title}`, undefined, extension, 'vod');
    } catch (err) {
      console.error('Failed to resolve catch-up URL:', err);
    } finally {
      resolvingCatchup = false;
    }
  }
</script>

<div class="flex h-full flex-col overflow-y-auto border-t border-border bg-surface p-4">
  {#if !activeChannel}
    <div class="flex flex-1 items-center justify-center text-sm text-text-muted">
      Select a channel to see program info
    </div>
  {:else if currentProgram}
    <div class="flex items-start gap-3">
      <span class="mt-1.5 inline-flex h-2 w-2 flex-shrink-0 rounded-full bg-error shadow-[0_0_8px_rgba(239,68,68,0.6)]"></span>
      <div class="min-w-0 flex-1">
        <div class="text-xs text-text-muted font-mono">
          {formatTime(currentProgram.start)} - {formatTime(currentProgram.stop)}
        </div>
        <h3 class="mt-1 truncate text-base font-semibold text-text-primary">{currentProgram.title}</h3>
        {#if currentProgram.category}
          <Badge variant="default" class="mt-2">{currentProgram.category}</Badge>
        {/if}
        {#if currentProgram.description}
          <p class="mt-2 text-sm text-text-secondary">{currentProgram.description}</p>
        {/if}
        {#if catchupSupported}
          <button
            class="mt-3 inline-flex items-center gap-1.5 rounded-lg bg-surface-elevated px-3 py-1.5 text-xs font-medium text-text-primary hover:bg-primary hover:text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            onclick={watchFromStart}
            disabled={resolvingCatchup}
          >
            <Icon name="Rewind" size={14} />
            {resolvingCatchup ? 'Loading...' : 'Watch from Start'}
          </button>
        {/if}
      </div>
      {#if currentProgram.icon}
        <img src={currentProgram.icon} alt="" class="h-14 w-14 flex-shrink-0 rounded object-cover" />
      {/if}
    </div>
    <div class="mt-3 h-1 w-full overflow-hidden rounded-full bg-surface-elevated">
      <div
        class="h-full bg-primary"
        style="width: {getProgress(currentProgram.start, currentProgram.stop)}%"
      ></div>
    </div>
  {:else}
    <div class="flex flex-1 items-center justify-center text-sm text-text-muted">
      No EPG data available
    </div>
  {/if}
</div>
