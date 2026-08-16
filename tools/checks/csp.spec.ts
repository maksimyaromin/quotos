import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "vitest";

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

interface TauriConfig {
  app?: {
    security?: {
      csp?: string;
    };
  };
}

function directives(): Map<string, string[]> {
  const conf = JSON.parse(
    readFileSync(path.join(rootDir, "src-tauri/tauri.conf.json"), "utf8"),
  ) as TauriConfig;
  const csp = conf.app?.security?.csp;
  if (!csp) throw new Error("tauri.conf.json declares no app.security.csp");
  const map = new Map<string, string[]>();
  for (const clause of csp.split(";")) {
    const [name, ...sources] = clause.trim().split(/\s+/);
    if (name) map.set(name, sources);
  }
  return map;
}

// The app draws every icon and glyph as inline SVG, and the status item's
// own bitmap is set natively from Rust; nothing in the webview ever
// resource-fetches an image. `img-src` grants no more than that.
test("img-src grants only the app's own origin", () => {
  expect(directives().get("img-src")).toEqual(["'self'"]);
});

// Tauri's own local IPC bridge, not a remote host; see "connect-src ipc:
// http://ipc.localhost for Tauri's own IPC transport" in
// docs/platform-constraints.md.
const IPC_ORIGIN = "http://ipc.localhost";

// See "The policy names no remote host anywhere" in
// docs/platform-constraints.md: every HTTP request this app makes happens
// on the Rust side, so no directive here should need a wildcard, a bare
// scheme, or an external host to do its job.
test("no directive names a remote host or a wildcard source", () => {
  for (const [name, sources] of directives()) {
    for (const source of sources.filter((s) => s !== IPC_ORIGIN)) {
      expect(source, `${name} source "${source}"`).not.toMatch(/^\*$|^https?:$|^https?:\/\//);
    }
  }
});
