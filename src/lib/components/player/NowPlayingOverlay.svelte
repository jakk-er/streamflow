<script lang="ts">
  import { page } from '$app/stores';
  import { playerStore } from '$lib/stores';
  import VideoPlayer from './VideoPlayer.svelte';
  import Icon from '$lib/components/ui/Icon.svelte';

  // The /live route embeds its own `<VideoPlayer>`/`<PlayerControls>` for live content;
  // showing this overlay too would mount a second mpegts.js/hls.js instance on the same
  // URL. The route check alone isn't enough since a VOD started elsewhere keeps playing
  // if the user navigates to /live, and that's not what the live page's player is for.
  const onLivePage = $derived($page.url.pathname.startsWith('/live'));
  const isLiveContent = $derived(playerStore.sourceKind === 'live');
  // External/mpv/vlc sessions play in their own OS window - nothing in-app to show.
  const isExternalSession = $derived(
    playerStore.playerType === 'mpv' || playerStore.playerType === 'vlc' || playerStore.playerType === 'external'
  );
  const visible = $derived(!!playerStore.currentUrl && !(onLivePage && isLiveContent) && !isExternalSession);
</script>

{#if visible}
  <div class="absolute inset-0 z-30 bg-black">
    <button
      class="absolute right-4 top-4 z-20 rounded-full bg-black/60 p-2 text-white transition-colors hover:bg-black/80"
      onclick={() => playerStore.stop()}
      aria-label="Close player"
    >
      <Icon name="X" size={20} />
    </button>
    <!-- PlayerControls renders inside VideoPlayer, not as a sibling here - fullscreen
         only shows the requested element's DOM descendants. -->
    <VideoPlayer />
  </div>
{/if}

{#if playerStore.lastError}
  <!-- Rendered here, not inside PlayerControls, since a failed external-player launch
       can leave `visible` false, which would unmount PlayerControls before showing the error. -->
  <div class="fixed bottom-4 right-4 z-50 max-w-sm rounded-lg bg-red-900/95 p-4 text-sm text-white shadow-lg">
    <div class="flex items-start justify-between gap-3">
      <span>{playerStore.lastError}</span>
      <button
        class="shrink-0 text-white/70 transition-colors hover:text-white"
        onclick={() => playerStore.clearError()}
        aria-label="Dismiss"
      >
        <Icon name="X" size={16} />
      </button>
    </div>
  </div>
{/if}
