import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

// This directory is a verbatim copy of design/system/ (the design system's
// source of truth), and no build step syncs them — the standing rule is
// "when you change a component, edit both copies identically". This test is
// that rule, executed: every file here (tests excepted — they live only on
// the app side) must exist in design/system/ at the same relative path with
// identical bytes. design/system/ owning extra material the app doesn't
// consume (cards, ui_kits, the prebuilt bundle) is fine and not checked.

const appCopyDir = path.dirname(fileURLToPath(import.meta.url));
const sourceDir = path.resolve(appCopyDir, "../../../design/system");

const isTestFile = (name) => /\.test\.[jt]sx?$/.test(name);

function walk(dir, prefix = "") {
  const files = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) files.push(...walk(path.join(dir, entry.name), rel));
    else if (!isTestFile(entry.name)) files.push(rel);
  }
  return files;
}

describe("design-system copies stay in sync", () => {
  const shared = walk(appCopyDir);

  it("walks the real app copy, not an empty or wrong directory", () => {
    // Guards the sync check below against passing vacuously if this file
    // ever moves and the relative paths above stop pointing at the trees.
    expect(shared).toContain("components/subscription/SubscriptionRow.jsx");
    expect(shared).toContain("tokens/colors.css");
    expect(existsSync(path.join(sourceDir, "readme.md"))).toBe(true);
  });

  it("every app-copy file is byte-identical to its design/system counterpart", () => {
    const missing = [];
    const diverged = [];
    for (const rel of shared) {
      const sourcePath = path.join(sourceDir, rel);
      if (!existsSync(sourcePath)) {
        missing.push(rel);
      } else if (!readFileSync(path.join(appCopyDir, rel)).equals(readFileSync(sourcePath))) {
        diverged.push(rel);
      }
    }
    expect(missing, "app-copy files with no design/system counterpart — add them there too").toEqual([]);
    expect(diverged, "files differing between the copies — edit both identically").toEqual([]);
  });
});
