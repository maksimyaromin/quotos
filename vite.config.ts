import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const host = process.env.TAURI_DEV_HOST

export default defineConfig(async () => ({
  plugins: [react()],

  build: {
    // Pinned to match tauri.conf.json's minimumSystemVersion; Vite's own
    // default target drifts forward on its own schedule otherwise.
    target: 'safari18',
  },

  resolve: {
    tsconfigPaths: true,
  },

  test: {
    environment: 'jsdom',
    css: {
      // Vitest stubs CSS imports as empty strings by default; these
      // patterns opt module and ?raw imports back into their real content.
      include: [/\?raw$/, /\.module\.css$/],
    },
  },

  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
}))
