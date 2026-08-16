import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],

  resolve: {
    // The single source of truth for path aliases is tsconfig.app.json's
    // own `paths`; Vite reads it natively rather than duplicating the
    // mapping in a second `resolve.alias` block.
    tsconfigPaths: true,
  },

  test: {
    environment: "jsdom",
  },

  // Tauri requires the dev server not to obscure Rust build errors and to
  // fail fast on a taken port rather than fall back to another one.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
