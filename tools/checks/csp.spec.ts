import { readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { expect, test } from 'vitest'

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')

interface TauriConfig {
  app?: {
    security?: {
      csp?: string
    }
  }
}

function directives(): Map<string, string[]> {
  const conf = JSON.parse(
    readFileSync(path.join(rootDir, 'src-tauri/tauri.conf.json'), 'utf8'),
  ) as TauriConfig
  const csp = conf.app?.security?.csp
  if (!csp) throw new Error('tauri.conf.json declares no app.security.csp')
  const map = new Map<string, string[]>()
  for (const clause of csp.split(';')) {
    const [name, ...sources] = clause.trim().split(/\s+/)
    if (name) map.set(name, sources)
  }
  return map
}

test("img-src grants only the app's own origin", () => {
  expect(directives().get('img-src')).toEqual(["'self'"])
})

const IPC_ORIGIN = 'http://ipc.localhost'

test('no directive names a remote host or a wildcard source', () => {
  for (const [name, sources] of directives()) {
    for (const source of sources.filter((s) => s !== IPC_ORIGIN)) {
      expect(source, `${name} source "${source}"`).not.toMatch(/^\*$|^https?:$|^https?:\/\//)
    }
  }
})
