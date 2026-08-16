#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { basename } from "node:path";

const KEBAB_CASE = /^\.?[a-z0-9]+(-[a-z0-9]+)*(\.[a-z0-9]+(-[a-z0-9]+)*)*$/;
const RUST_MODULE_FILE = /^[a-z0-9]+(_[a-z0-9]+)*\.rs$/;

// Established ecosystem conventions the owner chose to keep, spelled exactly
// as their tooling requires.
const EXACT_EXCEPTIONS = new Set([
  "README.md",
  "AGENTS.md",
  "CLAUDE.md",
  "Cargo.toml",
  "Cargo.lock",
  "Info.plist",
]);

// Path prefixes carrying their own naming authority: Rust's own snake_case
// module convention, Tauri's generated icon set, and vendor font family
// names.
const EXEMPT_PREFIXES = ["src-tauri/icons/", "src/design-system/assets/fonts/"];

const trackedFiles = execFileSync("git", ["ls-files", "-z"], { encoding: "utf8" })
  .split("\0")
  .filter(Boolean);

const findings: string[] = [];

for (const file of trackedFiles) {
  const name = basename(file);
  if (EXACT_EXCEPTIONS.has(name)) continue;
  if (EXEMPT_PREFIXES.some((prefix) => file.startsWith(prefix))) continue;
  if (file.startsWith("src-tauri/src/") && name.endsWith(".rs")) {
    if (!RUST_MODULE_FILE.test(name)) findings.push(file);
    continue;
  }
  if (!KEBAB_CASE.test(name)) findings.push(file);
}

if (findings.length > 0) {
  console.error("Tracked file names must be kebab-case:");
  for (const file of findings) {
    console.error(`  ${file}`);
  }
  process.exit(1);
}
