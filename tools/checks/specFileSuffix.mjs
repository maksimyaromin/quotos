#!/usr/bin/env node
import { execFileSync } from "node:child_process";

const STRAY_TEST_FILE = /\.test\.(ts|tsx|js|jsx)$/;

const trackedFiles = execFileSync("git", ["ls-files", "-z"], { encoding: "utf8" })
  .split("\0")
  .filter(Boolean);

const strayFiles = trackedFiles.filter((file) => STRAY_TEST_FILE.test(file));

if (strayFiles.length > 0) {
  console.error("Test files must use the .spec suffix, not .test:");
  for (const file of strayFiles) {
    console.error(`  ${file}`);
  }
  process.exit(1);
}
