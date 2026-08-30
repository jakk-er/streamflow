<script lang="ts">
  import { stalkerSessionStore } from '$lib/stores';

  let {
    playlistId,
    onsuccess,
    oncancel,
  }: {
    playlistId: string;
    onsuccess?: () => void;
    oncancel?: () => void;
  } = $props();

  let username = $state('');
  let password = $state('');

  async function handleSubmit(e: Event) {
    e.preventDefault();
    if (!username.trim() || !password) return;
    try {
      const result = await stalkerSessionStore.completeLogin(playlistId, username.trim(), password);
      if (result.kind === 'success') {
        onsuccess?.();
      }
    } catch {
      // stalkerSessionStore.error already carries the message; nothing else to do here.
    }
  }
</script>

<form class="space-y-4" onsubmit={handleSubmit}>
  <div class="rounded-lg bg-blue-900/30 border border-blue-700 p-3 text-sm text-blue-200">
    This portal requires a username and password to continue.
  </div>

  {#if stalkerSessionStore.error}
    <div class="rounded-lg bg-red-900/50 border border-red-700 p-3 text-sm text-red-300">
      {stalkerSessionStore.error}
    </div>
  {/if}

  <div>
    <label class="block text-sm font-medium text-white mb-1" for="stalker-login-username">Username *</label>
    <input
      type="text"
      id="stalker-login-username"
      autocomplete="username"
      value={username}
      oninput={(e) => (username = (e.target as HTMLInputElement).value)}
      class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
    />
  </div>
  <div>
    <label class="block text-sm font-medium text-white mb-1" for="stalker-login-password">Password *</label>
    <input
      type="password"
      id="stalker-login-password"
      autocomplete="current-password"
      value={password}
      oninput={(e) => (password = (e.target as HTMLInputElement).value)}
      class="w-full rounded-lg bg-gray-700 px-4 py-2 text-sm text-white placeholder-gray-400 outline-none focus:ring-2 focus:ring-blue-500 border border-gray-600"
    />
  </div>

  <div class="flex gap-2">
    <button
      type="submit"
      class="flex-1 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      disabled={!username.trim() || !password || stalkerSessionStore.loading}
    >
      {stalkerSessionStore.loading ? 'Signing in…' : 'Sign In'}
    </button>
    {#if oncancel}
      <button
        type="button"
        class="rounded-lg bg-gray-700 px-4 py-2 text-sm font-medium text-gray-200 hover:bg-gray-600 transition-colors"
        onclick={oncancel}
      >
        Cancel
      </button>
    {/if}
  </div>
</form>
