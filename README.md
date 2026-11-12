# Illy

Client mail Windows minimaliste : trois panneaux, thème sombre, **sans IA**, **sans archivage automatique**, **sans abonnement**.

- Comptes IMAP/SMTP : Gmail et Yahoo (mot de passe d'application), OVH, ou n'importe quel serveur.
- Synchro rapide : les mails récents d'abord, l'historique complet en arrière-plan, les nouveaux mails en direct (IMAP IDLE).
- Lecture sûre : HTML nettoyé et isolé, images externes bloquées par défaut, pièces jointes exécutables jamais ouvertes directement.
- Rédaction : nouveau mail dans une fenêtre, réponse sous le mail, transfert avec les pièces jointes, carnet d'adresses automatique, brouillons copiés sur le serveur, **10 s pour annuler un envoi**, clic droit sur un mail.

Specs : `docs/superpowers/specs/` · Plans : `docs/superpowers/plans/` · Vérification sur Windows : `docs/checklists/jalon-1.md`, `docs/checklists/lot-a.md` · Publier une version : `docs/release.md`.

## Installer (Windows)

Chaque push sur GitHub fait compiler l'installateur par la CI (job « Installateur Windows ») : télécharge l'artefact **illy-windows** depuis l'onglet Actions, puis lance le `.msi` ou le `.exe`.

## Développer

Prérequis : Rust stable, Node 22, et pour les tests d'intégration, Java 21.

```bash
npm ci
npm run dev                 # interface seule dans le navigateur, avec des données fictives
npm run tauri dev           # application complète (Windows, ou Linux avec webkit2gtk)
npm test                    # tests de l'interface
scripts/fetch-greenmail.sh  # serveur mail de test
cargo test -p illy-core --features insecure-plain   # tests du cœur (dont intégration IMAP/SMTP)
npm run tauri build         # installateur (sous Windows)
node scripts/bump-version.mjs 0.2.1  # même version partout (voir docs/release.md)
```

## Organisation

- `crates/illy-core` : cœur en Rust (base SQLite, IMAP, synchro, analyse et nettoyage des mails, façade `Core`). Ne dépend pas de Tauri.
- `src-tauri` : application Tauri (commandes, événements, fenêtre, installateur).
- `src` : interface React + TypeScript (`api/` est le seul point de contact avec le cœur).

Données locales : `%LOCALAPPDATA%\Illy\` (`illy.db`, `logs\`, `tmp\`, `drafts\` pour les pièces jointes des brouillons). Mots de passe : Gestionnaire d'identification Windows.
