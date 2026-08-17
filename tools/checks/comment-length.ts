#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'

const MAX_BLOCK_LINES = 3

interface LanguageRule {
  extensions: string[]
  lineCommentPrefixes: string[]
}

const RULES: LanguageRule[] = [
  { extensions: ['.rs'], lineCommentPrefixes: ['///', '//!', '//'] },
  { extensions: ['.toml'], lineCommentPrefixes: ['#'] },
]

interface Block {
  startLine: number
  endLine: number
}

function ruleFor(file: string): LanguageRule | null {
  return RULES.find((rule) => rule.extensions.some((ext) => file.endsWith(ext))) ?? null
}

function findLineCommentBlocks(lines: string[], rule: LanguageRule): Block[] {
  const blocks: Block[] = []
  let start: number | null = null
  for (let i = 0; i < lines.length; i++) {
    const trimmed = lines[i].trim()
    if (rule.lineCommentPrefixes.some((prefix) => trimmed.startsWith(prefix))) {
      if (start === null) start = i
    } else if (start !== null) {
      blocks.push({ startLine: start + 1, endLine: i })
      start = null
    }
  }
  if (start !== null) blocks.push({ startLine: start + 1, endLine: lines.length })
  return blocks
}

const trackedFiles = execFileSync('git', ['ls-files', '-z'], { encoding: 'utf8' })
  .split('\0')
  .filter(Boolean)

const findings: { file: string; block: Block }[] = []

for (const file of trackedFiles) {
  const rule = ruleFor(file)
  if (!rule) continue

  let text: string
  try {
    text = readFileSync(file, 'utf8')
  } catch {
    continue
  }

  for (const block of findLineCommentBlocks(text.split('\n'), rule)) {
    if (block.endLine - block.startLine + 1 > MAX_BLOCK_LINES) {
      findings.push({ file, block })
    }
  }
}

if (findings.length > 0) {
  console.error(`Comment blocks longer than ${MAX_BLOCK_LINES} lines found:`)
  for (const { file, block } of findings) {
    console.error(
      `  ${file}:${block.startLine}-${block.endLine} (${block.endLine - block.startLine + 1} lines)`,
    )
  }
  console.error(
    '\nMove the prose to docs/, in the page nearest the behavior, and leave a one-line pointer.',
  )
  process.exit(1)
}
