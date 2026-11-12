// @vitest-environment node
import { cpSync, mkdirSync, mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, test } from 'vitest';
import { bump } from './bump-version.mjs';

function copyRepoFiles() {
  const dir = mkdtempSync(join(tmpdir(), 'illy-bump-'));
  mkdirSync(join(dir, 'src-tauri'));
  for (const f of ['package.json', 'package-lock.json', 'Cargo.toml', 'src-tauri/tauri.conf.json']) cpSync(f, join(dir, f));
  return dir;
}

test('updates the three versions', () => {
  const dir = copyRepoFiles();
  bump('0.2.0', dir);
  bump('0.2.0', dir); // idempotent
  expect(JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8')).version).toBe('0.2.0');
  expect(JSON.parse(readFileSync(join(dir, 'src-tauri/tauri.conf.json'), 'utf8')).version).toBe('0.2.0');
  const lock = JSON.parse(readFileSync(join(dir, 'package-lock.json'), 'utf8'));
  expect(lock.version).toBe('0.2.0');
  expect(lock.packages[''].version).toBe('0.2.0');
  const cargo = readFileSync(join(dir, 'Cargo.toml'), 'utf8');
  expect(cargo).toMatch(/\[workspace\.package\][^[]*\nversion = "0\.2\.0"/);
  // Les versions des dépendances ne bougent pas.
  expect(cargo.match(/version = "0\.2\.0"/g)).toHaveLength(1);
});

test('rejects a version that is not x.y.z', () => {
  const dir = copyRepoFiles();
  expect(() => bump('abc', dir)).toThrow(/x\.y\.z/);
  expect(() => bump('1.2', dir)).toThrow();
  expect(() => bump('v1.2.3', dir)).toThrow();
});
