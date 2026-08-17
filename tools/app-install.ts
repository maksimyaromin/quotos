#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { cpSync, existsSync } from 'node:fs'

const BUNDLE_PATH = 'src-tauri/target/release/bundle/macos/Quotos.app'
const INSTALL_PATH = '/Applications/Quotos.app'

execFileSync('npm', ['run', 'tauri', 'build'], { stdio: 'inherit' })

if (!existsSync(BUNDLE_PATH)) {
  console.error(`Build finished but ${BUNDLE_PATH} is missing.`)
  process.exit(1)
}

cpSync(BUNDLE_PATH, INSTALL_PATH, { recursive: true, force: true })
console.log(`Installed ${INSTALL_PATH}`)
