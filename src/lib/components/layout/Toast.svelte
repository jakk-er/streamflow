<script lang="ts">
  import { getContext } from 'svelte';

  interface ToastContext {
    subscribe: (run: (value: import('svelte/store').Readable<{ id: string; message: string; type: string }>[]) => void) => () => void;
    addToast: (message: string, type?: 'success' | 'error' | 'info') => string;
    dismissToast: (id: string) => void;
  }

  const { subscribe, addToast, dismissToast } = getContext<ToastContext>('toasts');

  let { message = '', type = 'info', onDismiss }: { message?: string; type?: 'success' | 'error' | 'info'; onDismiss?: () => void } = $props();

  let visible = $state(true);

  function handleDismiss() {
    visible = false;
    onDismiss?.();
  }

  let bgColor = $derived(({
    success: 'bg-green-600',
    error: 'bg-red-600',
    info: 'bg-blue-600',
  } as const)[type]);
</script>

{#if visible}
  <div
    class="toast-item flex items-center gap-2 rounded-lg px-4 py-3 shadow-lg transition-all duration-300 {bgColor} text-white"
    role="alert"
  >
    <span class="flex-1 text-sm font-medium">{message}</span>
    <button
      class="ml-2 text-white/80 hover:text-white"
      onclick={handleDismiss}
      aria-label="Dismiss"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
      </svg>
    </button>
  </div>
{/if}

<style>
  .toast-item {
    animation: slideIn 0.3s ease-out;
  }
  @keyframes slideIn {
    from {
      transform: translateX(100%);
      opacity: 0;
    }
    to {
      transform: translateX(0);
      opacity: 1;
    }
  }
</style>
