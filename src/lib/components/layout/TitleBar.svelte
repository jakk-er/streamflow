<script lang="ts">
  import { settingsStore } from '$lib/stores';
  import Icon from '$lib/components/ui/Icon.svelte';
  import Button from '$lib/components/ui/Button.svelte';

  let { onToggleSidebar }: { onToggleSidebar?: () => void } = $props();

  async function minimize() {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const appWindow = getCurrentWindow();
      await appWindow.minimize();
    } catch {
      // Tauri API not available (e.g. a plain browser dev preview) - no
      // window chrome to control, silently do nothing.
    }
  }

  async function toggleMaximize() {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const appWindow = getCurrentWindow();
      await appWindow.toggleMaximize();
    } catch {
      // Tauri API not available (e.g. a plain browser dev preview) - no
      // window chrome to control, silently do nothing.
    }
  }

  async function close() {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const appWindow = getCurrentWindow();
      await appWindow.close();
    } catch {
      // Tauri API not available (e.g. a plain browser dev preview) - no
      // window chrome to control, silently do nothing.
    }
  }

  const isMac = typeof navigator !== 'undefined' && /mac|iphone|ipad|ipod/.test(navigator.platform.toLowerCase());
  const titleBarHeight = isMac ? 'h-8' : 'h-10';
</script>

<div
  class={`glass glass-background flex items-center justify-between ${titleBarHeight} border-b border-border px-2 select-none`}
  data-tauri-drag-region
>
  <div class="flex items-center gap-2" data-tauri-drag-region>
    <button
      class="rounded-lg p-1.5 text-text-secondary hover:text-text-primary hover:bg-surface-hover transition-all duration-200"
      onclick={onToggleSidebar}
      aria-label="Toggle sidebar"
    >
      <Icon name="Menu" size={18} />
    </button>
    <div class="flex items-center gap-2" data-tauri-drag-region>
      <img src="/streamflow-icon.svg" alt="" class="h-7 w-7 flex-shrink-0" data-tauri-drag-region />
      <span class="text-sm font-semibold text-text-primary tracking-tight" data-tauri-drag-region>StreamFlow</span>
    </div>
  </div>

  <div class="flex items-center gap-1" data-tauri-drag-region>
    <span class="text-sm font-medium text-text-secondary" data-tauri-drag-region>StreamFlow</span>
  </div>

  {#if !isMac}
    <div class="flex items-center gap-1">
      <button
        class="h-8 w-10 rounded-lg hover:bg-surface-hover text-text-secondary hover:text-text-primary transition-all duration-200"
        onclick={minimize}
        aria-label="Minimize"
      >
        <Icon name="Minus" size={14} class="mx-auto" />
      </button>
      <button
        class="h-8 w-10 rounded-lg hover:bg-surface-hover text-text-secondary hover:text-text-primary transition-all duration-200"
        onclick={toggleMaximize}
        aria-label="Maximize"
      >
        <Icon name="Maximize" size={14} class="mx-auto" />
      </button>
      <button
        class="h-8 w-10 rounded-lg hover:bg-error text-text-secondary hover:text-white transition-all duration-200"
        onclick={close}
        aria-label="Close"
      >
        <Icon name="X" size={14} class="mx-auto" />
      </button>
    </div>
  {/if}
</div>
