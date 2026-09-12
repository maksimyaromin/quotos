#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { basename } from 'node:path'

const KEBAB_CASE = /^\.?[a-z0-9]+(-[a-z0-9]+)*(\.[a-z0-9]+(-[a-z0-9]+)*)*$/
const RUST_SOURCE_FILE = /^[a-z0-9]+(_[a-z0-9]+)*\.rs$/

const EXACT_EXCEPTIONS = new Set([
  'README.md',
  'AGENTS.md',
  'CLAUDE.md',
  'CONTRIBUTING.md',
  'SECURITY.md',
  'Cargo.toml',
  'Cargo.lock',
  'Info.plist',
  'LICENSE',
  // GitHub only recognizes this exact, underscored name as the
  // default pull request template.
  'pull_request_template.md',
  // A Claude Code skill is only found under this exact, capitalized
  // name in its own directory.
  'SKILL.md',
])

const EXEMPT_PREFIXES = ['src-tauri/icons/', 'src/design-system/assets/fonts/']

const trackedFiles = execFileSync('git', ['ls-files', '-z'], { encoding: 'utf8' })
  .split('\0')
  .filter(Boolean)

const findings: string[] = []

for (const file of trackedFiles) {
  const name = basename(file)
  if (EXACT_EXCEPTIONS.has(name)) continue
  if (EXEMPT_PREFIXES.some((prefix) => file.startsWith(prefix))) continue
  // Every Rust file in the crate, not just the modules under `src/`:
  // examples, build script, and anything Cargo adds later all follow
  // the one snake_case convention. See "Rust source file names" in
  // docs/architecture.md.
  if (file.startsWith('src-tauri/') && name.endsWith('.rs')) {
    if (!RUST_SOURCE_FILE.test(name)) findings.push(file)
    continue
  }
  if (!KEBAB_CASE.test(name)) findings.push(file)
}

if (findings.length > 0) {
  console.error('Tracked file names must be kebab-case:')
  for (const file of findings) {
    console.error(`  ${file}`)
  }
  process.exit(1)
}
