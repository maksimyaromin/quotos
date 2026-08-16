import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

// src-tauri/Cargo.toml's [package] version is the single source of truth.
// Tauri derives its own version from it (tauri.conf.json carries no
// "version" field of its own); package.json can't do that automatically,
// so this test is the check that keeps it from drifting.

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function cargoVersion() {
  const cargoToml = readFileSync(path.join(rootDir, "src-tauri/Cargo.toml"), "utf8");
  const match = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) throw new Error("src-tauri/Cargo.toml has no [package] version");
  return match[1];
}

describe("the version lives in one place", () => {
  it("package.json matches src-tauri/Cargo.toml", () => {
    const pkg = JSON.parse(readFileSync(path.join(rootDir, "package.json"), "utf8"));
    expect(pkg.version).toBe(cargoVersion());
  });

  it("tauri.conf.json carries no version of its own, so Tauri derives it from Cargo.toml", () => {
    const tauriConf = JSON.parse(
      readFileSync(path.join(rootDir, "src-tauri/tauri.conf.json"), "utf8"),
    );
    expect(tauriConf.version).toBeUndefined();
  });
});
