#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { cpSync, existsSync, rmSync } from 'node:fs'

const BUNDLE_PATH = 'src-tauri/target/release/bundle/macos/Quotos.app'
const INSTALL_PATH = '/Applications/Quotos.app'
// Cargo.toml's package name, not the productName Quotos.app is titled with.
const RUNNING_PROCESS_NAME = 'quotos-app'

function refuseIfRunning(): void {
  try {
    execFileSync('pgrep', ['-x', RUNNING_PROCESS_NAME])
  } catch {
    return
  }
  console.error('Quit Quotos first: installing over a running copy can corrupt it.')
  process.exit(1)
}

refuseIfRunning()
execFileSync('npm', ['run', 'tauri', 'build'], { stdio: 'inherit' })

if (!existsSync(BUNDLE_PATH)) {
  console.error(`Build finished but ${BUNDLE_PATH} is missing.`)
  process.exit(1)
}

refuseIfRunning()

try {
  rmSync(INSTALL_PATH, { recursive: true, force: true })
  cpSync(BUNDLE_PATH, INSTALL_PATH, { recursive: true })
} catch (error) {
  const code = (error as NodeJS.ErrnoException).code
  if (code === 'EACCES' || code === 'EPERM') {
    console.error(
      `No permission to write to ${INSTALL_PATH}. Fix its permissions, or move the built bundle there yourself.`,
    )
    process.exit(1)
  }
  throw error
}

console.log(`Installed ${INSTALL_PATH}`)
