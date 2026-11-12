# Illy — Lot B : paramètres et mises à jour — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Un vrai écran Paramètres (comptes, apparence, lecture et rédaction, mises à jour, à propos) et les mises à jour automatiques depuis les releases GitHub, signées. Version 0.3.0.

**Architecture:**
- Le cœur gagne la gestion des comptes (ordre, nom, couleur, signature, serveurs), la liste des expéditeurs autorisés, le réglage « images toujours », l'espace disque et « Vider le cache local ».
- L'interface applique les réglages d'apparence par des attributs `data-*` sur `<html>`, dans toutes les fenêtres.
- Les mises à jour passent par `tauri-plugin-updater` (JS) et `tauri-plugin-process` (relance). La CI publie les releases signées sur un tag `v*`.

**Tech Stack:** l'existant, plus `tauri-plugin-updater` 2, `tauri-plugin-process` 2, `@tauri-apps/plugin-updater` 2.14.1, `@tauri-apps/plugin-process` 2.4.0, `tauri-apps/tauri-action@v0`.

**Spec:** `docs/superpowers/specs/2026-11-12-illy-redaction-parametres-design.md` § 4 et § 5.

## Global Constraints

- Interface en français ; réglages appliqués en direct, sans redémarrage.
- Endpoint des mises à jour : `https://github.com/Weder77/Illy/releases/latest/download/latest.json`.
- Clé publique (dans `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`) : `dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDExOEZGRDVERDBEMjNGRUYKUldUdlA5TFFYZjJQRWFDc1oxVXMvS3FLV0U2RFp3VUtWM3lWNkk4UysvZUJhckZUMHdJbUpaTVAK`.
- La clé privée n'apparaît jamais dans le dépôt : seulement les secrets `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` de l'environnement GitHub **`PRD`** (règles : branche `main`, tag `v*`). Le job de release déclare `environment: PRD`.
- **Aucune installation de mise à jour sans clic** ; « Plus tard » masque jusqu'au prochain démarrage.
- Jetons du thème clair, accents, tailles (13 / 14 / 15,5 px) et hauteurs de ligne (76 / 58 / 62 / 48) : exactement ceux de la spec § 4.3.
- Texte de suppression de compte : « Supprimer {email} d'Illy ? Tes mails restent sur le serveur ; seules les données de ce PC sont effacées. »
- « Vider le cache local » refusé tant que la file d'envoi n'est pas vide : « Attends la fin des envois en cours ».
- La pastille d'un compte garde sa couleur, quel que soit l'accent.

## Review Focus

1. **Modifier les serveurs avec une faute de frappe** : le test échoue, rien n'est enregistré, le compte continue de marcher. Testé à la tâche 1.
2. **Vider le cache pendant qu'un brouillon est ouvert** : les brouillons locaux et leurs pièces jointes survivent. Testé à la tâche 2.
3. **Supprimer un compte qui a des envois en file ou des brouillons** : plus aucune op ni fichier orphelin ; une fenêtre « Nouveau mail » ouverte sur ce compte affiche une erreur propre. Testé à la tâche 1.
4. **Thème « automatique » quand Windows change de thème** pendant qu'Illy est ouvert : bascule en direct. Testé à la tâche 4.
5. **CI d'une branche sans les secrets** : le job « Installateur Windows » reste vert (artefacts de mise à jour désactivés hors release). Vérifié à la tâche 3.

---

### Task 1: Cœur — gestion des comptes

**Files:**
- Modify: `crates/illy-core/src/model.rs` (`Account.signature_html: String`, `AccountPatch`), `store/accounts.rs`, `core.rs`
- Test: `store/accounts.rs` (unit), `tests/core_it.rs`

**Interfaces:**
- Produces:
  - `Account.signature_html` (TS `signatureHtml`), lu par `list_accounts` ;
  - `AccountPatch { display_name: Option<String>, color: Option<String>, signature_html: Option<String> }` (camelCase) ;
  - `Core::reorder_accounts(ids: Vec<AccountId>) -> Result<Vec<Account>>` (ids inconnus ou manquants → `InvalidInput`) ;
  - `Core::update_account(id, AccountPatch) -> Result<Account>` (nom non vide après trim ; couleur `#RRGGBB` ; signature passée par `sanitize_html` en gardant les `data:` images) ;
  - `Core::update_servers(id, imap: ServerConfig, smtp: ServerConfig) -> Result<Account>` : refus pour Gmail / Yahoo (`InvalidInput("Les serveurs de ce fournisseur ne se modifient pas")`) ; teste IMAP puis SMTP avec le mot de passe du trousseau ; n'enregistre qu'en cas de succès ; redémarre le worker ;
  - `Core::remove_account` supprime aussi les brouillons du compte et leurs dossiers `drafts/<id>`.
- [ ] **Step 1: Write the failing tests**
  - unit `reorder_sets_sort_order` ; `reorder_rejects_unknown_or_missing_ids` ; `update_account_validates` (nom vide, couleur `blue`) ; `signature_is_sanitized` (`<script>` retiré, `<b>` gardé).
  - IT `update_servers_with_bad_host_saves_nothing` (Review Focus 1) ; `update_servers_ok_restarts_sync` (port GreenMail correct → compte `ok`) ; `remove_account_cleans_drafts_and_ops` (Review Focus 3).
- [ ] **Step 2: Run them** — `cargo test -p illy-core --features insecure-plain` → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them + clippy** → PASS.
- [ ] **Step 5: Commit** — `feat(core): gestion des comptes (ordre, nom, couleur, signature, serveurs)`

### Task 2: Cœur — images, expéditeurs autorisés, espace et cache

**Files:**
- Modify: `store/allowlist.rs`, `store/messages.rs` (`message_detail` reçoit le réglage), `core.rs`, `sync/manager.rs` (`stop_all` / `start_all`)
- Test: unit + `tests/core_it.rs`

**Interfaces:**
- Produces:
  - `remote_images = always` : `open_message` renvoie `images_blocked = false`, `remote_images_allowed = true` ;
  - `Core::list_image_allowlist() -> Result<Vec<String>>` (triée), `Core::remove_from_image_allowlist(&str) -> Result<()>` ;
  - `StorageUsage { database: u64, drafts: u64, temporary: u64 }` et `Core::storage_usage() -> Result<StorageUsage>` (base = `illy.db` + `-wal` + `-shm`) ;
  - `Core::clear_cache() -> Result<()>` : `InvalidInput("Attends la fin des envois en cours")` si `list_outbox` n'est pas vide ; sinon arrête les workers, supprime `folders` (cascade : messages, parties, ops), vide `tmp/`, `VACUUM`, puis redémarre les workers. Garde comptes, réglages, allowlist, contacts, brouillons et `drafts/`.
- [ ] **Step 1: Write the failing tests** — `always_setting_allows_remote_images` ; `allowlist_list_and_remove` ; `clear_cache_keeps_accounts_settings_and_drafts` (Review Focus 2 ; les mails reviennent après la synchro) ; `clear_cache_refused_while_sending`.
- [ ] **Step 2–4:** FAIL puis PASS (+ clippy).
- [ ] **Step 5: Commit** — `feat(core): réglage des images, expéditeurs autorisés, vider le cache`

### Task 3: Tauri — commandes, module de mise à jour, publication

**Files:**
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `.github/workflows/ci.yml`, `package.json`
- Create: `.github/workflows/release.yml`, `docs/release.md` (mise à jour)

**Interfaces:**
- Produces:
  - commandes `reorder_accounts`, `update_account`, `update_servers`, `remove_account`, `list_image_allowlist`, `remove_from_image_allowlist`, `storage_usage`, `clear_cache`, `open_logs_folder`, `app_info -> { version: String, logsDir: String }` ;
  - plugins `tauri_plugin_updater::Builder::new().build()` et `tauri_plugin_process::init()` ; permissions `updater:default`, `process:allow-restart` (fenêtre `main` seulement : une capacité séparée `main-only`) ;
  - `tauri.conf.json` : `bundle.createUpdaterArtifacts: true`, `plugins.updater { pubkey, endpoints: [endpoint], windows: { installMode: "passive" } }` ;
  - `ci.yml` (job Windows) : `npm run tauri build -- --config '{"bundle":{"createUpdaterArtifacts":false}}'` (pas de secrets hors release ; Review Focus 5) ;
  - `release.yml` : `on: push: tags: ['v*']`, job `windows-latest`, `environment: PRD`, `permissions: contents: write`, `tauri-apps/tauri-action@v0` avec `tagName: ${{ github.ref_name }}`, `releaseName: 'Illy ${{ github.ref_name }}'`, `releaseDraft: false`, env `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` depuis `secrets` ; il vérifie d'abord que le tag égale `v` + la version de `tauri.conf.json`.
- [ ] **Step 1: Write the failing check** — `release_workflow_is_consistent` (Vitest, `scripts/release-config.test.mjs`) : `release.yml` contient `environment: PRD`, `tags:` `v*` et les deux secrets ; `tauri.conf.json` contient la clé publique et l'endpoint ; `ci.yml` désactive `createUpdaterArtifacts`.
- [ ] **Step 2: Run it** → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Verify** — `cargo test -p illy-app`, clippy workspace, `npm test` → PASS ; push : CI verte.
- [ ] **Step 5: Commit** — `feat(app): commandes des paramètres, module de mise à jour, workflow de publication`

### Task 4: Interface — apparence en direct

**Files:**
- Create: `src/ui/appearance.ts` (+ test)
- Modify: `src/ui/tokens.css`, `src/main.tsx`, `src/state/store.ts`, `src/features/message-list/MessageList.tsx`, `MessageRow.tsx`, `src/features/compose-window/ComposeWindow.tsx`

**Interfaces:**
- Produces:
  - `applyAppearance(settings: Settings, prefersDark: boolean, root = document.documentElement)` : pose `data-theme` (`dark` | `light`, « automatique » résolu), `data-accent`, `data-text-size`, `data-density`, `data-snippet` ;
  - `useAppearance()` : applique au chargement, à chaque `settings_changed` et à chaque changement de `prefers-color-scheme` (Review Focus 4) ;
  - `rowHeight(density, showSnippet) -> 76 | 58 | 62 | 48` ;
  - `tokens.css` : `[data-theme="light"]` avec les jetons de la spec, `[data-accent=…]` (6 couleurs), `[data-text-size=…]` sur `body`.
- [ ] **Step 1: Write the failing tests** — `applies data attributes` ; `system theme follows prefers-color-scheme` ; `rowHeight table` ; `MessageList hides snippet when showSnippet is false`.
- [ ] **Step 2–4:** FAIL puis PASS (`npm run typecheck && npm test`).
- [ ] **Step 5: Commit** — `feat(ui): thème clair, accents, taille du texte et densité en direct`

### Task 5: Interface — panneau Paramètres

**Files:**
- Create: `src/features/settings/SettingsPanel.tsx`, `AccountsSection.tsx`, `AccountDetails.tsx`, `AppearanceSection.tsx`, `ReadingSection.tsx`, `UpdatesSection.tsx`, `AboutSection.tsx`, `Settings.module.css`, tests
- Modify: `src/api/*` (méthodes de la tâche 3), `src/state/store.ts` (`showSettings`, `settingsSection`), `src/features/sidebar/Sidebar.tsx` (la roue ouvre le panneau), `src/App.tsx`

**Interfaces:**
- Consumes: tâches 1–4.
- Produces: `<SettingsPanel onClose />` (960×680 au plus, Échap ferme, menu à gauche : Comptes · Apparence · Lecture et rédaction · Mises à jour · À propos).
- Behaviour:
  - Comptes : liste réordonnable au glisser-déposer (HTML5 drag) et au clavier (Alt+↑/↓) → `reorderAccounts` ; « Ajouter un compte » ouvre l'assistant ; détails : nom (enregistré à la sortie du champ), 6 pastilles de couleur, signature (TipTap minimal, 1 s après la dernière frappe), « Changer le mot de passe », serveurs (modifiables pour OVH / manuel, lecture seule sinon) avec « Tester et enregistrer », « Supprimer ce compte » avec la confirmation exacte.
  - Apparence : thème (Sombre / Clair / Automatique), accent (6 pastilles), taille du texte (Petite / Normale / Grande), densité (Confortable / Compacte), « Afficher l'extrait ».
  - Lecture et rédaction : images (Bloquées (recommandé) / Toujours afficher), expéditeurs autorisés avec « Retirer », délai d'annulation (5 / 10 / 30 s).
  - Mises à jour : version actuelle, « Rechercher des mises à jour » et le résultat (« Illy est à jour » / version disponible / erreur).
  - À propos : version, « Ouvrir le dossier des journaux », espace utilisé, « Vider le cache local » (confirmation ; message de refus si envois en file).
- [ ] **Step 1: Write the failing tests** — `gear opens settings and Escape closes` ; `sections navigate` ; `theme choice calls setSetting` ; `drag reorders accounts` ; `delete asks the exact confirmation` ; `servers read-only for gmail` ; `remove sender from allowlist` ; `clear cache refusal is shown`.
- [ ] **Step 2–4:** FAIL puis PASS ; contrôle visuel (Playwright sur le mock, thèmes sombre et clair).
- [ ] **Step 5: Commit** — `feat(ui): panneau Paramètres`

### Task 6: Interface — bandeau de mise à jour

**Files:**
- Create: `src/features/updates/updater.ts`, `UpdateBanner.tsx` (+ css, test)
- Modify: `src/api/*` (`checkUpdate() -> { version, notes } | null`, `installUpdate(onProgress) -> Promise<void>`, mock), `src/App.tsx`

**Interfaces:**
- Produces: `<UpdateBanner />` : vérification 10 s après le démarrage ; « Illy {version} est disponible · Installer · Plus tard » ; « Installer » : progression « Téléchargement… {n} % », puis installation passive et relance ; échec : « La mise à jour n'a pas pu être installée : {raison} » ; « Plus tard » : masqué jusqu'au prochain démarrage ; aucune action automatique.
- [ ] **Step 1: Write the failing tests** — `checks 10 s after start` ; `later hides` ; `install shows progress and calls installUpdate` ; `failure message` ; `nothing shown when up to date or check fails`.
- [ ] **Step 2–4:** FAIL puis PASS.
- [ ] **Step 5: Commit** — `feat(ui): bandeau de mise à jour`

### Task 7: Version 0.3.0, vérification, checklist

**Files:**
- Create: `docs/checklists/lot-b.md`
- Modify: versions (script), `README.md`

- [ ] **Step 1:** `node scripts/bump-version.mjs 0.3.0` ; suite complète ; test de fumée Linux (Xvfb) : la roue ouvre le panneau, le thème clair s'applique ; push → CI verte.
- [ ] **Step 2:** `docs/checklists/lot-b.md` : chaque section du panneau, thème automatique avec Windows, ordre des comptes, signature dans un envoi, serveurs OVH, suppression d'un compte, vider le cache, **vraie mise à jour** (publier v0.3.0 puis v0.3.1 et vérifier que la 0.3.0 propose et installe la 0.3.1).
- [ ] **Step 3: Commit** — `chore: version 0.3.0 et checklist du lot B`
