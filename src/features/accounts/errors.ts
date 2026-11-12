import { isApiError } from '../../api/types';

/** Message d'erreur compréhensible pour l'ajout ou la mise à jour d'un compte. */
export function accountErrorText(e: unknown): { text: string; detail: string | null } {
  if (!isApiError(e)) return { text: `Erreur : ${String(e)}`, detail: null };
  switch (e.kind) {
    case 'auth':
      return {
        text: "Mot de passe refusé. Vérifie que tu as collé le mot de passe d'application (et pas ton mot de passe habituel).",
        detail: e.message,
      };
    case 'network':
      return { text: "Serveur injoignable. Vérifie ta connexion internet et l'adresse du serveur.", detail: e.message };
    case 'tls':
      return { text: 'Connexion sécurisée impossible avec ce serveur.', detail: e.message };
    case 'invalid_input':
      return { text: e.message.charAt(0).toUpperCase() + e.message.slice(1) + '.', detail: null };
    default:
      return { text: `Erreur : ${e.message}`, detail: null };
  }
}
