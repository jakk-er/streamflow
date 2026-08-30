<script lang="ts">
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
</script>

<div class="flex h-screen w-screen items-center justify-center bg-gray-900 text-white">
  <div class="max-w-md px-6 text-center">
    <h1 class="text-8xl font-bold text-gray-700 mb-4">
      {$page.status}
    </h1>
    <p class="text-xl text-gray-300 mb-8">
      {$page.error?.message || 'Something went wrong'}
    </p>
    <div class="flex items-center justify-center gap-3">
      <button
        class="rounded-lg bg-blue-600 px-6 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors"
        onclick={() => goto('/')}
      >
        Go Home
      </button>
      <button
        class="rounded-lg bg-gray-800 px-6 py-2 text-sm font-medium text-gray-300 hover:bg-gray-700 transition-colors"
        onclick={() => window.location.reload()}
      >
        Reload
      </button>
    </div>
  </div>
</div>
