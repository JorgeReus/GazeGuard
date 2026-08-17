import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  publicDir: 'public',
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    rollupOptions: { input: { index: 'index.html', break: 'src/break.html' } }
  },
  server: {
    port: 1420,
    strictPort: true,
    clearScreen: false,
    watch: { ignored: ['**/.git/**', '**/dist/**', '**/src-tauri/**', '**/target/**'] }
  }
});
