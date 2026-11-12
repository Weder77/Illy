import { renderHook } from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import { defaultSettings, type Settings } from '../api/types';
import { applyAppearance, rowHeight, useAppearance } from './appearance';

afterEach(() => {
  vi.unstubAllGlobals();
  for (const k of Object.keys(document.documentElement.dataset)) delete document.documentElement.dataset[k];
});

test('applies data attributes', () => {
  const s: Settings = { ...defaultSettings, theme: 'light', accent: 'pink', textSize: 'large', density: 'compact', showSnippet: false };
  applyAppearance(s, true);
  expect(document.documentElement.dataset).toMatchObject({
    theme: 'light',
    accent: 'pink',
    textSize: 'large',
    density: 'compact',
    snippet: 'false',
  });
  applyAppearance({ ...s, theme: 'dark' }, false);
  expect(document.documentElement.dataset.theme).toBe('dark');
});

test('system theme follows prefers-color-scheme', () => {
  let dark = true;
  const listeners = new Set<() => void>();
  vi.stubGlobal('matchMedia', (q: string) => ({
    media: q,
    get matches() {
      return dark;
    },
    addEventListener: (_: string, cb: () => void) => listeners.add(cb),
    removeEventListener: (_: string, cb: () => void) => listeners.delete(cb),
  }));
  const { unmount } = renderHook(() => useAppearance({ ...defaultSettings, theme: 'system' }));
  expect(document.documentElement.dataset.theme).toBe('dark');
  // Windows passe en clair pendant qu'Illy est ouvert.
  dark = false;
  listeners.forEach((cb) => cb());
  expect(document.documentElement.dataset.theme).toBe('light');
  unmount();
  expect(listeners.size).toBe(0);
});

test('rowHeight table', () => {
  expect(rowHeight('comfortable', true)).toBe(76);
  expect(rowHeight('comfortable', false)).toBe(58);
  expect(rowHeight('compact', true)).toBe(62);
  expect(rowHeight('compact', false)).toBe(48);
  // Plus grand texte : lignes plus hautes dans la même proportion.
  expect(rowHeight('comfortable', true, 'large')).toBe(Math.round((76 * 15.5) / 14));
  expect(rowHeight('compact', false, 'small')).toBe(Math.round((48 * 13) / 14));
});
