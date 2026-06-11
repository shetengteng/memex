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

const hostTarget = execSync('rustc -vV', { encoding: 'utf8' })
  .split('\n')
  .find((l) => l.startsWith('host:'))
  .split(':')[1]
  .trim()

const envTarget = process.env.MEMEX_BUILD_TARGET || process.env.CARGO_BUILD_TARGET
const target = envTarget || hostTarget
const isCrossArch = target !== hostTarget

console.log(`[sidecars] host triple: ${hostTarget}`)
console.log(`[sidecars] build target: ${target}${isCrossArch ? ' (cross)' : ''}`)

// Phase 4 起 `memex-daemon` 不再有独立 binary（被折叠成 Tauri 主进程内的
// in-process tokio task），所以 sidecars 列表里只剩 CLI。
//
// 注：`memex` CLI 的 dest name 必须避开主 binary `Memex`（CFBundleExecutable）。
// macOS APFS 默认大小写不敏感，`memex` 与 `Memex` 会被视为同一个文件，后写
// 的会物理覆盖前写的——bundle 启动直接坏掉。把 CLI sidecar 改名为 `memex-cli`
// 后没有冲突；用户访问 CLI 路径变成 `/Applications/Memex.app/Contents/MacOS/memex-cli`，
// brew cask 通过 `target: "memex"` 仍然能把命令暴露成 `memex`。
const sidecars = [
  { crate: 'memex-cli', binary: 'memex', destName: 'memex-cli' },
]

const binDir = resolve(tauriRoot, 'src-tauri/binaries')
mkdirSync(binDir, { recursive: true })

const targetFlag = isCrossArch ? ` --target ${target}` : ''
const targetSubdir = isCrossArch ? `${target}/release` : 'release'

for (const { crate, binary, destName } of sidecars) {
  console.log(`[sidecars] building ${crate} (release)…`)
  execSync(`cargo build --release -p ${crate}${targetFlag}`, {
    cwd: repoRoot,
    stdio: 'inherit',
  })

  const src = resolve(repoRoot, `target/${targetSubdir}/${binary}`)
  if (!existsSync(src)) {
    console.error(`[sidecars] ERROR: ${src} not produced`)
    process.exit(1)
  }

  const dest = resolve(binDir, `${destName}-${target}`)
  copyFileSync(src, dest)
  chmodSync(dest, 0o755)
  console.log(`[sidecars] copied → ${dest}`)
}
