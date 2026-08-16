#!/usr/bin/env node
// The single quality gate: runs every lane concurrently, lets each one run
// to completion, and reports every failure instead of stopping at the
// first. CI invokes this same file through `npm run verify`, so a green
// local run and a red CI run can never disagree about what ran.

import { spawn } from "node:child_process";

interface Lane {
  name: string;
  script: string;
}

interface LaneResult extends Lane {
  code: number | null;
  output: string;
}

const lanes: Lane[] = [
  { name: "format", script: "format:check" },
  { name: "lint", script: "lint" },
  { name: "typecheck", script: "typecheck" },
  { name: "tools typecheck", script: "typecheck:tools" },
  { name: "test", script: "test" },
  { name: "hidden characters", script: "check:hidden-characters" },
  { name: "spec suffix", script: "check:spec-suffix" },
  { name: "cargo fmt", script: "cargo:fmt" },
  { name: "cargo clippy", script: "cargo:clippy" },
];

function runLane(lane: Lane): Promise<LaneResult> {
  return new Promise((resolve) => {
    const child = spawn("npm", ["run", "--silent", lane.script], {
      stdio: ["ignore", "pipe", "pipe"],
    });
    let output = "";
    child.stdout.on("data", (chunk) => {
      output += chunk;
    });
    child.stderr.on("data", (chunk) => {
      output += chunk;
    });
    child.on("close", (code) => {
      resolve({ ...lane, code, output });
    });
  });
}

const results = await Promise.all(lanes.map(runLane));

for (const result of results) {
  if (result.code !== 0) {
    console.log(`\n── ${result.name} ${"─".repeat(60 - result.name.length)}\n`);
    process.stdout.write(result.output);
  }
}

console.log("\nverify summary:");
for (const result of results) {
  console.log(`  ${result.code === 0 ? "PASS" : "FAIL"}  ${result.name}`);
}

const failed = results.filter((result) => result.code !== 0);
if (failed.length > 0) {
  console.log(`\n${failed.length} of ${results.length} lanes failed.`);
  process.exit(1);
}
console.log(`\nall ${results.length} lanes passed.`);
