#!/usr/bin/env node
// Met le même numéro de version dans package.json (et son verrou), Cargo.toml (workspace)
// et src-tauri/tauri.conf.json.
// Usage : node scripts/bump-version.mjs 0.3.1
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');

function setJsonVersion(path, version) {
  const text = readFileSync(path, 'utf8');
  const out = text.replace(/("version"\s*:\s*")[^"]*(")/, `$1${version}$2`);
  if (out === text && !text.includes(`"version": "${version}"`)) throw new Error(`pas de champ "version" dans ${path}`);
  writeFileSync(path, out);
}

export function bump(version, dir = ROOT) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error(`version attendue au format x.y.z (par exemple 0.3.1), reçu « ${version} »`);
  setJsonVersion(join(dir, 'package.json'), version);
  setJsonVersion(join(dir, 'src-tauri', 'tauri.conf.json'), version);
  // package-lock.json garde aussi la version du paquet racine (deux endroits).
  const lockPath = join(dir, 'package-lock.json');
  let lockText = null;
  try {
    lockText = readFileSync(lockPath, 'utf8');
  } catch {
    // pas de verrou : rien à faire
  }
  if (lockText) {
    const lock = JSON.parse(lockText);
    lock.version = version;
    if (lock.packages?.['']) lock.packages[''].version = version;
    writeFileSync(lockPath, JSON.stringify(lock, null, 2) + '\n');
  }
  const cargoPath = join(dir, 'Cargo.toml');
  const cargo = readFileSync(cargoPath, 'utf8');
  const updated = cargo.replace(/(\[workspace\.package\][^[]*?\nversion\s*=\s*")[^"]*(")/, `$1${version}$2`);
  if (updated === cargo && !cargo.includes(`version = "${version}"`)) throw new Error('pas de version dans [workspace.package] de Cargo.toml');
  writeFileSync(cargoPath, updated);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    bump(process.argv[2] ?? '');
    console.log(`Version ${process.argv[2]} écrite dans package.json, Cargo.toml et src-tauri/tauri.conf.json.`);
    console.log('Pense à lancer « cargo check » pour mettre Cargo.lock à jour avant de commiter.');
  } catch (e) {
    console.error(e.message);
    process.exit(1);
  }
}
