# Illy — Jalon 1 : fondations, comptes, synchro, lecture — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Une application Windows Illy dans laquelle on ajoute ses comptes Gmail / Yahoo / OVH, qui synchronise les dossiers et les mails (hybride, IDLE), et qui affiche un mail HTML sur une carte claire, avec images bloquées, pièces jointes et passage en « lu » côté serveur.

**Architecture:** Workspace Cargo avec une bibliothèque `illy-core` (aucune dépendance à Tauri, testable sous Linux) qui contient le stockage SQLite, l'IMAP, la synchro, l'analyse MIME et la façade `Core`. Un crate `illy-app` (`src-tauri/`) expose `Core` à l'interface via des commandes et des événements Tauri. L'interface React + TypeScript (`src/`) ne parle qu'à une couche `api/`, qui a une implémentation Tauri et une implémentation simulée pour le navigateur.

**Tech Stack:**
- Rust 1.97 : `tauri` 2.12, `tokio` 1, `async-imap` 0.12 (`runtime-tokio`, sans les features par défaut), `tokio-rustls` 0.26 + `webpki-roots` 1, `lettre` 0.11 (tokio1 + rustls, pour le test SMTP), `mail-parser` 1.0 (`full_encoding`), `ammonia` 4, `rusqlite` 0.42 (`bundled`), `keyring` 4 (store Windows natif), `utf7-imap` 0.3, `directories` 6, `tracing`, `tracing-subscriber`, `tracing-appender`, `serde`, `serde_json`, `thiserror` 2, `tauri-plugin-opener` 2, `tauri-plugin-dialog` 2.
- Front : React 19, TypeScript ~5.9 (strict), Vite 8, `@tauri-apps/api` 2, Zustand 5, `@tanstack/react-virtual` 3, `@fontsource/plus-jakarta-sans` 5, Vitest 5, `@testing-library/react` 16, jsdom.
- Tests d'intégration : GreenMail standalone 2.1.15 (jar Java, Java 21).

**Spec:** `docs/superpowers/specs/2026-11-11-illy-client-mail-design.md`. Ce plan couvre le jalon 1 du § 12. Les autres jalons auront leur propre plan.

## Global Constraints

- Windows 10 et 11 uniquement pour l'application finale. `illy-core` doit compiler et passer ses tests sous Linux.
- Toute l'interface est en **français**.
- Thème **sombre uniquement**. Jetons exacts du § 6 de la spec : fond `#0D0F13`, panneau `#16191F`, surélevé `#1F232B`, bordure `rgba(255,255,255,0.06)`, texte `#F2F4F8`, secondaire `#8B93A1`, accent `#2F7BFF`, halo `0 0 18px rgba(47,123,255,0.55)`, liseré `1px solid rgba(47,123,255,0.35)`, avertissement `#F5A524`, erreur `#F25F5C`. Rayons : panneaux 20 px, lignes et champs 12 px, pilules 999 px. Police Plus Jakarta Sans embarquée. Carte du mail : `#FFFFFF`, rayon 16 px, marge intérieure 24 px.
- **Aucune requête réseau depuis l'interface.** Seul le cœur Rust parle aux serveurs, et l'interface lit la base locale.
- Les corps sont toujours récupérés avec `BODY.PEEK[...]`. Rien ne marque un mail comme lu, sauf `open_message`.
- Aucune action n'archive ni ne déplace un mail dans ce jalon.
- Les mots de passe ne vont que dans le Gestionnaire d'identification Windows (service `Illy`, utilisateur = `account:<id>`). Ils ne figurent ni en base ni dans les logs. Les logs ne contiennent aucun contenu ni objet de mail.
- Données dans `%LOCALAPPDATA%\Illy\` : `illy.db`, `logs\`, `tmp\` (vidé au démarrage).
- La connexion sans TLS (`TlsMode::Plain`) n'existe que derrière la feature Cargo `insecure-plain`, utilisée par les tests d'intégration. `Core::add_account` la refuse sinon.
- Couleurs des comptes, attribuées dans l'ordre `sort_order % 6` : `#2F7BFF`, `#8B5CF6`, `#14B8A6`, `#F59E0B`, `#EC4899`, `#22C55E`.
- Paramètres de synchro par défaut (`WorkerConfig::default()`) :
  - `header_batch` = 500 ;
  - `recent_body_days` = 30 ;
  - `poll_interval` = 60 s (boîte de réception, si le serveur ne gère pas IDLE) ;
  - `other_folders_interval` = 5 min ;
  - `idle_renew` = 25 min ;
  - `flags_recent_window` = 2000 ;
  - `backoff_min` = 5 s, `backoff_max` = 5 min.

## Review Focus

1. **Mail aux en-têtes encodés ou au charset exotique** (objet en `=?ISO-8859-1?Q?...?=`, corps en `windows-1252`, nom de dossier en UTF-7 modifié comme `R&AOk-ception`). L'utilisateur s'attend à voir les accents correctement. Couvert par les tests des tâches 4 et 7.
2. **HTML hostile** (`<script>`, `onerror=`, `javascript:`, `<meta http-equiv=refresh>`, `<link rel=stylesheet href=https://…>`, CSS `url(https://…)` qui sert de pixel espion). L'utilisateur s'attend à ce que rien ne s'exécute et à ce qu'aucune requête ne parte tant que les images sont bloquées. Couvert par le test de la tâche 5.
3. **Mot de passe révoqué alors que le compte tourne.** L'utilisateur s'attend à un point orange avec « Mot de passe refusé », et à ce qu'Illy cesse de réessayer en boucle. Couvert par le test de la tâche 10.
4. **Mail supprimé ailleurs pendant qu'il est affiché, ou `UIDVALIDITY` changé.** L'utilisateur s'attend à ce que la liste se mette à jour sans erreur et sans doublon. Couvert par les tests des tâches 8 et 9.
5. **Coupure réseau au moment d'ouvrir un mail non lu.** L'utilisateur s'attend à ce que le mail passe lu localement, et sur le serveur dès le retour du réseau. Couvert par le test de la tâche 11.

---

## Structure des fichiers

```
Cargo.toml                         workspace : crates/illy-core, src-tauri
package.json, vite.config.ts, tsconfig.json, index.html, vitest.setup.ts
scripts/fetch-greenmail.sh         télécharge le jar GreenMail dans target/greenmail/
.github/workflows/ci.yml

crates/illy-core/
  Cargo.toml                       feature `insecure-plain`
  src/lib.rs                       ré-exports publics
  src/error.rs                     CoreError
  src/model.rs                     types partagés (sérialisés vers l'interface)
  src/events.rs                    CoreEvent, EventSink
  src/providers.rs                 profils Gmail/Yahoo/OVH, détection par domaine
  src/secrets.rs                   SecretStore, MemorySecretStore, KeyringSecretStore (Windows)
  src/store/mod.rs                 Store (ouverture, WAL, migrations)
  src/store/schema.rs              SQL v1
  src/store/accounts.rs            CRUD comptes
  src/store/folders.rs             dossiers
  src/store/messages.rs            en-têtes, parties, corps, listes, lu/non lu
  src/store/ops.rs                 file d'attente des actions
  src/store/allowlist.rs           expéditeurs autorisés pour les images
  src/mime/mod.rs
  src/mime/headers.rs              bloc d'en-têtes → ParsedHeaders
  src/mime/structure.rs            BODYSTRUCTURE → Vec<PartInfo>
  src/mime/decode.rs               partie MIME → texte ou octets
  src/mime/sanitize.rs             nettoyage HTML, blocage images, cid → data:
  src/imap/mod.rs
  src/imap/conn.rs                 connexion TLS/Plain, login, capacités
  src/imap/folders.rs              LIST → FolderInfo (rôle, masqué, nom affiché)
  src/imap/fetch.rs                FETCH en-têtes et parties
  src/sync/mod.rs
  src/sync/diff.rs                 calculs purs de synchro
  src/sync/folder_sync.rs          synchro d'un dossier
  src/sync/body.rs                 téléchargement d'un corps, d'une pièce jointe
  src/sync/ops_exec.rs             exécution de la file ops
  src/sync/worker.rs               worker d'un compte (boucle, IDLE, priorités)
  src/sync/manager.rs              SyncManager (un worker par compte)
  src/core.rs                      façade Core utilisée par les commandes
  tests/common/mod.rs              démarrage de GreenMail, helpers APPEND
  tests/fixtures/*.eml             mails fabriqués pour les tests
  tests/sync_it.rs, tests/core_it.rs

src-tauri/
  Cargo.toml, build.rs, tauri.conf.json, capabilities/default.json, icons/
  src/main.rs, src/lib.rs          démarrage : dossiers, logs, Core, pont d'événements
  src/commands.rs                  commandes Tauri

src/
  main.tsx, App.tsx, App.module.css
  api/types.ts                     miroir TS de model.rs et events.rs
  api/index.ts                     interface Api + choix Tauri ou simulée
  api/tauri.ts, api/mock.ts
  state/store.ts                   Zustand
  ui/tokens.css, ui/global.css, ui/TitleBar.tsx, ui/icons.tsx
  features/sidebar/Sidebar.tsx (+ .module.css, .test.tsx)
  features/message-list/MessageList.tsx, MessageRow.tsx, SearchBar.tsx (+ css, tests)
  features/reader/Reader.tsx, MailFrame.tsx, ImagesBanner.tsx, Attachments.tsx (+ css, tests)
  features/accounts/AddAccountWizard.tsx, UpdatePasswordDialog.tsx (+ css, tests)
```

---

### Task 1: Workspace, `illy-core` vide, outillage front et CI

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `crates/illy-core/Cargo.toml`, `crates/illy-core/src/lib.rs`, `crates/illy-core/src/error.rs`
- Create: `package.json`, `tsconfig.json`, `vite.config.ts`, `vitest.setup.ts`, `index.html`, `src/main.tsx`, `src/App.tsx`, `src/App.test.tsx`
- Create: `scripts/fetch-greenmail.sh`, `.github/workflows/ci.yml`

**Interfaces:**
- Produces: `illy_core::CoreError`, the error enum used by every later task:
  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum CoreError {
      #[error("base de données : {0}")] Db(#[from] rusqlite::Error),
      #[error("mot de passe refusé : {0}")] Auth(String),
      #[error("serveur injoignable : {0}")] Network(String),
      #[error("erreur TLS : {0}")] Tls(String),
      #[error("erreur IMAP : {0}")] Imap(String),
      #[error("erreur SMTP : {0}")] Smtp(String),
      #[error("introuvable")] NotFound,
      #[error("{0}")] InvalidInput(String),
      #[error("trousseau : {0}")] Secret(String),
      #[error("fichier : {0}")] Io(#[from] std::io::Error),
  }
  pub type Result<T> = std::result::Result<T, CoreError>;
  ```
  `CoreError` serializes (custom `Serialize`) as `{ "kind": "auth" | "network" | "tls" | "imap" | "smtp" | "not_found" | "invalid_input" | "secret" | "db" | "io", "message": String }`.
- Produces: npm scripts `dev`, `build`, `test` (`vitest run`), `typecheck` (`tsc --noEmit`), `tauri`.

- [ ] **Step 1: Write the failing tests**
  - `crates/illy-core/src/error.rs` unit test `auth_error_serializes_with_kind`: `serde_json::to_value(CoreError::Auth("NO".into()))` equals `{"kind":"auth","message":"mot de passe refusé : NO"}`.
  - `src/App.test.tsx`: `renders app root` renders `<App/>` and expects `screen.getByTestId('app-root')`.
- [ ] **Step 2: Run them to verify they fail**
  Run: `cargo test -p illy-core` and `npm test`. Expected: compile errors or missing module failures.
- [ ] **Step 3: Implement**
  - Root `Cargo.toml`: `[workspace] members = ["crates/illy-core", "src-tauri"]`, `resolver = "2"`. `src-tauri` is created in Task 13, so list only `crates/illy-core` until then.
  - `illy-core` gets all the Rust dependencies listed in the Tech Stack except `tauri*`, with `keyring` under `[target.'cfg(windows)'.dependencies]` and `[features] insecure-plain = []`.
  - Front: Vite + React + TS strict; Vitest with `environment: 'jsdom'` and `setupFiles: ['vitest.setup.ts']` (imports `@testing-library/jest-dom/vitest`). `App` renders `<div data-testid="app-root" />`.
  - `scripts/fetch-greenmail.sh` downloads `https://repo1.maven.org/maven2/com/icegreen/greenmail-standalone/2.1.15/greenmail-standalone-2.1.15.jar` into `target/greenmail/greenmail-standalone.jar`. It does nothing if the jar is already there.
  - `ci.yml`:
    - job `test` on `ubuntu-latest`: `actions/setup-java` with Java 21, `scripts/fetch-greenmail.sh`, `cargo test -p illy-core --features insecure-plain`, `npm ci`, `npm run typecheck`, `npm test`;
    - job `build-windows` on `windows-latest`: `npm ci`, `npm run tauri build`, upload `target/release/bundle/msi/*.msi` and `target/release/bundle/nsis/*.exe` as the artifact `illy-windows`. Leave this job commented out until Task 13 creates `src-tauri`.
  - `.gitignore`: `target/`, `node_modules/`, `dist/`.
- [ ] **Step 4: Run them to verify they pass**
  Run: `cargo test -p illy-core && npm run typecheck && npm test`. Expected: all PASS.
- [ ] **Step 5: Commit**
  `git commit -m "chore: workspace Rust, outillage front et CI"`

### Task 2: Types partagés, événements, profils de fournisseurs, secrets

**Files:**
- Create: `crates/illy-core/src/model.rs`, `src/events.rs`, `src/providers.rs`, `src/secrets.rs`
- Test: unit tests in the same files

**Interfaces:**
- Produces (`model.rs`). Every type derives `Serialize, Deserialize, Clone, Debug, PartialEq`; enums use `#[serde(rename_all = "snake_case")]`; structs use `#[serde(rename_all = "camelCase")]`.
  ```rust
  pub type AccountId = i64; pub type FolderId = i64; pub type MessageId = i64;
  pub enum Provider { Gmail, Yahoo, Ovh, Custom }
  pub enum TlsMode { Implicit, StartTls, Plain }
  pub struct ServerConfig { pub host: String, pub port: u16, pub tls: TlsMode }
  pub struct NewAccount { pub email: String, pub display_name: String, pub provider: Provider, pub imap: ServerConfig, pub smtp: ServerConfig }
  pub enum AccountStatus { Ok, Syncing, AuthError, Offline, Error }
  pub struct Account { pub id: AccountId, pub email: String, pub display_name: String, pub provider: Provider,
                       pub imap: ServerConfig, pub smtp: ServerConfig, pub color: String, pub sort_order: i64,
                       pub status: AccountStatus, pub last_error: Option<String> }
  pub enum FolderRole { Inbox, Drafts, Sent, Spam, Trash, Custom }   // declaration order = display order
  pub struct Folder { pub id: FolderId, pub account_id: AccountId, pub imap_path: String, pub display_name: String,
                      pub role: FolderRole, pub unread_count: u32, pub total_count: u32 }
  pub struct Address { pub name: Option<String>, pub email: String }
  pub struct MessageSummary { pub id: MessageId, pub account_id: AccountId, pub folder_id: FolderId,
                              pub from: Address, pub subject: String, pub snippet: String, pub date: i64,
                              pub seen: bool, pub has_attachments: bool }
  pub struct Attachment { pub part_path: String, pub filename: String, pub mime_type: String, pub size: u64, pub can_open: bool }
  #[serde(tag = "kind", content = "content")] pub enum MessageBody { Html(String), Text(String) }
  pub struct MessageDetail { pub summary: MessageSummary, pub to: Vec<Address>, pub cc: Vec<Address>,
                             pub body: MessageBody, pub attachments: Vec<Attachment>,
                             pub images_blocked: bool, pub degraded: bool }
  #[serde(tag = "kind", content = "folderId")] pub enum ListScope { Folder(FolderId), UnifiedInbox }
  pub struct ProviderGuess { pub provider: Option<Provider>, pub imap: Option<ServerConfig>, pub smtp: Option<ServerConfig> }
  ```
- Produces (`events.rs`):
  ```rust
  #[serde(tag = "type", rename_all = "snake_case")]
  pub enum CoreEvent {
      AccountStatus { account_id: AccountId, status: AccountStatus, error: Option<String> },
      FoldersChanged { account_id: AccountId },
      FolderChanged { account_id: AccountId, folder_id: FolderId },
      MessageUpdated { message_id: MessageId },
      SyncProgress { account_id: AccountId, folder_id: FolderId, fetched: u32, total: u32 },
  }
  pub trait EventSink: Send + Sync + 'static { fn emit(&self, event: CoreEvent); }
  pub struct RecordingSink(pub std::sync::Mutex<Vec<CoreEvent>>);   // impl EventSink; used by tests
  ```
  Field names inside the variants serialize in camelCase.
- Produces (`providers.rs`):
  - `pub fn detect_provider(email: &str) -> ProviderGuess`
  - `pub fn ovh_preset() -> (ServerConfig, ServerConfig)`
  - `pub fn color_for(sort_order: i64) -> &'static str`
  - `pub fn is_dangerous_filename(name: &str) -> bool`
- Produces (`secrets.rs`):
  - `pub trait SecretStore: Send + Sync + 'static`, with `fn set(&self, account: AccountId, password: &str) -> Result<()>`, `fn get(&self, account: AccountId) -> Result<Option<String>>` and `fn delete(&self, account: AccountId) -> Result<()>`;
  - `pub struct MemorySecretStore` (always compiled);
  - `#[cfg(windows)] pub struct KeyringSecretStore`, with service `"Illy"` and user `format!("account:{id}")`, mapping keyring errors to `CoreError::Secret`.

- [ ] **Step 1: Write the failing tests**
  - `detect_gmail`: `detect_provider("Moi@GMail.com")` gives `provider == Some(Gmail)`, imap `imap.gmail.com:993 Implicit`, smtp `smtp.gmail.com:465 Implicit`. Same for `googlemail.com`.
  - `detect_yahoo`: `yahoo.fr`, `yahoo.com`, `ymail.com` give `Some(Yahoo)`, imap `imap.mail.yahoo.com:993`, smtp `smtp.mail.yahoo.com:465`, both `Implicit`.
  - `detect_unknown`: `contact@mondomaine.fr` gives `ProviderGuess { provider: None, imap: None, smtp: None }`.
  - `ovh_preset_values`: `ssl0.ovh.net:993 Implicit` and `ssl0.ovh.net:465 Implicit`.
  - `colors_cycle`: `color_for(0) == "#2F7BFF"`, `color_for(6) == "#2F7BFF"`, `color_for(1) == "#8B5CF6"`.
  - `dangerous_names`: true for `a.EXE`, `x.bat`, `y.cmd`, `z.ps1`, `w.vbs`, `v.js`, `u.msi`, `t.scr`; false for `devis.pdf`, `photo.jpg`, `noext`.
  - `event_serialization`: `CoreEvent::FolderChanged{account_id:1,folder_id:2}` serializes to `{"type":"folder_changed","accountId":1,"folderId":2}`.
  - `list_scope_serialization`: `ListScope::Folder(3)` serializes to `{"kind":"folder","folderId":3}`; `UnifiedInbox` serializes to `{"kind":"unified_inbox"}`.
  - `memory_secret_roundtrip`: set, get returns `Some`, delete, get returns `None`.
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core`. Expected: FAIL (missing items).
- [ ] **Step 3: Implement** the items above. The domain comparison is case-insensitive and applies to the part after the last `@`.
- [ ] **Step 4: Run them to verify they pass** — `cargo test -p illy-core`. Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(core): types partagés, événements, fournisseurs, secrets"`

### Task 3: Store SQLite — schéma, comptes, dossiers, allowlist

**Files:**
- Create: `crates/illy-core/src/store/mod.rs`, `schema.rs`, `accounts.rs`, `folders.rs`, `allowlist.rs`
- Test: unit tests in the same files, all using `Store::open_in_memory()`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone)] pub struct Store { /* Arc<Mutex<rusqlite::Connection>> */ }
  impl Store {
      pub fn open(path: &Path) -> Result<Store>;          // WAL, foreign_keys=ON, migrations
      pub fn open_in_memory() -> Result<Store>;
      // accounts.rs
      pub fn insert_account(&self, a: &NewAccount) -> Result<Account>;   // sort_order = max+1, color = color_for(sort_order), status Ok
      pub fn list_accounts(&self) -> Result<Vec<Account>>;               // by sort_order
      pub fn get_account(&self, id: AccountId) -> Result<Account>;        // NotFound if absent
      pub fn set_account_status(&self, id: AccountId, s: AccountStatus, err: Option<&str>) -> Result<()>;
      pub fn delete_account(&self, id: AccountId) -> Result<()>;          // cascades to folders/messages/parts/ops
      // folders.rs
      pub fn upsert_folders(&self, account: AccountId, found: &[FolderInfo]) -> Result<()>; // insert new, update role/hidden/display_name, delete folders no longer listed
      pub fn list_folders(&self, account: AccountId) -> Result<Vec<Folder>>;  // hidden excluded; sorted by role order then display_name (case-insensitive)
      pub fn folder_state(&self, id: FolderId) -> Result<FolderSyncState>;
      pub fn set_folder_state(&self, id: FolderId, st: &FolderSyncState) -> Result<()>;
      pub fn syncable_folders(&self, account: AccountId) -> Result<Vec<(FolderId, String, FolderRole)>>; // non-hidden: (id, imap_path, role)
      // allowlist.rs
      pub fn allow_images_from(&self, sender: &str) -> Result<()>;   // lowercase
      pub fn images_allowed_for(&self, sender: &str) -> Result<bool>;
  }
  pub struct FolderInfo { pub imap_path: String, pub display_name: String, pub role: FolderRole, pub hidden: bool }
  pub struct FolderSyncState { pub uidvalidity: Option<u32>, pub uidnext: Option<u32>, pub highest_modseq: Option<u64>, pub last_sync_at: Option<i64> }
  ```
  `unread_count` and `total_count` in `Folder` are computed by the SQL query from `messages`; they are not stored.
- Schema v1 (`schema.rs`) creates every table of spec § 4 (`accounts`, `folders`, `messages`, `parts`, `messages_fts`, `contacts`, `image_allowlist`, `drafts`, `ops`):
  - `accounts` stores `imap_host/imap_port/imap_tls/smtp_host/smtp_port/smtp_tls` and `signature_html TEXT NOT NULL DEFAULT ''`;
  - `messages` has `UNIQUE(folder_id, uid)`, `INDEX(folder_id, date DESC)` and `INDEX(account_id, message_id)`;
  - `parts` has `UNIQUE(message_id, part_path)`, and takes the spec's columns with `is_attachment` replaced by `kind TEXT` (`body_text` | `body_html` | `inline` | `attachment`), plus `encoding TEXT` and `charset TEXT`;
  - `messages` takes the spec's columns plus `degraded INTEGER NOT NULL DEFAULT 0`;
  - every `ON DELETE CASCADE` follows the ownership account → folder → message → part;
  - `PRAGMA user_version = 1`.

  `messages_fts`, `contacts` and `drafts` are created now but stay unused until later milestones.

- [ ] **Step 1: Write the failing tests**
  - `open_sets_wal_and_version`: on a temp file, `PRAGMA journal_mode` gives `wal` and `user_version` gives `1`. Reopening the same file does not fail (migrations are idempotent).
  - `insert_and_list_accounts`: two inserts give sort_order 0 and 1, colors `#2F7BFF` and `#8B5CF6`, status `Ok`.
  - `status_roundtrip`: after `set_account_status(id, AuthError, Some("NO [AUTH]"))`, `get_account` returns that status and that error.
  - `upsert_folders_insert_update_delete`: first call with INBOX + "Factures" + hidden "[Gmail]/Tous les messages". `list_folders` returns 2 folders, Inbox first. Second call without "Factures" and with "Travaux": `list_folders` returns INBOX + Travaux.
  - `folders_sorted_by_role_then_name`: Custom "b", Trash, Inbox, Custom "A", Sent are listed as `Inbox, Sent, Trash, A, b`.
  - `allowlist_case_insensitive`: `allow_images_from("Marie@Ex.fr")`, then `images_allowed_for("marie@ex.fr")` is true.
  - `delete_account_cascades`: after deleting an account with folders, `list_folders` for it is empty.
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core store::`. Expected: FAIL.
- [ ] **Step 3: Implement** the API above. Use a single `Mutex<Connection>` for now; the spec's single-writer, multiple-readers split is deferred until a measurement shows a need (Task 17 measures it).
- [ ] **Step 4: Run them to verify they pass** — `cargo test -p illy-core store::`. Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(core): store SQLite, comptes, dossiers, allowlist"`

### Task 4: MIME — en-têtes, structure, décodage

**Files:**
- Create: `crates/illy-core/src/mime/mod.rs`, `headers.rs`, `structure.rs`, `decode.rs`
- Create fixtures: `crates/illy-core/tests/fixtures/latin1_subject.eml`, `alternative_html.eml`, `inline_cid_and_pdf.eml`, `plain_windows1252.eml`, `broken_mime.eml` (hand-written, fictional people)

**Interfaces:**
- Produces:
  ```rust
  pub struct ParsedHeaders { pub subject: String, pub from: Address, pub to: Vec<Address>, pub cc: Vec<Address>,
                             pub date: Option<i64>, pub message_id: Option<String>, pub in_reply_to: Option<String>,
                             pub references: Vec<String> }
  pub fn parse_header_block(raw: &[u8]) -> ParsedHeaders;          // never fails: missing fields → empty / None
  pub struct PartInfo { pub part_path: String, pub mime_type: String, pub filename: Option<String>, pub size: u64,
                        pub content_id: Option<String>, pub encoding: String, pub charset: Option<String>,
                        pub kind: PartKind }
  pub enum PartKind { BodyText, BodyHtml, Inline, Attachment }
  pub fn flatten_structure(bs: &imap_proto::types::BodyStructure<'_>) -> Vec<PartInfo>;
  pub enum Decoded { Text(String), Bytes(Vec<u8>) }
  pub fn decode_part(mime_header: &[u8], body: &[u8]) -> Decoded;   // text/* → Text (charset-decoded), others → Bytes
  pub fn snippet_from(text: &str) -> String;                        // whitespace collapsed, max 200 chars
  pub fn html_to_text(html: &str) -> String;                        // for snippets of HTML-only mails
  ```
- Classification rules for `flatten_structure`:
  - Part numbering follows IMAP: a single-part message is `"1"`, and multipart children are `"1"`, `"2"`, `"1.2"`….
  - `Attachment` when: disposition is `attachment`; or there is a filename and no content-id; or the type is `message/rfc822` (filename falls back to `message.eml`).
  - `Inline` for `image/*` with a content-id that is not an attachment.
  - `BodyHtml` / `BodyText` for `text/html` / `text/plain` that are not attachments.
  - Content-ids are stored without `<>`.

- [ ] **Step 1: Write the failing tests**
  - `header_latin1_encoded_word`: `latin1_subject.eml` gives subject `"Réunion à 14h – compte rendu"`; from name `"Élodie Martin"`, email `elodie@exemple.fr`.
  - `header_missing_fields`: `b"X-Foo: bar\r\n\r\n"` gives subject `""`, from email `""`, date `None`.
  - `structure_alternative`: a hand-built `BodyStructure::Multipart` (alternative: text/plain + text/html) gives `[("1", BodyText), ("2", BodyHtml)]`.
  - `structure_mixed_inline_pdf`: mixed[ related[ html, image/png cid `logo@x` ], application/pdf "devis.pdf" attachment ] gives paths `1.1` BodyHtml, `1.2` Inline (content_id `logo@x`), `2` Attachment (filename `devis.pdf`).
  - `structure_single_part`: a single text/plain gives `[("1", BodyText)]`.
  - `decode_qp_windows1252`: MIME header `Content-Type: text/plain; charset=windows-1252\r\nContent-Transfer-Encoding: quoted-printable` with body `caf=E9` gives `Text("café")`.
  - `decode_base64_binary`: `Content-Type: image/png\r\nContent-Transfer-Encoding: base64` gives `Bytes` equal to the decoded bytes.
  - `snippet_collapses_and_truncates`: 500 chars with newlines give ≤ 200 chars and no `\n`.
  - `html_to_text_strips_tags`: `<p>Bonjour <b>Marie</b></p><style>p{}</style>` gives `"Bonjour Marie"`.
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core mime::`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - `parse_header_block` and `decode_part` use `mail_parser::MessageParser`. `decode_part` parses `mime_header + b"\r\n" + body` as one single-part message and reads its decoded text or binary content.
  - `html_to_text` drops `<style>`/`<script>` contents, strips tags, decodes entities and collapses whitespace.
- [ ] **Step 4: Run them to verify they pass** — `cargo test -p illy-core mime::`. Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(core): analyse MIME (en-têtes, structure, décodage)"`

### Task 5: Nettoyage HTML et blocage des images

**Files:**
- Create: `crates/illy-core/src/mime/sanitize.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct SanitizedHtml { pub html: String, pub has_remote_images: bool }
  pub fn sanitize_html(html: &str, inline: &HashMap<String, (String, Vec<u8>)>) -> SanitizedHtml; // inline: content_id → (mime_type, bytes)
  pub fn unblock_images(html: &str) -> String;     // data-blocked-src → src, data-blocked-srcset → srcset
  pub fn text_to_html(text: &str) -> String;        // escaped, line breaks kept, http(s) URLs as <a>
  ```
- Rules:
  - `ammonia` removes `script`, `style`'s `@import`, `iframe`, `object`, `embed`, `form`, `input`, `button`, `base`, `meta`, `link`, every `on*` attribute, and the `javascript:` / `vbscript:` / `data:` schemes on `href`.
  - Allowed: inline `style` attributes and `<style>` elements, both needed for newsletters. Inside them, `url(http…)` is replaced by `url()`. Remote CSS backgrounds therefore never load, even when images are allowed (decision for J1: keeps the tracking surface minimal).
  - `img src="cid:X"` becomes `data:<mime>;base64,…` when X is in `inline`.
  - `img src`/`srcset` starting with `http://` or `https://` are renamed to `data-blocked-src`/`data-blocked-srcset`, and `has_remote_images = true`.
  - Every `<a>` gets `target="_blank" rel="noopener noreferrer"`.

- [ ] **Step 1: Write the failing tests** (in `sanitize.rs`)
  - `removes_scripts_and_handlers`: input `<p onclick="x()">a</p><script>alert(1)</script><img src="x" onerror="y()">` → output contains neither `script`, `onclick` nor `onerror`.
  - `removes_javascript_links`: `<a href="javascript:alert(1)">x</a>` → no `javascript:`.
  - `removes_meta_refresh_base_link`: `<meta http-equiv="refresh" content="0;url=https://evil">`, `<base href="https://evil/">` and `<link rel="stylesheet" href="https://t.example/x.css">` are all absent from the output.
  - `blocks_remote_images`: `<img src="https://t.example/p.gif" srcset="https://t.example/a.png 2x">` → output contains `data-blocked-src="https://t.example/p.gif"`, contains no ` src="https://`, and `has_remote_images == true`.
  - `neutralizes_css_urls`: `<div style="background:url(https://t.example/b.png)">` and `<style>.a{background:url('https://t.example/c.png')}</style>` → output contains no `https://t.example`.
  - `inlines_cid_images`: `<img src="cid:logo@x">` with inline `logo@x → ("image/png", [1,2,3])` → `src="data:image/png;base64,AQID"`, and `has_remote_images == false`.
  - `unblock_restores_src`: `unblock_images` on the blocked output contains `src="https://t.example/p.gif"`.
  - `text_to_html_links_and_escapes`: `"a<b>\nvoir https://ex.fr/x"` → contains `a&lt;b&gt;<br>` and `<a href="https://ex.fr/x"`.
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core sanitize`. Expected: FAIL.
- [ ] **Step 3: Implement.** Configure `ammonia::Builder` (allowed tags and attributes, `attribute_filter` for the src/srcset/style rewrite, `url_schemes` = http, https, mailto, cid, data limited to images). Rewrite the content of `<style>` elements with a regex on `url\(\s*['"]?https?:`.
- [ ] **Step 4: Run them to verify they pass** — `cargo test -p illy-core sanitize`. Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(core): nettoyage HTML et blocage des images distantes"`

### Task 6: Store — messages, parties, corps, listes ; file `ops`

**Files:**
- Create: `crates/illy-core/src/store/messages.rs`, `crates/illy-core/src/store/ops.rs`

**Interfaces:**
- Consumes: `ParsedHeaders`, `PartInfo` (Task 4), the model types (Task 2).
- Produces:
  ```rust
  pub struct NewHeader { pub uid: u32, pub headers: ParsedHeaders, pub internal_date: i64, pub size: u64,
                         pub seen: bool, pub answered: bool, pub parts: Vec<PartInfo> }
  impl Store {
      pub fn upsert_headers(&self, account: AccountId, folder: FolderId, items: &[NewHeader]) -> Result<usize>; // one transaction; date = headers.date.unwrap_or(internal_date); has_attachments = any Attachment part
      pub fn known_uids(&self, folder: FolderId) -> Result<Vec<u32>>;
      pub fn delete_uids(&self, folder: FolderId, uids: &[u32]) -> Result<usize>;
      pub fn clear_folder(&self, folder: FolderId) -> Result<()>;
      pub fn apply_flags(&self, folder: FolderId, flags: &[(u32, bool /*seen*/, bool /*answered*/)]) -> Result<Vec<MessageId>>; // returns ids that changed
      pub fn list_messages(&self, scope: ListScope, offset: u32, limit: u32) -> Result<Vec<MessageSummary>>; // date DESC, id DESC
      pub fn count_messages(&self, scope: ListScope) -> Result<u32>;
      pub fn message_location(&self, id: MessageId) -> Result<(AccountId, FolderId, u32 /*uid*/)>;
      pub fn body_parts_to_fetch(&self, id: MessageId) -> Result<Vec<PartInfo>>;   // BodyText, BodyHtml, Inline
      pub fn store_body(&self, id: MessageId, text: Option<&str>, html: Option<&SanitizedHtml>, snippet: &str,
                        inline_data: &[(String /*part_path*/, Vec<u8>)], degraded: bool) -> Result<()>; // body_state = ready
      pub fn store_part_data(&self, id: MessageId, part_path: &str, data: &[u8]) -> Result<()>;
      pub fn part(&self, id: MessageId, part_path: &str) -> Result<(PartInfo, Option<Vec<u8>>)>;
      pub fn message_detail(&self, id: MessageId, show_images: bool) -> Result<Option<MessageDetail>>; // None if body not ready
      pub fn set_seen_local(&self, id: MessageId, seen: bool) -> Result<bool>;    // true if it changed
      pub fn recent_without_body(&self, account: AccountId, since: i64, limit: u32) -> Result<Vec<MessageId>>; // newest first
      pub fn any_without_body(&self, account: AccountId, limit: u32) -> Result<Vec<MessageId>>;               // newest first
  }
  // ops.rs
  pub enum OpKind { SetSeen }                     // more kinds in later milestones
  pub struct Op { pub id: i64, pub account_id: AccountId, pub kind: OpKind, pub payload: serde_json::Value, pub attempts: u32 }
  impl Store {
      pub fn enqueue_op(&self, account: AccountId, kind: OpKind, payload: serde_json::Value) -> Result<i64>;
      pub fn due_ops(&self, account: AccountId, now: i64) -> Result<Vec<Op>>;          // next_attempt_at <= now, by id
      pub fn op_done(&self, id: i64) -> Result<()>;
      pub fn op_retry(&self, id: i64, next_attempt_at: i64, error: &str) -> Result<()>; // attempts += 1
  }
  ```
- Rules for `message_detail`:
  - `images_blocked = has_remote_images && !show_images && !images_allowed_for(from)`;
  - when images are not blocked, the HTML goes through `unblock_images`;
  - `body` is `Html(body_html)` if present, otherwise `Text(body_text)`;
  - `attachments` lists the `Attachment` parts with `can_open = !is_dangerous_filename(filename)`;
  - `degraded` comes from the stored flag.
  - `ListScope::UnifiedInbox` covers every folder with role Inbox.

- [ ] **Step 1: Write the failing tests**
  - `upsert_and_list_by_folder`: 3 headers with dates 1, 3, 2 → `list_messages(Folder, 0, 10)` returns them in date order 3, 2, 1, and `count_messages` returns 3. Upserting the same uid again does not duplicate it.
  - `unified_inbox_spans_accounts`: inboxes of 2 accounts plus a "Factures" folder → unified view lists only the inbox messages, sorted by date.
  - `paging`: 250 messages; `list_messages(.., 200, 100)` returns 50.
  - `apply_flags_reports_changes`: 2 messages unseen; apply `[(uid1, true, false)]` → returns `[id1]`, and the folder's `unread_count` goes from 2 to 1.
  - `delete_and_clear`: `delete_uids` removes the rows; `clear_folder` empties the folder.
  - `detail_none_until_body`: `message_detail` is `None` before `store_body`, and `Some` with `Html` after.
  - `detail_blocks_then_shows_images`: stored html with `has_remote_images` → `images_blocked == true` and the body contains `data-blocked-src`. With `show_images = true`: `images_blocked == false` and the body contains `src="https://`. After `allow_images_from(sender)`: not blocked, even with `show_images = false`.
  - `attachments_can_open`: parts `devis.pdf` and `setup.exe` → `can_open` true and false.
  - `ops_queue_lifecycle`: enqueue → due at now → `op_retry(next = now + 60)` → not due at now, due at now + 60, `attempts == 1` → `op_done` → empty.
  - `set_seen_local_idempotent`: first call true, second false.
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core store::`. Expected: FAIL.
- [ ] **Step 3: Implement** with plain SQL. The `MessageSummary.from` comes from `from_name`/`from_addr`.
- [ ] **Step 4: Run them to verify they pass** — `cargo test -p illy-core store::`. Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(core): stockage des messages, corps, parties et file ops"`

### Task 7: IMAP — connexion, dossiers, fetch, harnais GreenMail

**Files:**
- Create: `crates/illy-core/src/imap/mod.rs`, `conn.rs`, `folders.rs`, `fetch.rs`
- Create: `crates/illy-core/tests/common/mod.rs`, `crates/illy-core/tests/imap_it.rs`

**Interfaces:**
- Produces:
  ```rust
  pub type ImapSession = async_imap::Session<Box<dyn ImapStream>>;   // ImapStream: AsyncRead + AsyncWrite + Unpin + Send + Debug (TLS or plain TcpStream)
  pub struct Caps { pub idle: bool, pub condstore: bool, pub r#move: bool, pub special_use: bool }
  pub async fn connect(cfg: &ServerConfig, user: &str, password: &str) -> Result<(ImapSession, Caps)>;
  pub fn folder_info(path: &str, delimiter: Option<&str>, attributes: &[String]) -> FolderInfo;  // pure
  pub async fn list_folders(s: &mut ImapSession) -> Result<Vec<FolderInfo>>;
  pub struct Selected { pub uidvalidity: u32, pub uidnext: Option<u32>, pub highest_modseq: Option<u64>, pub exists: u32 }
  pub async fn select(s: &mut ImapSession, path: &str, condstore: bool) -> Result<Selected>;
  pub async fn search_all_uids(s: &mut ImapSession) -> Result<Vec<u32>>;
  pub async fn fetch_headers(s: &mut ImapSession, uids: &[u32]) -> Result<Vec<NewHeader>>;
  pub async fn fetch_flags(s: &mut ImapSession, uid_set: &str, changed_since: Option<u64>) -> Result<Vec<(u32, bool, bool)>>;
  pub async fn fetch_part(s: &mut ImapSession, uid: u32, part_path: &str) -> Result<(Vec<u8> /*mime header*/, Vec<u8> /*body*/)>;
  pub async fn store_seen(s: &mut ImapSession, uid: u32, seen: bool) -> Result<()>;
  pub fn uid_set(uids: &[u32]) -> String;   // pure: [1,2,3,7,9,10] → "1:3,7,9:10"
  ```
- Rules for `connect`:
  - `Implicit` uses TLS on connect (`tokio-rustls`, roots from `webpki-roots`). `StartTls` sends STARTTLS before login. `Plain` is allowed only with the `insecure-plain` feature; otherwise it returns `CoreError::InvalidInput("connexion non chiffrée interdite")`.
  - A login `NO` maps to `CoreError::Auth(<server text>)`. DNS, connect and timeout failures map to `Network`. A handshake failure maps to `Tls`.
  - A 30 s timeout applies to connect and to login.
- Rules for `folder_info` (pure):
  - `INBOX` (any case) → Inbox, "Boîte de réception".
  - SPECIAL-USE attributes come first: `\Sent` Sent "Envoyés", `\Drafts` Drafts "Brouillons", `\Trash` Trash "Corbeille", `\Junk` Spam "Spam". `\All`, `\Flagged`, `\Important` and `\Noselect` are hidden.
  - Otherwise the role comes from the last path segment, case-insensitive:
    - Sent: `Sent`, `Sent Items`, `Sent Messages`, `Envoyés`, `Éléments envoyés`, `Messages envoyés` ;
    - Drafts: `Drafts`, `Brouillons` ;
    - Trash: `Trash`, `Deleted Items`, `Deleted Messages`, `Corbeille`, `Éléments supprimés` ;
    - Spam: `Spam`, `Junk`, `Bulk`, `Bulk Mail`, `Courrier indésirable`, `Pourriels`.
  - `[Gmail]` and `[Google Mail]` themselves are hidden.
  - Custom folders take the last segment, decoded from modified UTF-7 with `utf7-imap`.
- `fetch_headers` requests `(UID FLAGS INTERNALDATE RFC822.SIZE BODYSTRUCTURE BODY.PEEK[HEADER.FIELDS (FROM TO CC SUBJECT DATE MESSAGE-ID IN-REPLY-TO REFERENCES)])`.
- `fetch_part` requests `BODY.PEEK[<p>.MIME] BODY.PEEK[<p>]`. For a single-part message (`p == "1"` with no multipart), it requests `BODY.PEEK[HEADER]` instead of `.MIME`.
- Harness (`tests/common/mod.rs`):
  - `GreenMail::start() -> GreenMail` launches `java -Dgreenmail.setup.test.smtp -Dgreenmail.setup.test.imap -Dgreenmail.smtp.port=<free> -Dgreenmail.imap.port=<free> -Dgreenmail.hostname=127.0.0.1 -Dgreenmail.users=alice:secret@localhost -jar target/greenmail/greenmail-standalone.jar`, waits until the IMAP port accepts connections (timeout 20 s), and kills the process on `Drop`;
  - `gm.server(Protocol) -> ServerConfig` returns `Plain` on the chosen ports;
  - `append(&gm, folder, eml_bytes, seen: bool)` uses a separate IMAP session (`CREATE` the folder if needed).
  - The integration test files carry `#![cfg(feature = "insecure-plain")]`.

- [ ] **Step 1: Write the failing tests**
  - Unit tests in `folders.rs`:
    - `inbox_any_case`;
    - `special_use_wins`: `("[Gmail]/Messages envoyés", ["\\Sent"])` → Sent, "Envoyés";
    - `gmail_all_hidden`: `["\\All"]` → hidden;
    - `name_fallback_ovh`: `"INBOX.Sent"` with delimiter `.` → Sent;
    - `yahoo_bulk_is_spam`;
    - `utf7_custom`: `"R&AOk-ception"` → Custom, "Réception";
    - `gmail_container_hidden`: `"[Gmail]"` with `\Noselect` → hidden.
  - Unit test `uid_set_compresses` in `fetch.rs`: `[1,2,3,7,9,10]` → `"1:3,7,9:10"`, `[]` → `""`.
  - Integration tests in `tests/imap_it.rs`:
    - `login_ok_and_caps`: connects, and `Caps` is returned. Print the caps (`eprintln!`), so the implementer learns whether GreenMail supports IDLE.
    - `login_wrong_password_is_auth`: `connect(.., "alice", "wrong")` returns `Err(CoreError::Auth(_))`.
    - `unreachable_is_network`: port 1 → `Err(CoreError::Network(_))`.
    - `plain_refused_without_feature`: a unit test in `conn.rs` under `#[cfg(not(feature = "insecure-plain"))]`.
    - `headers_and_part_roundtrip`: append `inline_cid_and_pdf.eml`, then `select` + `search_all_uids` → 1 uid. `fetch_headers` gives the expected subject, `seen == false` and 3 parts. `fetch_part(uid, "1.1")`, decoded with `decode_part`, gives HTML containing `cid:logo@x`. Fetching the part does not set `\Seen`: a re-fetch of the flags still shows unseen.
    - `store_seen_sets_flag`: after `store_seen(uid, true)`, `fetch_flags` shows seen.
- [ ] **Step 2: Run them to verify they fail** — `scripts/fetch-greenmail.sh && cargo test -p illy-core --features insecure-plain`. Expected: FAIL.
- [ ] **Step 3: Implement** the functions above with `async-imap` (`runtime-tokio`). Convert each fetch result into `NewHeader` using `parse_header_block` and `flatten_structure`.
- [ ] **Step 4: Run them to verify they pass** — same command. Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(core): connexion IMAP, dossiers, fetch, harnais GreenMail"`

### Task 8: Synchro d'un dossier (initiale hybride + incrémentale)

**Files:**
- Create: `crates/illy-core/src/sync/mod.rs`, `diff.rs`, `folder_sync.rs`
- Test: unit tests in `diff.rs`, integration tests in `crates/illy-core/tests/sync_it.rs`

**Interfaces:**
- Consumes: Store (Tasks 3 and 6), IMAP (Task 7).
- Produces:
  ```rust
  // diff.rs (pure)
  pub struct UidDiff { pub new_desc: Vec<u32>, pub vanished: Vec<u32> }
  pub fn diff_uids(server: &[u32], known: &[u32]) -> UidDiff;            // new_desc sorted descending
  pub fn uidvalidity_changed(stored: Option<u32>, server: u32) -> bool;   // None → false
  // folder_sync.rs
  pub struct FolderBacklog { pub folder_id: FolderId, pub pending_desc: Vec<u32> }
  pub async fn sync_folder(s: &mut ImapSession, caps: &Caps, store: &Store, events: &dyn EventSink,
                           account: AccountId, folder: FolderId, path: &str, cfg: &WorkerConfig) -> Result<FolderBacklog>;
  pub async fn backfill_step(s: &mut ImapSession, store: &Store, events: &dyn EventSink, account: AccountId,
                             path: &str, backlog: &mut FolderBacklog, cfg: &WorkerConfig) -> Result<()>; // fetch next header_batch uids
  pub struct WorkerConfig { /* fields and defaults from Global Constraints */ }
  ```
- `sync_folder` algorithm:
  1. `select(path, caps.condstore)`.
  2. If `uidvalidity_changed`: `clear_folder` and reset the state.
  3. `search_all_uids`, then `diff_uids` with `known_uids`.
  4. `delete_uids(vanished)`.
  5. Fetch the **first** `header_batch` of `new_desc` and upsert it; the rest goes into the returned backlog.
  6. Flags:
     - if `condstore` and both `highest_modseq` values are known: `fetch_flags("1:*", Some(stored))`;
     - otherwise `fetch_flags` on the newest `flags_recent_window` known uids, plus every known uid when `last_sync_at` is more than 1 h old.
     - Then `apply_flags`.
  7. Save `FolderSyncState` (uidvalidity, uidnext, highest_modseq, last_sync_at = now).
  8. Emit `FolderChanged` if anything changed, and `SyncProgress` after each batch.

- [ ] **Step 1: Write the failing tests**
  - Unit tests:
    - `diff_new_and_vanished`: server `[1,2,5,6]`, known `[1,2,3]` → new_desc `[6,5]`, vanished `[3]`.
    - `diff_empty`.
    - `uidvalidity_none_is_not_change`.
  - Integration tests (GreenMail, Store in memory, `RecordingSink`):
    - `initial_sync_newest_first_with_backlog`: append 7 mails; `cfg.header_batch = 3` → after `sync_folder`, the store holds the 3 newest uids, the backlog holds 4 uids in descending order, and a `FolderChanged` event was emitted. Two `backfill_step` calls → 7 messages.
    - `incremental_new_and_deleted`: sync, append 1, delete 1 on the server (STORE `\Deleted` + EXPUNGE through the helper session), sync again → 1 added, 1 removed locally.
    - `flags_from_elsewhere`: sync, mark seen through the helper session, sync again → `unread_count` drops by 1.
    - `uidvalidity_reset`: sync, then `set_folder_state` with a fake uidvalidity of 1, then sync again → no duplicates, and the count equals the server count.
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core --features insecure-plain sync`. Expected: FAIL.
- [ ] **Step 3: Implement** the functions above.
- [ ] **Step 4: Run them to verify they pass** — same command. Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(core): synchro d'un dossier, initiale et incrémentale"`

### Task 9: Corps des mails et pièces jointes

**Files:**
- Create: `crates/illy-core/src/sync/body.rs`
- Test: integration tests appended to `crates/illy-core/tests/sync_it.rs`

**Interfaces:**
- Produces:
  ```rust
  pub async fn fetch_body(s: &mut ImapSession, store: &Store, events: &dyn EventSink, message: MessageId) -> Result<()>;
  pub async fn fetch_attachment(s: &mut ImapSession, store: &Store, message: MessageId, part_path: &str) -> Result<Vec<u8>>; // cached in parts.data
  ```
- `fetch_body` algorithm:
  1. `message_location`, then `select` the folder if needed.
  2. For each `body_parts_to_fetch`: `fetch_part` + `decode_part`.
  3. HTML = concatenation of the `BodyHtml` parts. Text = concatenation of the `BodyText` parts.
  4. Inline map = content_id → (mime, bytes) from the `Inline` parts.
  5. If HTML is present, `sanitize_html`; otherwise text only.
  6. Snippet = `snippet_from(text)`, or `html_to_text(html)` when there is no text.
  7. If decoding fails, or neither HTML nor text exists, store the text `"(contenu indisponible)"` with `degraded = true`.
  8. `store_body`, then emit `MessageUpdated`.
  9. If the message no longer exists on the server (empty FETCH), delete it locally and return `CoreError::NotFound`.

- [ ] **Step 1: Write the failing tests**
  - `body_html_with_cid_is_inlined`: append `inline_cid_and_pdf.eml`, sync, `fetch_body` → `message_detail(.., false)` gives `Html` containing `data:image/png;base64,`, and attachments `[devis.pdf]` with `can_open = true`.
  - `body_plain_windows1252`: `plain_windows1252.eml` → `Text` containing `"café"`.
  - `broken_mime_is_degraded`: `broken_mime.eml` → `Some(detail)` with `degraded == true` and no panic.
  - `fetch_body_does_not_mark_seen`: after `fetch_body`, the server flags still show unseen.
  - `attachment_bytes_and_cache`: `fetch_attachment(.., "2")` returns the PDF bytes from the fixture. A second call is answered from `parts.data`: close the session, then check that the result still comes back by reading the store directly through `Store::part`.
  - `vanished_message_not_found`: sync, delete the mail on the server, `fetch_body` → `Err(NotFound)`, and the message is gone locally.
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core --features insecure-plain body`. Expected: FAIL.
- [ ] **Step 3: Implement** the functions above.
- [ ] **Step 4: Run them to verify they pass** — same command. Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(core): téléchargement des corps et pièces jointes"`

### Task 10: Worker de compte, file ops, IDLE et SyncManager

**Files:**
- Create: `crates/illy-core/src/sync/ops_exec.rs`, `worker.rs`, `manager.rs`
- Test: integration tests in `crates/illy-core/tests/worker_it.rs`

**Interfaces:**
- Consumes: Tasks 6–9, `SecretStore`.
- Produces:
  ```rust
  pub enum WorkerRequest {
      SyncFolder(FolderId),
      FetchBody(MessageId, oneshot::Sender<Result<()>>),
      FetchAttachment(MessageId, String, oneshot::Sender<Result<Vec<u8>>>),
      OpsAvailable,
  }
  pub struct SyncManager { /* store, secrets, events, cfg, workers: Mutex<HashMap<AccountId, WorkerHandle>> */ }
  impl SyncManager {
      pub fn new(store: Store, secrets: Arc<dyn SecretStore>, events: Arc<dyn EventSink>, cfg: WorkerConfig) -> Self;
      pub fn start(&self, account: AccountId);          // no-op if already running
      pub fn stop(&self, account: AccountId);
      pub fn restart(&self, account: AccountId);
      pub async fn send(&self, account: AccountId, req: WorkerRequest) -> Result<()>; // NotFound if no worker
  }
  pub async fn run_due_ops(s: &mut ImapSession, store: &Store, events: &dyn EventSink, account: AccountId, cfg: &WorkerConfig) -> Result<()>; // ops_exec.rs
  ```
- Worker behavior (`worker.rs`, one tokio task per account):
  1. Read the password from `SecretStore`. Missing → status `AuthError` "Mot de passe manquant", then wait for a restart.
  2. Connect with status `Syncing`.
     - `Auth` → status `AuthError` with the server text, emit `AccountStatus`, and **stop retrying** until `restart`.
     - `Network` or `Tls` → status `Offline`, then retry with exponential backoff between `backoff_min` and `backoff_max`.
  3. Run `list_folders` → `upsert_folders` → emit `FoldersChanged`. Then `sync_folder` on the inbox first, then on the other folders. Status `Ok`.
  4. If `caps.idle`, spawn a second connection that SELECTs INBOX and IDLEs, renewed every `idle_renew`. Any untagged response sends an internal `InboxChanged`. If the IDLE connection drops, reconnect it with the same backoff. Without IDLE, a timer fires `InboxChanged` every `poll_interval`.
  5. Main loop, in strict priority order:
     1. Requests from the channel, handled first (`FetchBody`, `FetchAttachment`, `SyncFolder`).
     2. Due ops (`run_due_ops`).
     3. `InboxChanged` → sync the inbox.
     4. The `other_folders_interval` timer → sync the other folders.
     5. One `backfill_step` across folder backlogs (inbox first).
     6. One batch of up to 20 bodies from `recent_without_body(now - recent_body_days)`, then from `any_without_body`.
     7. Otherwise, wait on the channel, the IDLE notification and the timers.
  6. A `Network` error mid-loop → status `Offline`, drop the sessions, back off and reconnect from step 2. Pending `FetchBody` requests receive `Err(Network)`.
- `run_due_ops` rules:
  - `SetSeen` payload: `{ "folderId": i64, "uid": u32, "seen": bool }` → `select` + `store_seen` → `op_done`.
  - `Network` → `op_retry` (backoff by attempts) and return the error.
  - Server `NO`/`BAD` → `op_done`, revert the local seen flag, and emit `MessageUpdated` (generic revert for later milestones).

- [ ] **Step 1: Write the failing tests** (GreenMail; `cfg.poll_interval = 1 s`, other intervals 2 s)
  - `worker_syncs_and_emits`: add an account to the store, put the password in `MemorySecretStore`, `start` → within 10 s, events contain `FoldersChanged` and `FolderChanged`, and the store holds the appended mails.
  - `new_mail_arrives`: after the first sync, append 1 mail → within 10 s, a `FolderChanged` arrives for the inbox and the count goes up by 1 (exercises IDLE or polling, depending on GreenMail's caps).
  - `auth_error_stops_retrying`: wrong password → status `AuthError`, exactly one `AccountStatus(AuthError)` event in 5 s (no retry loop). After `secrets.set(correct)` + `restart` → status `Ok`.
  - `fetch_body_request_has_priority`: 2000 appended mails, `header_batch = 100` (big backlog). Immediately request `FetchBody` for the newest message → the answer arrives within 5 s.
  - `set_seen_op_reaches_server`: enqueue `SetSeen` + send `OpsAvailable` → the server flag is set within 5 s, and the ops table is empty.
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core --features insecure-plain worker`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above (`tokio::select!` over the mpsc receiver, the IDLE notify channel and `tokio::time::Interval`s).
- [ ] **Step 4: Run them to verify they pass** — same command. Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(core): worker de compte, IDLE, file ops, SyncManager"`

### Task 11: Façade `Core` (comptes, listes, ouverture, pièces jointes)

**Files:**
- Create: `crates/illy-core/src/core.rs`, `crates/illy-core/src/smtp.rs`
- Modify: `crates/illy-core/src/lib.rs` (re-export `Core`)
- Test: `crates/illy-core/tests/core_it.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct Core { /* store, secrets, events, manager, tmp_dir */ }
  impl Core {
      pub fn open(db_path: &Path, tmp_dir: &Path, secrets: Arc<dyn SecretStore>, events: Arc<dyn EventSink>, cfg: WorkerConfig) -> Result<Core>; // empties tmp_dir, starts a worker per account
      pub fn detect_provider(&self, email: &str) -> ProviderGuess;
      pub async fn add_account(&self, input: NewAccount, password: String) -> Result<Account>; // validates, tests IMAP + SMTP, inserts, stores the secret, starts the worker
      pub async fn update_password(&self, account: AccountId, password: String) -> Result<()>; // tests IMAP, stores, restart
      pub fn remove_account(&self, account: AccountId) -> Result<()>;                           // stop worker, delete secret + data
      pub fn list_accounts(&self) -> Result<Vec<Account>>;
      pub fn list_folders(&self, account: AccountId) -> Result<Vec<Folder>>;
      pub fn list_messages(&self, scope: ListScope, offset: u32, limit: u32) -> Result<Vec<MessageSummary>>;
      pub fn count_messages(&self, scope: ListScope) -> Result<u32>;
      pub async fn select_folder(&self, folder: FolderId) -> Result<()>;          // asks the worker for an immediate SyncFolder
      pub async fn open_message(&self, id: MessageId, show_images: bool) -> Result<MessageDetail>;
      pub fn allow_images_from(&self, sender: &str) -> Result<()>;
      pub async fn attachment_to_tmp(&self, id: MessageId, part_path: &str) -> Result<PathBuf>; // refuses when can_open == false
      pub async fn save_attachment(&self, id: MessageId, part_path: &str, dest: &Path) -> Result<()>;
  }
  pub async fn test_smtp(cfg: &ServerConfig, user: &str, password: &str) -> Result<()>;   // smtp.rs, lettre; auth failure → Auth
  ```
- Rules:
  - `add_account` validation: email contains `@`; display_name is not empty (otherwise it defaults to the part before `@`); `Plain` is refused without `insecure-plain`. The IMAP and SMTP login is the full email address. On `Auth`, `Network` or `Tls`, the error is returned unchanged and **nothing is saved**.
  - `open_message` algorithm:
    1. If `message_detail` is `None`, send `FetchBody` and await it (timeout 30 s → `Network`).
    2. If `set_seen_local(id, true)` returns true: `enqueue_op(SetSeen)`, send `OpsAvailable`, and emit `FolderChanged` (the unread count changed).
    3. Return the detail.
  - `attachment_to_tmp` writes `tmp_dir/<message id>-<sanitized filename>`. The filename is sanitized by stripping path separators and `..`.

- [ ] **Step 1: Write the failing tests** (GreenMail)
  - `add_account_ok`: returns an `Account`, `secrets.get(id)` is `Some`, and the worker syncs (the inbox appears within 10 s).
  - `add_account_wrong_password_saves_nothing`: `Err(Auth)`, `list_accounts` is empty, and the secret is absent.
  - `add_account_rejects_bad_email`: `"pasunemail"` → `Err(InvalidInput)`.
  - `open_message_marks_seen_everywhere`: after sync, `open_message(newest)` → detail returned, the local summary is `seen`, and the server flag is set within 5 s.
  - `open_message_offline_then_recovers`: sync, fetch the newest mail's body, then `stop` the worker (stands in for the outage: GreenMail cannot be restarted with its mailbox intact). `open_message(newest)` → the local summary is `seen`, one `SetSeen` op is queued, and the server flag is still unset. Then `start` the worker → the server flag is set within 5 s and the ops table is empty.
  - `remove_account_deletes_data_and_secret`: after `remove_account`, `list_accounts` is empty, `secrets.get(id)` is `None`, and `list_messages(UnifiedInbox, 0, 10)` is empty.
  - `attachment_exe_refused`: an attachment named `setup.exe` → `attachment_to_tmp` returns `Err(InvalidInput)`.
  - `attachment_to_tmp_writes_file`: PDF → the file exists in tmp with the right bytes; `../../evil.pdf` is written inside `tmp_dir`.
  - `open_cleans_tmp`: a file left in `tmp_dir` before `Core::open` is gone afterwards.
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core --features insecure-plain core_it`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Run them to verify they pass** — `cargo test -p illy-core --features insecure-plain`. Expected: the whole crate suite PASSes.
- [ ] **Step 5: Commit** — `git commit -m "feat(core): façade Core (comptes, listes, ouverture, pièces jointes)"`

### Task 12: Interface — jetons, coque 3 panneaux, couche `api` (Tauri + simulée), état

**Files:**
- Create: `src/api/types.ts`, `src/api/index.ts`, `src/api/tauri.ts`, `src/api/mock.ts`, `src/state/store.ts`, `src/ui/tokens.css`, `src/ui/global.css`, `src/ui/TitleBar.tsx`, `src/ui/TitleBar.module.css`, `src/ui/icons.tsx`, `src/App.module.css`
- Modify: `src/App.tsx`, `src/main.tsx`
- Test: `src/state/store.test.ts`, `src/App.test.tsx`, `src/ui/TitleBar.test.tsx`

**Interfaces:**
- Produces (`types.ts`): exact TS mirrors of Task 2's `model.rs` and `events.rs`, in camelCase, with these helper types:
  - `type ListScope = { kind: 'folder'; folderId: number } | { kind: 'unified_inbox' }`;
  - `type MessageBody = { kind: 'html' | 'text'; content: string }`;
  - `type ApiError = { kind: string; message: string }`.
- Produces (`api/index.ts`):
  ```ts
  export interface Api {
    detectProvider(email: string): Promise<ProviderGuess>;
    addAccount(input: NewAccount, password: string): Promise<Account>;
    updatePassword(accountId: number, password: string): Promise<void>;
    listAccounts(): Promise<Account[]>;
    listFolders(accountId: number): Promise<Folder[]>;
    listMessages(scope: ListScope, offset: number, limit: number): Promise<MessageSummary[]>;
    countMessages(scope: ListScope): Promise<number>;
    selectFolder(folderId: number): Promise<void>;
    openMessage(id: number, showImages: boolean): Promise<MessageDetail>;
    allowImagesFrom(sender: string): Promise<void>;
    openAttachment(id: number, partPath: string): Promise<void>;
    saveAttachment(id: number, partPath: string): Promise<void>;
    openUrl(url: string): Promise<void>;
    onEvent(cb: (e: CoreEvent) => void): () => void;   // returns unsubscribe
  }
  export const api: Api;   // tauri.ts when '__TAURI_INTERNALS__' in window, else mock.ts
  ```
  Tauri command names are the snake_case of the method names (`list_messages`, …). The event channel is `"core-event"`.
- Produces (`state/store.ts`, Zustand):
  ```ts
  type Selection = { scope: ListScope; messageId: number | null };
  interface AppState {
    accounts: Account[]; folders: Record<number, Folder[]>; selection: Selection; showWizard: boolean;
    loadAccounts(): Promise<void>; loadFolders(accountId: number): Promise<void>;
    selectScope(scope: ListScope): void;            // also calls api.selectFolder for folder scopes
    selectMessage(id: number | null): void;
    applyEvent(e: CoreEvent): void;                 // account_status → patch account; folders_changed / folder_changed → loadFolders
  }
  ```
  On startup: `loadAccounts`, then `loadFolders` for each account. Initial selection = the first account's inbox, or `unified_inbox` when there is no folder yet. `showWizard = accounts.length === 0`.
- `mock.ts`: 3 fictional accounts (`moi@gmail.com`, `moi@yahoo.fr` with status `auth_error`, `contact@mondomaine.fr`), their folders, 300 generated messages, and one HTML detail containing a blocked remote image, an inline image and a PDF attachment. Everything resolves after 50 ms.
- Layout (`App.tsx` + `App.module.css`):
  - `TitleBar`: drag region `data-tauri-drag-region`, "Illy", and minimize / maximize / close buttons through `getCurrentWindow()` (no-ops in mock mode). When there is at least one account and every account has status `offline`, it shows a discreet grey pill "Hors ligne" (spec § 10).
  - Below it, a CSS grid of 3 panels: `260px 380px 1fr`, gap 12 px, padding 12 px, each panel `#16191F` with radius 20 px.
  - The window has the 1 px accent outline.
  - Panel widths are not resizable in J1 (deferred to J5 finitions).

- [ ] **Step 1: Write the failing tests**
  - `store.test.ts`:
    - `applyEvent account_status patches account`: after the event, `accounts[i].status === 'auth_error'` and `lastError` is set.
    - `selectScope folder calls api.selectFolder`: with `api` mocked by `vi.mock`.
    - `startup selects first inbox`: with mock data, `selection.scope` equals the inbox folder of account 1.
  - `App.test.tsx`: `renders three panels and title`: `getByText('Illy')`, plus `getByTestId('panel-sidebar')`, `panel-list` and `panel-reader`.
  - `TitleBar` test `offline pill when all accounts offline`: with every account `offline` → `getByText('Hors ligne')`; with one account `ok` → no such text.
- [ ] **Step 2: Run them to verify they fail** — `npm test`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above. `tokens.css` defines every token from Global Constraints as CSS variables (`--bg`, `--panel`, `--raised`, `--border`, `--text`, `--text-2`, `--accent`, `--accent-glow`, `--warn`, `--error`, `--r-panel`, `--r-row`, `--r-pill`). `global.css` imports `@fontsource/plus-jakarta-sans/400.css`, `500.css`, `600.css` and `700.css`.
- [ ] **Step 4: Run them to verify they pass** — `npm run typecheck && npm test`. Expected: PASS. Also run `npm run dev` and check the 3 panels in a browser with the mock data.
- [ ] **Step 5: Commit** — `git commit -m "feat(ui): coque 3 panneaux, jetons de design, couche api et état"`

### Task 13: App Tauri — commandes, événements, fenêtre, icônes, build Windows en CI

**Files:**
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/icons/app-icon.svg` (+ generated icons), `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs`
- Modify: root `Cargo.toml` (add `src-tauri` to members), `.github/workflows/ci.yml` (enable `build-windows`)

**Interfaces:**
- Consumes: `Core` (Task 11), `Api` command names (Task 12).
- Produces:
  - One `#[tauri::command]` per `Api` method except `onEvent`. Each is `async`, takes `State<'_, Core>` and returns `Result<T, CoreError>`.
  - `open_attachment` calls `core.attachment_to_tmp`, then `tauri_plugin_opener::open_path`.
  - `save_attachment` opens the dialog plugin's save dialog with the default filename, then calls `core.save_attachment`. Cancelling is `Ok(())`.
  - `open_url` validates that the scheme is `http` or `https`, then calls `opener::open_url`.
  - `TauriSink(AppHandle)` implements `EventSink` by calling `app.emit("core-event", event)`.
- Startup in `lib.rs`:
  1. `data = directories::BaseDirs::new().data_local_dir().join("Illy")`; create `logs/` and `tmp/`.
  2. `tracing_subscriber` with a daily rolling file in `logs/` at level `info`.
  3. `Core::open(data/illy.db, data/tmp, KeyringSecretStore, TauriSink, WorkerConfig::default())`.
  4. `app.manage(core)`, then register the plugins (`opener`, `dialog`).
- `tauri.conf.json`:
  - `productName` "Illy", `identifier` `fr.illy.app`;
  - one window `main`: 1280×800, min 1000×640, `decorations: false`, title "Illy";
  - `bundle.targets` `["msi", "nsis"]`, `bundle.windows.webviewInstallMode` `downloadBootstrapper`;
  - CSP: `default-src 'self'; img-src 'self' data: https: http:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; frame-src 'self' about: data:; connect-src ipc: http://ipc.localhost`.

  The srcdoc iframe inherits this policy. The stricter per-mail CSP from Task 15 applies on top of it.
- `capabilities/default.json`: `core:default`, `core:window:allow-minimize`, `core:window:allow-toggle-maximize`, `core:window:allow-close`, `core:window:allow-start-dragging`, `opener:default`, `dialog:default`.
- Icon: simple SVG (a rounded square `#0D0F13` with a glowing `#2F7BFF` envelope outline). Generate the icons with `npx tauri icon src-tauri/icons/app-icon.svg`.

- [ ] **Step 1: Write the failing check.** `src-tauri/src/commands.rs` unit test `open_url_rejects_non_http`: the validation helper `fn check_url(url: &str) -> Result<(), CoreError>` rejects `file:///C:/x`, `javascript:x` and `mailto:a@b`, and accepts `https://ex.fr`.
- [ ] **Step 2: Run it to verify it fails** — first install the Linux build deps: `sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev librsvg2-dev libxdo-dev libssl-dev`. Then run `cargo test -p illy-app`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Verify**
  - `cargo test -p illy-app` and `cargo clippy --workspace -- -D warnings`: PASS.
  - Push the branch: the CI job `build-windows` is green and the `illy-windows` artifact contains a `.msi` and a `.exe`.
- [ ] **Step 5: Commit** — `git commit -m "feat(app): application Tauri, commandes, événements, build Windows"`

### Task 14: Panneau de gauche et liste des mails

**Files:**
- Create: `src/features/sidebar/Sidebar.tsx`, `Sidebar.module.css`, `Sidebar.test.tsx`, `src/features/message-list/MessageList.tsx`, `MessageRow.tsx`, `SearchBar.tsx`, `MessageList.module.css`, `MessageList.test.tsx`
- Modify: `src/App.tsx` (mount them)

**Interfaces:**
- Consumes: `useAppStore` and `api` (Task 12).
- Produces:
  - `<Sidebar/>` and `<MessageList/>`, with no props (both read the store).
  - `formatListDate(ts: number, now: Date): string`: `HH:MM` today, `hier`, the weekday name (`lundi`…) within the last 7 days, `JJ/MM` this year, `JJ/MM/AAAA` before that.
- Sidebar:
  - Top entry "Toutes les boîtes de réception", with a pill showing the sum of the inbox unread counts.
  - Then each account: a header showing its color dot and email, collapsible (state persisted in `localStorage` under `illy.collapsed`, read and written inside try/catch).
  - Then its folders: role names from the core, and a blue pill when `unreadCount > 0`. The active entry gets the raised background and the glowing accent.
  - An account with status `auth_error` shows an orange dot `#F5A524`; clicking it opens a small inline card "Mot de passe refusé — Mettre à jour". `offline` shows a grey dot titled "Hors ligne".
  - Footer: the button "Nouveau mail" (disabled in J1, tooltip "Disponible bientôt") and a gear button that opens the wizard (J1: "Ajouter un compte").
- MessageList:
  - `SearchBar` is shown but disabled in J1 (placeholder "Rechercher…", tooltip "Disponible bientôt"), with the "Chercher partout" button and the chips "Non lus", "Pièce jointe", "Dates", all disabled.
  - The list is virtualized with `useVirtualizer`, 76 px rows, using `countMessages`. Pages of 100 load through `listMessages` and are cached by page index; the cache is invalidated on `folder_changed`/`message_updated` for the current scope.
  - Row content: sender (name, else email), date, subject (or "(sans objet)"), snippet, a blue dot plus semibold text when `!seen`, a paperclip when `hasAttachments`, and the account color dot in the unified view.
  - Selected row: `#1F232B` background and a 3 px accent bar on the left.
  - Clicking a row calls `selectMessage`. Keyboard ↑/↓ moves the selection (minimal accessibility; the full shortcuts come later).
  - Empty state: "Aucun mail dans cette boîte". While the first page loads: 8 skeleton rows.

- [ ] **Step 1: Write the failing tests**
  - `Sidebar.test.tsx`:
    - `shows accounts folders and unread pills`: with mock data, `getByText('moi@gmail.com')`, `getByText('Boîte de réception')` and the pill `'3'`.
    - `unified inbox sums unread`.
    - `click folder selects scope`.
    - `auth_error shows warning card`: click the orange dot → `getByText('Mot de passe refusé')`.
  - `MessageList.test.tsx`:
    - `formatListDate` cases: today 14:32 → `'14:32'`; yesterday → `'hier'`; 3 days ago → its weekday name; 2023-05-04 → `'04/05/2023'`.
    - `renders rows from api`: the first sender is visible.
    - `unread row has dot`: `getAllByTestId('unread-dot')`.
    - `click selects message`.
    - `empty state`: `countMessages` = 0 → `'Aucun mail dans cette boîte'`.

  In jsdom, mock `getBoundingClientRect`/`offsetHeight` for the virtualizer, or render it with `initialRect`.
- [ ] **Step 2: Run them to verify they fail** — `npm test`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Run them to verify they pass** — `npm run typecheck && npm test`. Expected: PASS. Visual check with `npm run dev`: the result matches the prototypes (variant chosen by the owner).
- [ ] **Step 5: Commit** — `git commit -m "feat(ui): panneau des boîtes et liste virtualisée des mails"`

### Task 15: Lecteur — iframe isolée, images, liens, pièces jointes

**Files:**
- Create: `src/features/reader/Reader.tsx`, `MailFrame.tsx`, `ImagesBanner.tsx`, `Attachments.tsx`, `Reader.module.css`, `Reader.test.tsx`, `MailFrame.test.tsx`
- Modify: `src/App.tsx`

**Interfaces:**
- Consumes: `api.openMessage`, `allowImagesFrom`, `openAttachment`, `saveAttachment`, `openUrl`; `selection.messageId`.
- Produces:
  - `buildSrcDoc(body: MessageBody, allowRemoteImages: boolean): string` (exported, pure);
  - `<MailFrame body allowRemoteImages />`;
  - `<Reader/>`.
- `buildSrcDoc` rules:
  - It produces `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:{X}; font-src data:"><style>…base…</style></head><body>…</body></html>`.
  - `{X}` is ` https: http:` when `allowRemoteImages`, and empty otherwise.
  - Base style: `body{margin:0;font:15px/1.5 system-ui, sans-serif;color:#111;background:#fff;word-wrap:break-word} img{max-width:100%;height:auto}`.
  - A `text` body is already HTML from the core (`text_to_html`) and is inserted unchanged.
- MailFrame:
  - `<iframe sandbox="allow-same-origin" srcDoc=…>` inside the white card (radius 16, padding 24).
  - On `load`: the height follows `contentDocument.documentElement.scrollHeight` (a `ResizeObserver` on the iframe body keeps it updated).
  - A `click` listener on `contentDocument` finds the closest `a[href]`, calls `preventDefault`, and calls `api.openUrl(href)` when it starts with `http`.
  - `mouseover` on a link shows its real URL in a status line under the card.
- Reader:
  - When `selection.messageId` changes: `openMessage(id, false)`, with a loading state (skeleton) and errors displayed as "Impossible d'ouvrir ce mail : {message}".
  - Header: sender name + `<email>`, "à {destinataires}", date as `jeudi 12 novembre 2026, 14:32` (`Intl.DateTimeFormat('fr-FR', { dateStyle: 'full', timeStyle: 'short' })`).
  - Action bar: icon buttons Répondre, Répondre à tous, Transférer, Marquer non lu, Déplacer, Spam, Supprimer, **all visible but disabled in J1**, each with a tooltip "Disponible bientôt".
  - If `imagesBlocked`: `ImagesBanner` shows "Images bloquées pour protéger ta vie privée · Afficher les images · Toujours pour cet expéditeur". The first action re-calls `openMessage(id, true)`; the second calls `allowImagesFrom(from.email)` then `openMessage(id, false)`.
  - If `degraded`: a note "Ce mail n'a pas pu être affiché correctement".
  - Attachments: pills showing name and size (Ko/Mo, French formatting). Click → `openAttachment` when `canOpen`. A "⋯" menu offers "Enregistrer sous…" → `saveAttachment`. When `!canOpen`, the pill shows a lock and clicking it triggers "Enregistrer sous…" directly.
  - Empty state when nothing is selected: "Sélectionne un mail pour l'afficher".

- [ ] **Step 1: Write the failing tests**
  - `MailFrame.test.tsx`:
    - `csp blocks remote images by default`: `buildSrcDoc({kind:'html',content:'<p>x</p>'}, false)` contains `img-src data:;` and does not contain `https:`.
    - `csp allows remote when asked`: with `true`, it contains `img-src data: https: http:`.
    - `iframe is sandboxed without scripts`: the rendered iframe has `sandbox === 'allow-same-origin'`.
  - `Reader.test.tsx` (api mocked):
    - `shows sender subject and date`;
    - `images banner shows and show-images reopens with true`: `openMessage` is called with `(id, true)`;
    - `always-for-sender calls allowImagesFrom`;
    - `dangerous attachment cannot be opened`: `canOpen: false` → clicking calls `saveAttachment`, not `openAttachment`;
    - `degraded note`;
    - `error message on failure`: rejected `{kind:'network', message:'serveur injoignable'}` → the text contains `Impossible d'ouvrir ce mail`.
- [ ] **Step 2: Run them to verify they fail** — `npm test`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Run them to verify they pass** — `npm run typecheck && npm test`. Expected: PASS. Visual check with `npm run dev`: the mock HTML mail appears on the white card, with the banner and the attachments.
- [ ] **Step 5: Commit** — `git commit -m "feat(ui): lecteur de mails isolé, images bloquées, pièces jointes"`

### Task 16: Assistant d'ajout de compte et mise à jour du mot de passe

**Files:**
- Create: `src/features/accounts/AddAccountWizard.tsx`, `UpdatePasswordDialog.tsx`, `Accounts.module.css`, `AddAccountWizard.test.tsx`, `UpdatePasswordDialog.test.tsx`
- Modify: `src/App.tsx` (wizard shown when `showWizard`), `src/features/sidebar/Sidebar.tsx` (the "Mettre à jour" button opens `UpdatePasswordDialog`)

**Interfaces:**
- Consumes: `api.detectProvider`, `addAccount`, `updatePassword`, `openUrl`; the store's `loadAccounts` and `loadFolders`.
- Produces: `<AddAccountWizard onClose/>` and `<UpdatePasswordDialog accountId onClose/>`.
- Wizard flow (modal over the dimmed window; step indicator "1 Adresse · 2 Mot de passe · 3 Vérification"):
  1. **Adresse**: fields "Ton nom" and "Adresse mail", button "Continuer" → `detectProvider`.
     - If `provider` is null, show the choice "Hébergé chez OVH" / "Autre hébergeur (réglages manuels)".
     - "Autre" reveals IMAP server/port and SMTP server/port, with TLS Implicit by default and a "STARTTLS" option for SMTP port 587.
  2. **Mot de passe**: for Gmail and Yahoo, show exactly these instructions, then the field "Mot de passe d'application". For OVH and manual, show only the field "Mot de passe".

     Gmail:
     > **Gmail demande un mot de passe d'application.**
     > 1. Active la validation en deux étapes sur ton compte Google.
     > 2. Vérifie qu'IMAP est activé dans Gmail (Paramètres › Transfert et POP/IMAP).
     > 3. Crée un mot de passe d'application et colle-le ci-dessous.

     Buttons: "Ouvrir la page des mots de passe d'application" → `openUrl('https://myaccount.google.com/apppasswords')`; "Ouvrir les réglages IMAP de Gmail" → `openUrl('https://mail.google.com/mail/u/0/#settings/fwdandpop')`.

     Yahoo: same title with "Yahoo", steps 1 and 3 identical with "Yahoo" instead of "Google", no IMAP step. Button "Ouvrir la sécurité du compte Yahoo" → `openUrl('https://login.yahoo.com/account/security')`.
  3. **Vérification**: "Tester et ajouter" → `addAccount` with a spinner "Connexion à {serveur IMAP}…".
     - Success → close, `loadAccounts`, select the new account's inbox.
     - Error by `kind`:
       - `auth` → "Mot de passe refusé. Vérifie que tu as collé le mot de passe d'application (et pas ton mot de passe habituel)." followed by the server text in grey;
       - `network` → "Serveur injoignable. Vérifie ta connexion internet et l'adresse du serveur.";
       - `tls` → "Connexion sécurisée impossible avec ce serveur.";
       - any other → "Erreur : {message}".
     - "Retour" goes back to step 2 with the password kept.
- When `accounts.length === 0`, the wizard has no close button. Otherwise "Annuler" closes it.
- UpdatePasswordDialog: title "Mettre à jour le mot de passe de {email}", one field, "Enregistrer" → `updatePassword`. Errors use the same French mapping.

- [ ] **Step 1: Write the failing tests**
  - `AddAccountWizard.test.tsx`:
    - `gmail shows app password instructions`: entering `moi@gmail.com` shows the text "Gmail demande un mot de passe d'application".
    - `unknown domain offers OVH or manual`.
    - `manual reveals server fields`.
    - `auth error message`: `addAccount` rejects with `{kind:'auth', message:'NO [AUTHENTICATIONFAILED]'}` → the French text plus the server text are visible.
    - `success closes and reloads`: `onClose` is called and `listAccounts` is called again.
    - `no close button on first run`.
  - `UpdatePasswordDialog.test.tsx`: `saves and closes`; `shows auth error`.
- [ ] **Step 2: Run them to verify they fail** — `npm test`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Run them to verify they pass** — `npm run typecheck && npm test`. Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(ui): assistant d'ajout de compte et mise à jour du mot de passe"`

### Task 17: Mesures de performance et liste de vérification du jalon 1

**Files:**
- Create: `crates/illy-core/tests/perf_it.rs` (ignored by default), `docs/checklists/jalon-1.md`

**Interfaces:**
- Consumes: `Store` (Tasks 3 and 6).

- [ ] **Step 1: Write the perf test** `#[ignore] perf_100k`:
  - generate 100 000 messages in a temp-file `Store` (2 accounts, 10 folders, bodies for 10 %);
  - assert that `list_messages(Folder(inbox), 0, 100)` and `count_messages` together take < 150 ms (median of 20 runs);
  - assert that `list_messages(UnifiedInbox, 0, 100)` takes < 150 ms;
  - assert that `message_detail` takes < 20 ms.
- [ ] **Step 2: Run it** — `cargo test -p illy-core --release -- --ignored perf_100k --nocapture`. If a threshold fails, add the missing index or query fix in `store/` and rerun until it passes.
- [ ] **Step 3: Write `docs/checklists/jalon-1.md`**, the manual checklist the owner runs on Windows with the CI `.msi`:
  1. Install, then first launch: the wizard opens.
  2. Add Gmail (app password), Yahoo (app password) and OVH: the inbox of each appears in under 10 s.
  3. Gmail shows no "Tous les messages" / "Important" / "Suivis"; libellés appear as folders; accents in folder names are correct.
  4. Send yourself a mail from your phone: it appears within 10 s, without any action.
  5. Open an unread mail: it becomes read in Illy and on the webmail; nothing is archived or moved.
  6. A newsletter shows the "Images bloquées" banner; "Afficher" shows the images; "Toujours pour cet expéditeur" persists after restart.
  7. A link opens in the default browser.
  8. Open a PDF attachment; "Enregistrer sous…" works; an `.exe` cannot be opened directly.
  9. Revoke the Gmail app password: the account shows the orange dot "Mot de passe refusé"; "Mettre à jour" with a new password brings it back.
  10. Turn Wi-Fi off: reading stays possible; open an unread mail; turn Wi-Fi back on: the mail is read on the webmail within 1 minute.
  11. Task Manager: Illy uses < 200 Mo of memory at rest after the initial sync.
  12. Startup with data already synced: inbox visible in < 2 s.
- [ ] **Step 4: Commit** — `git commit -m "test: mesures de performance et checklist du jalon 1"`
