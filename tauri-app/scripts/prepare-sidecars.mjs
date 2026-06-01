#!/usr/bin/env node
// Build memex-daemon and copy it into src-tauri/binaries/memex-daemon-<target>
// so the Tauri externalBin sidecar mechanism picks it up.

import { execSync } from 'node:child_process'
import { existsSync, mkdirSync, copyFileSync, chmodSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const tauriRoot = resolve(here, '..')
const repoRoot = resolve(tauriRoot, '..')

const target = execSync('rustc -vV', { encoding: 'utf8' })
  .split('\n')
  .find((l) => l.startsWith('host:'))
  .split(':')[1]
  .trim()

console.log(`[sidecars] target triple: ${target}`)

console.log('[sidecars] building memex-daemon (release)…')
execSync('cargo build --release -p memex-daemon', {
  cwd: repoRoot,
  stdio: 'inherit',
})

const src = resolve(repoRoot, 'target/release/memex-daemon')
if (!existsSync(src)) {
  console.error(`[sidecars] ERROR: ${src} not produced`)
  process.exit(1)
}

const binDir = resolve(tauriRoot, 'src-tauri/binaries')
mkdirSync(binDir, { recursive: true })
const dest = resolve(binDir, `memex-daemon-${target}`)
copyFileSync(src, dest)
chmodSync(dest, 0o755)
console.log(`[sidecars] copied → ${dest}`)
