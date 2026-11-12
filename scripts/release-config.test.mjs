// @vitest-environment node
// Garde-fous de la publication : la clé privée ne vit que dans l'environnement GitHub « PRD ».
import { readFileSync } from 'node:fs';
import { expect, test } from 'vitest';

const read = (p) => readFileSync(p, 'utf8');
const PUBKEY =
  'dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDExOEZGRDVERDBEMjNGRUYKUldUdlA5TFFYZjJQRWFDc1oxVXMvS3FLV0U2RFp3VUtWM3lWNkk4UysvZUJhckZUMHdJbUpaTVAK';

test('release workflow is consistent', () => {
  const release = read('.github/workflows/release.yml');
  expect(release).toMatch(/tags:\s*\n\s*-\s*['"]?v\*['"]?/);
  expect(release).toMatch(/environment:\s*PRD/);
  expect(release).toContain('${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}');
  expect(release).toContain('${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}');
  expect(release).toContain('tauri-apps/tauri-action@');
  expect(release).toMatch(/contents:\s*write/);
  // Lancé à la main seulement sur un tag (la vérification refuse une branche).
  expect(release).toContain('"$GITHUB_REF_TYPE" != "tag"');

  const tag = read('.github/workflows/tag.yml');
  expect(tag).toContain('workflow_dispatch');
  expect(tag).toContain('gh workflow run release.yml --ref');
  expect(tag).not.toContain('TAURI_SIGNING_PRIVATE_KEY');

  const conf = JSON.parse(read('src-tauri/tauri.conf.json'));
  expect(conf.bundle.createUpdaterArtifacts).toBe(true);
  expect(conf.plugins.updater.pubkey).toBe(PUBKEY);
  expect(conf.plugins.updater.endpoints).toEqual(['https://github.com/Weder77/Illy/releases/latest/download/latest.json']);
  expect(conf.plugins.updater.windows.installMode).toBe('passive');

  // Hors release, pas de secrets : la CI des branches ne signe pas.
  const ci = read('.github/workflows/ci.yml');
  expect(ci).toContain('--config src-tauri/tauri.ci.conf.json');
  expect(JSON.parse(read('src-tauri/tauri.ci.conf.json')).bundle.createUpdaterArtifacts).toBe(false);
  expect(ci).not.toContain('TAURI_SIGNING_PRIVATE_KEY');
});

test('no private key in the repository files we publish', () => {
  for (const f of ['src-tauri/tauri.conf.json', '.github/workflows/release.yml', '.github/workflows/ci.yml', '.github/workflows/tag.yml', 'docs/release.md']) {
    expect(read(f)).not.toMatch(/untrusted comment: rsign encrypted secret key/i);
  }
});
