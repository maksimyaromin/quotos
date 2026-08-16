import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],

  build: {
    // This value matches `minimumSystemVersion` in src-tauri/tauri.conf.json.
    // macOS 14.0 bundles Safari 17 as its WebKit engine. Vite's own default
    // target floats forward over time, so pinning this explicitly keeps the
    // bundle's actual engine floor from drifting away from the declared one.
    target: "safari17",
  },

  resolve: {
    // The single source of truth for path aliases is tsconfig.app.json's
    // own `paths`; Vite reads it natively rather than duplicating the
    // mapping in a second `resolve.alias` block.
    tsconfigPaths: true,
  },

  test: {
    environment: "jsdom",
    // Vitest mocks every CSS import to an empty string by default, since it
    // normally cannot tell a style-injection import from a text one. `?raw`
    // is opted in here so a spec can read a stylesheet's real text the way
    // Vite already serves it, and `.module.css` is opted in so a spec that
    // renders a component gets that component's real class-scoped rules
    // applied in jsdom, not an empty stylesheet, letting `getComputedStyle`
    // assertions verify actual CSS Modules behavior instead of a component's
    // now-removed inline `style` objects.
    css: {
      include: [/\?raw$/, /\.module\.css$/],
    },
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
