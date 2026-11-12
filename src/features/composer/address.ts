import type { Address } from '../../api/types';

// Volontairement simple : le cœur fait la vraie validation (lettre) avant l'envoi.
const EMAIL = /^[^\s@<>(),;:"[\]]+@[^\s@<>(),;:"[\]]+\.[^\s@<>(),;:"[\]]{2,}$/;

export function isValidEmail(email: string): boolean {
  return EMAIL.test(email.trim());
}

/** « Nom <adresse> » ou « adresse » ; `null` pour un texte vide. L'adresse peut être invalide. */
export function parseAddress(text: string): Address | null {
  const t = text.trim().replace(/[,;]+$/, '').trim();
  if (!t) return null;
  const m = /^(.*?)<\s*([^<>]+?)\s*>$/.exec(t);
  if (m) {
    const name = m[1].trim().replace(/^"(.*)"$/, '$1').trim();
    return { name: name || null, email: m[2].trim() };
  }
  return { name: null, email: t };
}

/** Découpe un texte collé (virgules, points-virgules, retours à la ligne) en adresses. */
export function parseAddressList(text: string): Address[] {
  return text
    .split(/[,;\n]+/)
    .map(parseAddress)
    .filter((a): a is Address => a !== null);
}

export function addressLabel(a: Address): string {
  return a.name?.trim() || a.email;
}
