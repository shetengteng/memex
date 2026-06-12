#!/usr/bin/env node

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');

const files = {
  workspaceCargo: path.join(rootDir, 'app', 'Cargo.toml'),
  tauriConf: path.join(rootDir, 'app', 'desktop', 'src-tauri', 'tauri.conf.json'),
  packageJson: path.join(rootDir, 'app', 'desktop', 'package.json'),
  cask: path.join(rootDir, 'app', 'Casks', 'memex.rb'),
};

const args = process.argv.slice(2);
const usage = () => {
  console.log(`Usage:
  node scripts/sync-version.js <version>          # 写入指定版本号
  node scripts/sync-version.js bump <part>        # patch | minor | major 自增
  node scripts/sync-version.js                    # 用 package.json 当前值再同步一次
Options:
  APP_VERSION=<v> node scripts/sync-version.js    # 环境变量同效
`);
};

const readPkgVersion = () => {
  const pkg = JSON.parse(fs.readFileSync(files.packageJson, 'utf8'));
  return pkg.version;
};

const bump = (current, part) => {
  const [major, minor, patch] = current.split('.').map((n) => parseInt(n, 10));
  if (part === 'major') return `${major + 1}.0.0`;
  if (part === 'minor') return `${major}.${minor + 1}.0`;
  if (part === 'patch') return `${major}.${minor}.${patch + 1}`;
  throw new Error(`unknown bump part: ${part}`);
};

let version;
if (args[0] === 'bump') {
  const part = args[1] || 'patch';
  version = bump(readPkgVersion(), part);
} else if (args[0] === '-h' || args[0] === '--help') {
  usage();
  process.exit(0);
} else {
  version = args[0] || process.env.APP_VERSION || readPkgVersion();
}

version = version.replace(/^v/, '');

if (!/^\d+\.\d+\.\d+/.test(version)) {
  console.error(`Invalid version format: ${version}`);
  usage();
  process.exit(1);
}

console.log(`Syncing version: ${version}`);

const rel = (p) => path.relative(rootDir, p);

let cargo = fs.readFileSync(files.workspaceCargo, 'utf8');
const cargoRe = /^version = ".*?"$/m;
if (cargoRe.test(cargo)) {
  cargo = cargo.replace(cargoRe, `version = "${version}"`);
  fs.writeFileSync(files.workspaceCargo, cargo);
  console.log(`  Updated ${rel(files.workspaceCargo)} (workspace.package.version)`);
}

const tauriConf = JSON.parse(fs.readFileSync(files.tauriConf, 'utf8'));
tauriConf.version = version;
fs.writeFileSync(files.tauriConf, JSON.stringify(tauriConf, null, 2) + '\n');
console.log(`  Updated ${rel(files.tauriConf)}`);

const pkg = JSON.parse(fs.readFileSync(files.packageJson, 'utf8'));
pkg.version = version;
fs.writeFileSync(files.packageJson, JSON.stringify(pkg, null, 2) + '\n');
console.log(`  Updated ${rel(files.packageJson)}`);

if (fs.existsSync(files.cask)) {
  let cask = fs.readFileSync(files.cask, 'utf8');
  const caskRe = /^(\s*version\s+)".*?"/m;
  if (caskRe.test(cask)) {
    cask = cask.replace(caskRe, `$1"${version}"`);
    fs.writeFileSync(files.cask, cask);
    console.log(`  Updated ${rel(files.cask)}`);
  }
}

console.log(`\nVersion sync complete: ${version}`);
console.log(`\nNext steps:`);
console.log(`  1. git diff   # review changes`);
console.log(`  2. git commit -am "chore: bump version to ${version}"`);
console.log(`  3. git tag v${version} && git push && git push --tags`);
console.log(`     (tag will trigger .github/workflows/release.yml build & release)`);
console.log(`  4. To upgrade local install: bash scripts/upgrade-local.sh`);
