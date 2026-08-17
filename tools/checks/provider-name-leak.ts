#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const CLAUDE_WORD = /claude/gi;
const CLAUDE_CODE = /^claude\s+code\b/i;

const EXACT_EXCEPTIONS = new Set(["src/providers/registry.ts", "src/lib/mock-client.ts"]);
const EXEMPT_PREFIXES = ["src/providers/claude/"];

interface Finding {
  file: string;
  line: number;
  text: string;
}

const trackedFiles = execFileSync("git", ["ls-files", "-z"], { encoding: "utf8" })
  .split("\0")
  .filter(Boolean);

const findings: Finding[] = [];

for (const file of trackedFiles) {
  if (!file.startsWith("src/")) continue;
  if (!file.endsWith(".ts") && !file.endsWith(".tsx")) continue;
  if (file.endsWith(".spec.ts") || file.endsWith(".spec.tsx")) continue;
  if (EXACT_EXCEPTIONS.has(file)) continue;
  if (EXEMPT_PREFIXES.some((prefix) => file.startsWith(prefix))) continue;

  const lines = readFileSync(file, "utf8").split("\n");
  lines.forEach((line, index) => {
    for (const match of line.matchAll(CLAUDE_WORD)) {
      const rest = line.slice(match.index);
      if (CLAUDE_CODE.test(rest)) continue;
      findings.push({ file, line: index + 1, text: line.trim() });
    }
  });
}

if (findings.length > 0) {
  console.error(
    "The provider is named outside its registered boundary. The design brief " +
      '(section 1: "Not a Claude usage widget"; section 8: providers other ' +
      "than the first are out of scope for this version) treats Quotos as a " +
      "multi-provider capacity dashboard. Only src/providers/claude/ and its " +
      'single registry entry in src/providers/registry.ts may name "Claude" ' +
      'as a provider. "Claude Code", the external CLI tool Quotos integrates ' +
      "with, stays allowed anywhere as a feature reference, not a provider label.",
  );
  for (const { file, line, text } of findings) {
    console.error(`  ${file}:${line}: ${text}`);
  }
  process.exit(1);
}
