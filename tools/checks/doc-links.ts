#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'

const DOC_FILE = /^(AGENTS\.md|README\.md|docs\/.+\.md)$/
const MARKDOWN_LINK = /\[[^\]]*\]\(([^)]+)\)/g
const EXTERNAL_TARGET = /^([a-z][a-z0-9+.-]*:|#)/i

interface Finding {
  file: string
  target: string
}

const trackedFiles = execFileSync('git', ['ls-files', '-z'], { encoding: 'utf8' })
  .split('\0')
  .filter(Boolean)

const docFiles = trackedFiles.filter((file) => DOC_FILE.test(file))

const findings: Finding[] = []

for (const file of docFiles) {
  const text = readFileSync(file, 'utf8')
  for (const match of text.matchAll(MARKDOWN_LINK)) {
    const target = match[1].trim()
    if (target === '' || EXTERNAL_TARGET.test(target)) continue
    const withoutFragment = target.split('#')[0]
    if (withoutFragment === '') continue
    const resolved = resolve(dirname(file), withoutFragment)
    if (!existsSync(resolved)) {
      findings.push({ file, target })
    }
  }
}

if (findings.length > 0) {
  console.error('Relative links that do not resolve to a tracked file:')
  for (const { file, target } of findings) {
    console.error(`  ${file}: ${target}`)
  }
  process.exit(1)
}
