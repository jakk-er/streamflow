<script lang="ts">
  import { setContext } from 'svelte';
  import { writable, type Writable } from 'svelte/store';

  interface ToastItem {
    id: string;
    message: string;
    type: 'success' | 'error' | 'info';
  }

  const toasts: Writable<ToastItem[]> = writable([]);

  function generateId() {
    return `${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
  }

  function addToast(message: string, type: ToastItem['type'] = 'info') {
    const id = generateId();
    const toast: ToastItem = { id, message, type };
    toasts.update(list => [...list, toast]);
    setTimeout(() => {
      toasts.update(list => list.filter(t => t.id !== id));
    }, 4000);
    return id;
  }

  function dismissToast(id: string) {
    toasts.update(list => list.filter(t => t.id !== id));
  }

  setContext('toasts', {
    subscribe: toasts.subscribe,
    addToast,
    dismissToast,
  });
</script>

<div class="fixed bottom-4 right-4 z-50 flex max-w-sm flex-col gap-2">
  {#each $toasts as toast (toast.id)}
    <div
      class="toast-item flex items-center gap-2 rounded-lg px-4 py-3 shadow-lg transition-all duration-300"
      class:bg-green-600={toast.type === 'success'}
      class:bg-red-600={toast.type === 'error'}
      class:bg-blue-600={toast.type === 'info'}
      class:text-white={true}
      role="alert"
    >
      <span class="flex-1 text-sm font-medium">{toast.message}</span>
      <button
        class="ml-2 text-white/80 hover:text-white"
        onclick={() => dismissToast(toast.id)}
        aria-label="Dismiss"
      >
        <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  {/each}
</div>

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
