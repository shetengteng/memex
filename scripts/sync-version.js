#!/usr/bin/env node

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');

const files = {
  workspaceCargo: path.join(rootDir, 'Cargo.toml'),
  tauriConf: path.join(rootDir, 'tauri-app', 'src-tauri', 'tauri.conf.json'),
  packageJson: path.join(rootDir, 'tauri-app', 'package.json'),
};

let version = process.argv[2] || process.env.APP_VERSION;

if (!version) {
  const pkg = JSON.parse(fs.readFileSync(files.packageJson, 'utf8'));
  version = pkg.version;
}

version = version.replace(/^v/, '');

if (!/^\d+\.\d+\.\d+/.test(version)) {
  console.error(`Invalid version format: ${version}`);
  process.exit(1);
}

console.log(`Syncing version: ${version}`);

// workspace Cargo.toml
let cargo = fs.readFileSync(files.workspaceCargo, 'utf8');
const cargoRe = /^version = ".*?"$/m;
if (cargoRe.test(cargo)) {
  cargo = cargo.replace(cargoRe, `version = "${version}"`);
  fs.writeFileSync(files.workspaceCargo, cargo);
  console.log(`  Updated Cargo.toml`);
}

// tauri.conf.json
const tauriConf = JSON.parse(fs.readFileSync(files.tauriConf, 'utf8'));
tauriConf.version = version;
fs.writeFileSync(files.tauriConf, JSON.stringify(tauriConf, null, 2) + '\n');
console.log(`  Updated tauri.conf.json`);

// package.json
const pkg = JSON.parse(fs.readFileSync(files.packageJson, 'utf8'));
pkg.version = version;
fs.writeFileSync(files.packageJson, JSON.stringify(pkg, null, 2) + '\n');
console.log(`  Updated package.json`);

console.log(`Version sync complete: ${version}`);
