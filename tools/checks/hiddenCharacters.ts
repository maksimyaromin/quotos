#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const NAMED_CODE_POINTS = new Map([
  [0x200b, "zero-width space"],
  [0x200c, "zero-width non-joiner"],
  [0x200d, "zero-width joiner"],
  [0x2060, "word joiner"],
  [0x2063, "invisible separator"],
  [0x00ad, "soft hyphen"],
  [0x00a0, "non-breaking space"],
  [0xfeff, "byte-order mark"],
]);

const PRINTABLE_WHITESPACE = new Set(["\n", "\t", "\r", " "]);
const NON_PRINTING_CATEGORY = /\p{Cc}|\p{Cf}|\p{Co}|\p{Zl}|\p{Zp}|\p{Zs}/u;

interface Finding {
  line: number;
  column: number;
  description: string;
}

function describe(character: string): string {
  const codePoint = character.codePointAt(0) as number;
  const named = NAMED_CODE_POINTS.get(codePoint);
  return named ?? `U+${codePoint.toString(16).toUpperCase().padStart(4, "0")}`;
}

function isNonPrinting(character: string): boolean {
  if (PRINTABLE_WHITESPACE.has(character)) return false;
  const codePoint = character.codePointAt(0) as number;
  return NAMED_CODE_POINTS.has(codePoint) || NON_PRINTING_CATEGORY.test(character);
}

function findNonPrintingCharacters(text: string): Finding[] {
  const findings: Finding[] = [];
  let line = 1;
  let column = 0;
  for (const character of text) {
    if (character === "\n") {
      line += 1;
      column = 0;
      continue;
    }
    column += 1;
    if (isNonPrinting(character)) {
      findings.push({ line, column, description: describe(character) });
    }
  }
  return findings;
}

function decodeAsText(buffer: Buffer): string | null {
  try {
    return new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(buffer);
  } catch {
    return null;
  }
}

const trackedFiles = execFileSync("git", ["ls-files", "-z"], { encoding: "utf8" })
  .split("\0")
  .filter(Boolean);

const findingsByFile: { file: string; findings: Finding[] }[] = [];

for (const file of trackedFiles) {
  let buffer: Buffer;
  try {
    buffer = readFileSync(file);
  } catch {
    continue;
  }
  const text = decodeAsText(buffer);
  if (text === null) continue;

  const findings = findNonPrintingCharacters(text);
  if (findings.length > 0) findingsByFile.push({ file, findings });
}

if (findingsByFile.length > 0) {
  console.error("Non-printing characters found in tracked files:");
  for (const { file, findings } of findingsByFile) {
    for (const { line, column, description } of findings) {
      console.error(`  ${file}:${line}:${column} ${description}`);
    }
  }
  process.exit(1);
}
