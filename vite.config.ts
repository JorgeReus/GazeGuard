import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  publicDir: 'src',
  build: { outDir: 'dist', emptyOutDir: true },
  server: { port: 1420, strictPort: true, clearScreen: false }
});
