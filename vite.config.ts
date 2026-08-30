import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [sveltekit(), tailwindcss()],
  server: {
    port: 5173,
    strictPort: true,
    host: '0.0.0.0',
    warmup: {
      clientFiles: ['./src/routes/+page.svelte']
    }
  },
  build: {
    sourcemap: true
  },
  optimizeDeps: {
    include: ['lucide-svelte'],
    exclude: ['@tauri-apps/api']
  }
});
