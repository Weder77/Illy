// Apparence en direct : attributs data-* sur <html>, lus par tokens.css (toutes les fenêtres).
import { useEffect } from 'react';
import type { Density, Settings, TextSize, Theme } from '../api/types';

export const TEXT_SIZE_PX: Record<TextSize, number> = { small: 13, normal: 14, large: 15.5 };

const ROW: Record<Density, { snippet: number; plain: number }> = {
  comfortable: { snippet: 76, plain: 58 },
  compact: { snippet: 62, plain: 48 },
};

/** Hauteur d'une ligne de la liste (spec § 4.3), proportionnelle à la taille du texte. */
export function rowHeight(density: Density, showSnippet: boolean, textSize: TextSize = 'normal'): number {
  const base = showSnippet ? ROW[density].snippet : ROW[density].plain;
  return Math.round((base * TEXT_SIZE_PX[textSize]) / 14);
}

export function resolveTheme(theme: Theme, prefersDark: boolean): 'dark' | 'light' {
  if (theme === 'system') return prefersDark ? 'dark' : 'light';
  return theme;
}

export function applyAppearance(s: Settings, prefersDark: boolean, root: HTMLElement = document.documentElement) {
  root.dataset.theme = resolveTheme(s.theme, prefersDark);
  root.dataset.accent = s.accent;
  root.dataset.textSize = s.textSize;
  root.dataset.density = s.density;
  root.dataset.snippet = String(s.showSnippet);
}

function darkQuery(): MediaQueryList | null {
  return typeof window.matchMedia === 'function' ? window.matchMedia('(prefers-color-scheme: dark)') : null;
}

/** Applique les réglages, et suit le thème de Windows en direct quand le thème est « automatique ». */
export function useAppearance(s: Settings) {
  useEffect(() => {
    const q = darkQuery();
    const apply = () => applyAppearance(s, q?.matches ?? true);
    apply();
    if (s.theme !== 'system' || !q) return;
    q.addEventListener('change', apply);
    return () => q.removeEventListener('change', apply);
  }, [s]);
}
