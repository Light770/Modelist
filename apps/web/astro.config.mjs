import { defineConfig } from 'astro/config';
import react from '@astrojs/react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  integrations: [react()],
  // The studio is a full-window desktop app; Astro's dev toolbar would cover its status bar.
  devToolbar: { enabled: false },
  vite: {
    plugins: [tailwindcss()],
    server: {
      fs: {
        allow: ['../..'],
      },
    },
    optimizeDeps: {
      exclude: ['modelist-wasm'],
    },
  },
});
