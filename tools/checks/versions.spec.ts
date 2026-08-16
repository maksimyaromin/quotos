import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "vitest";

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

interface PackageManifest {
  version: string;
}

interface TauriConfig {
  version?: string;
}

function cargoVersion(): string {
  const cargoToml = readFileSync(path.join(rootDir, "src-tauri/Cargo.toml"), "utf8");
  const match = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) throw new Error("src-tauri/Cargo.toml has no [package] version");
  return match[1];
}

test("package.json's version matches src-tauri/Cargo.toml's", () => {
  const pkg = JSON.parse(
    readFileSync(path.join(rootDir, "package.json"), "utf8"),
  ) as PackageManifest;
  expect(pkg.version).toBe(cargoVersion());
});

test("tauri.conf.json carries no version of its own, so Tauri derives it from Cargo.toml", () => {
  const tauriConf = JSON.parse(
    readFileSync(path.join(rootDir, "src-tauri/tauri.conf.json"), "utf8"),
  ) as TauriConfig;
  expect(tauriConf.version).toBeUndefined();
});
