#!/usr/bin/env node
// Build all Memex side binaries (daemon + cli) and copy them into
// src-tauri/binaries/<name>-<target> so the Tauri externalBin
// sidecar mechanism picks them up and bundles them into Memex.app/Contents/MacOS.

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

const sidecars = [
  { crate: 'memex-daemon', binary: 'memex-daemon', destName: 'memex-daemon' },
  // memex CLI 也带进 .app，让 setup cursor/claude-code 配置出来的命令路径稳定指向
  // /Applications/Memex.app/Contents/MacOS/memex
  { crate: 'memex-cli', binary: 'memex', destName: 'memex' },
]

const binDir = resolve(tauriRoot, 'src-tauri/binaries')
mkdirSync(binDir, { recursive: true })

for (const { crate, binary, destName } of sidecars) {
  console.log(`[sidecars] building ${crate} (release)…`)
  execSync(`cargo build --release -p ${crate}`, {
    cwd: repoRoot,
    stdio: 'inherit',
  })

  const src = resolve(repoRoot, `target/release/${binary}`)
  if (!existsSync(src)) {
    console.error(`[sidecars] ERROR: ${src} not produced`)
    process.exit(1)
  }

  const dest = resolve(binDir, `${destName}-${target}`)
  copyFileSync(src, dest)
  chmodSync(dest, 0o755)
  console.log(`[sidecars] copied → ${dest}`)
}
