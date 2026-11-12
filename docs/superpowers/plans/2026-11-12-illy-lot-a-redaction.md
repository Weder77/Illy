# Illy — Lot A : rédaction — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Écrire, répondre, répondre à tous et transférer depuis Illy : éditeur riche, destinataires avec autocomplétion, pièces jointes, brouillons locaux et serveur, envoi SMTP avec annulation de 10 s, copie dans Envoyés, clic droit sur un mail. Version 0.2.0.

**Architecture:**
- Le cœur Rust gagne :
  - les brouillons (table `drafts`) et le préremplissage ;
  - la construction MIME (`lettre`) et l'envoi par la file `ops` (op `send` avec une échéance égale au délai d'annulation) ;
  - les ops `append_draft`, `set_answered` et `sent_copy` ;
  - les réglages (table `settings`) et les contacts.
- L'interface gagne un composant `Composer` (TipTap), commun à la réponse sous le mail et à une fenêtre Tauri `compose-<id>`, un bandeau d'envoi et un menu contextuel.

**Tech Stack:** l'existant, plus TipTap 2 (`@tiptap/react`, `@tiptap/pm`, `@tiptap/starter-kit`, `@tiptap/extension-underline`, `@tiptap/extension-link`, `@tiptap/extension-image`, `@tiptap/extension-placeholder`), `uuid` (Rust, `v4`), `lettre` (builder + `send_raw`).

**Spec:** `docs/superpowers/specs/2026-11-12-illy-redaction-parametres-design.md` § 3 (et § 5 Erreurs) ; spec principale `docs/superpowers/specs/2026-11-11-illy-client-mail-design.md` § 8 pour le reste.

## Global Constraints

- Interface en français. Préfixes « Re: » / « Fwd: », non ajoutés si l'objet commence déjà par `re:`, `fwd:`, `fw:` ou `tr:` (sans tenir compte de la casse).
- Délai d'annulation par défaut **10 s**. Valeurs permises 5, 10, 30.
- Les réglages vivent dans la table `settings` : une clé absente vaut sa valeur par défaut (spec § 3.1).
- Le `Bcc` est transmis au serveur SMTP mais n'apparaît **jamais** dans le MIME stocké (Envoyés, Brouillons).
- Images collées dans l'éditeur : 5 Mo au plus chacune. Confirmation au-delà de 25 Mo de pièces jointes, avec le texte exact de la spec § 3.5.
- Aucun envoi automatique sans que l'utilisateur ait cliqué « Envoyer ». Un brouillon n'est jamais envoyé par effet de bord.
- Brouillons : sauvegarde locale 2 s après la dernière frappe ; copie serveur au plus une fois par minute et à la fermeture ; en-tête `X-Illy-Draft: <draft_id>`.
- Pièces jointes ajoutées depuis le disque : copiées dans `<data>/drafts/<draft_id>/`.
- Les adresses de l'utilisateur (tous ses comptes, comparaison sans casse) sont exclues de « Répondre à tous » et du carnet de contacts.
- Toutes les nouvelles commandes Tauri sont `async` ou `#[tauri::command(async)]` (jamais sur le fil de la fenêtre).

## Review Focus

1. **Accents partout** : un nom, un objet et un nom de pièce jointe accentués arrivent intacts chez le destinataire (encodage RFC 2047 / 2231 par `lettre`). Testé à la tâche 7 (aller-retour GreenMail).
2. **Fermer Illy pendant les 10 s d'annulation** : l'envoi part au prochain démarrage, une seule fois. Testé à la tâche 7.
3. **Destinataire refusé par le serveur** (5xx) : état `failed` avec le message du serveur, pas de boucle de nouvelles tentatives. Testé à la tâche 6.
4. **Répondre à tous quand on figure soi-même dans To/Cc avec une autre casse** : on ne s'écrit pas à soi-même. Testé à la tâche 3.
5. **Annulation au moment exact du départ** (course avec le worker) : soit annulé et rien n'est parti, soit parti et `cancel_send` renvoie `false`, jamais les deux. Testé à la tâche 6 (`started_at`).

---

### Task 1: Schéma v2 et réglages

**Files:**
- Modify: `crates/illy-core/src/store/schema.rs`, `crates/illy-core/src/store/mod.rs`, `crates/illy-core/src/model.rs`
- Create: `crates/illy-core/src/store/settings.rs`

**Interfaces:**
- Produces:
  - `schema::V2` with:
    - `CREATE TABLE settings(key TEXT PRIMARY KEY, value_json TEXT NOT NULL)`;
    - `ALTER TABLE messages ADD COLUMN reply_to_json TEXT NOT NULL DEFAULT '[]'`;
    - `ALTER TABLE drafts ADD COLUMN state TEXT NOT NULL DEFAULT 'editing'`;
    - `ALTER TABLE drafts ADD COLUMN last_error TEXT`;
    - `ALTER TABLE drafts ADD COLUMN send_at INTEGER`;
    - `ALTER TABLE ops ADD COLUMN started_at INTEGER`;
    - `PRAGMA user_version = 2`.

    `Store::init` applies V1 then V2 according to `user_version`.
  - In `model.rs`, serde `snake_case` enums and a `camelCase` struct:
    ```rust
    pub enum RemoteImages { Block, Always }
    pub enum Theme { Dark, Light, System }
    pub enum Accent { Blue, Violet, Green, Orange, Pink, Teal }
    pub enum TextSize { Small, Normal, Large }
    pub enum Density { Comfortable, Compact }
    pub struct Settings { pub undo_send_seconds: u32, pub remote_images: RemoteImages, pub theme: Theme,
                          pub accent: Accent, pub text_size: TextSize, pub density: Density, pub show_snippet: bool }
    impl Default for Settings  // 10, Block, Dark, Blue, Normal, Comfortable, true
    ```
  - `Store::settings() -> Result<Settings>` and `Store::set_setting(key: &str, value: serde_json::Value) -> Result<Settings>`. An unknown key, or a value outside the allowed set (`undo_send_seconds` ∉ {5, 10, 30}), gives `InvalidInput`.

- [ ] **Step 1: Write the failing tests** (`store/settings.rs`)
  - `defaults_when_empty`: `settings()` equals `Settings::default()`; `undo_send_seconds == 10`.
  - `set_and_read_back`: set `undo_send_seconds` to 30 and `theme` to `"light"` → read back.
  - `rejects_bad_values`: `("undo_send_seconds", 7)`, `("theme", "pink")` and `("nope", 1)` each give `Err(InvalidInput)`.
  - In `store/mod.rs`, `migrates_v1_database`: create a v1 database (execute `schema::V1` with `user_version = 1`, insert an account and an op), then `Store::open` → `user_version == 2`, the data is still there, and `settings()` works.
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core --lib store::`. Expected: FAIL (compile).
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Run them to verify they pass** — `cargo test -p illy-core --lib`. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(core): schéma v2 et réglages`

### Task 2: Reply-To et carnet de contacts

**Files:**
- Modify: `crates/illy-core/src/mime/headers.rs`, `crates/illy-core/src/imap/fetch.rs` (`HEADER_FIELDS` + `REPLY-TO`), `crates/illy-core/src/store/messages.rs`
- Create: `crates/illy-core/src/store/contacts.rs`

**Interfaces:**
- Produces:
  - `ParsedHeaders.reply_to: Vec<Address>`;
  - `upsert_headers` stores `reply_to_json` and calls `record_contacts` for each **inserted** row;
  - `Store::record_contacts(addrs: &[Address], date: i64, own: &HashSet<String>) -> Result<()>`, where `own` holds the lowercase emails of all accounts;
  - `Store::search_contacts(query: &str, limit: u32) -> Result<Vec<Address>>`;
  - `Store::own_addresses() -> Result<HashSet<String>>`;
  - `Store::message_headers_for_reply(id) -> Result<ReplySource>`, with `ReplySource { from: Address, to: Vec<Address>, cc: Vec<Address>, reply_to: Vec<Address>, subject: String, date: i64, message_id: Option<String>, references: Vec<String>, account_id: AccountId }`.
- Search rules:
  - the query and the stored values are compared lowercase and without accents (`unicode-normalization` NFD with marks stripped, or a manual table for French letters);
  - a contact matches if the email starts with the query, or if any word of the name starts with it;
  - sorted by `count DESC, last_seen_at DESC`, `limit` results at most.

- [ ] **Step 1: Write the failing tests**
  - `headers.rs`: `reply_to_parsed`: a block containing `Reply-To: Liste <liste@ex.fr>` → `reply_to[0].email == "liste@ex.fr"`.
  - `contacts.rs`:
    - `contacts_counted_and_sorted`: Marie twice, Paul once → search `"m"` returns Marie first. `count` grows on repeated recording, and the name is updated only when the new one is non-empty.
    - `search_ignores_accents_and_case`: `Élodie Martin <e@x.fr>` is found with `"elo"`, `"ELO"`, `"mart"` and `"e@"`.
    - `own_addresses_excluded`: recording `Moi@GMAIL.com` when own = `{"moi@gmail.com"}` → not stored.
  - `messages.rs`: `upsert_records_contacts_once`: upsert the same header twice → contact count == 1. `reply_source_roundtrip`: `message_headers_for_reply` returns the stored reply_to, references and message_id.
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core --lib`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Run them to verify they pass** — `cargo test -p illy-core --lib`, and `cargo test -p illy-core --features insecure-plain --test sync_it` still PASS.
- [ ] **Step 5: Commit** — `feat(core): Reply-To et carnet de contacts`

### Task 3: Brouillons et préremplissage

**Files:**
- Create: `crates/illy-core/src/compose/mod.rs`, `crates/illy-core/src/compose/prefill.rs`, `crates/illy-core/src/store/drafts.rs`
- Modify: `crates/illy-core/src/model.rs`, `crates/illy-core/src/lib.rs`

**Interfaces:**
- Produces (`model.rs`; `camelCase` structs, `snake_case` enums):
  ```rust
  pub type DraftId = i64;
  pub enum DraftKind { New, Reply, ReplyAll, Forward }
  pub enum DraftState { Editing, Queued, Sending, Sent, Failed }
  pub enum AttachmentSource { File { path: String }, Part { message_id: MessageId, part_path: String } }
  pub struct DraftAttachment { pub filename: String, pub mime_type: String, pub size: u64, pub source: AttachmentSource }
  pub struct Draft { pub id: DraftId, pub account_id: AccountId, pub kind: DraftKind, pub source_message_id: Option<MessageId>,
                     pub to: Vec<Address>, pub cc: Vec<Address>, pub bcc: Vec<Address>, pub subject: String,
                     pub body_html: String, pub attachments: Vec<DraftAttachment>, pub state: DraftState,
                     pub last_error: Option<String>, pub send_at: Option<i64>, pub remote_uid: Option<u32>, pub updated_at: i64 }
  pub struct DraftInput { pub id: DraftId, pub account_id: AccountId, pub to: Vec<Address>, pub cc: Vec<Address>,
                          pub bcc: Vec<Address>, pub subject: String, pub body_html: String }
  ```
  `AttachmentSource` uses `#[serde(tag = "kind", rename_all = "snake_case")]`.
- `compose::prefill` (pure):
  - `pub fn subject_with_prefix(prefix: &str, subject: &str) -> String`;
  - `pub fn reply_recipients(src: &ReplySource, all: bool, own: &HashSet<String>) -> (Vec<Address>, Vec<Address>)`, returning `(to, cc)`;
  - `pub fn quoted_body(kind: DraftKind, src: &ReplySource, original_html: &str, signature_html: &str) -> String`;
  - `pub fn signature_block(signature_html: &str) -> String`, which gives `<div data-illy-signature>…</div>` or `""` when empty;
  - `pub fn forward_header(src: &ReplySource) -> String`.
  - Date format in the quote: `jeudi 12 novembre 2026 à 14:32`, local time, French days and months written in code (no locale dependency).
- `store/drafts.rs`:
  - `insert_draft(&Draft) -> Result<DraftId>`, `get_draft(id) -> Result<Draft>`, `update_draft(&DraftInput) -> Result<Draft>` (refused with `InvalidInput("brouillon en cours d'envoi")` unless the state is `editing` or `failed`);
  - `set_draft_state(id, DraftState, error: Option<&str>, send_at: Option<i64>)`;
  - `set_draft_attachments(id, &[DraftAttachment])`, `set_draft_remote_uid(id, Option<u32>)`;
  - `draft_for_message(message_id) -> Result<Option<Draft>>` (state editing or failed);
  - `delete_draft(id)`, `outbox() -> Result<Vec<Draft>>` (states queued, sending, failed).

- [ ] **Step 1: Write the failing tests** (`prefill.rs`, `drafts.rs`)
  - `prefix_rules`: `("Re: ", "Devis")` → `"Re: Devis"`; `("Re: ", "RE: Devis")` → unchanged; `("Fwd: ", "tr: x")` → unchanged; `("Fwd: ", "Fw: x")` → unchanged; `("Re: ", "  ")` → `"Re: "`.
  - `reply_uses_reply_to`: reply_to set → to = reply_to; empty → to = from.
  - `reply_all_excludes_self_and_duplicates`: from Marie, to `[MOI@gmail.com, Paul]`, cc `[Paul, Zoé]`, own `{moi@gmail.com}` → to `[Marie]`, cc `[Paul, Zoé]`.
  - `quote_contains_header_and_blockquote`: the reply body contains `a écrit :`, `<blockquote`, the original HTML and the signature block **before** the blockquote. With an empty signature, there is no `data-illy-signature`.
  - `forward_header_lists_fields`: contains `Message transféré`, `De :`, `Date :`, `Objet :`, `À :`.
  - `french_date`: 2026-11-12 14:32 local → `jeudi 12 novembre 2026 à 14:32`.
  - `drafts_roundtrip_and_states`: insert → get; `update_draft` while `queued` → `Err(InvalidInput)`; `outbox()` lists queued and failed, not editing.
  - `draft_for_message_finds_reply`.
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core --lib compose:: store::`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above. The original HTML is the sanitized `message_detail(id, false)` HTML, or `text_to_html(text)`.
- [ ] **Step 4: Run them to verify they pass** — `cargo test -p illy-core --lib`. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(core): brouillons et préremplissage des réponses`

### Task 4: Construction MIME

**Files:**
- Create: `crates/illy-core/src/compose/mime.rs`
- Modify: `crates/illy-core/Cargo.toml` (`uuid = { version = "1", features = ["v4"] }`)

**Interfaces:**
- Produces:
  ```rust
  pub struct OutgoingFile { pub filename: String, pub mime_type: String, pub data: Vec<u8> }
  pub struct BuiltMessage { pub raw_for_storage: Vec<u8>, pub envelope_to: Vec<String>, pub message_id: String }
  pub fn build_message(from: &Address, draft: &Draft, reply: Option<&ReplySource>, files: &[OutgoingFile],
                       draft_marker: Option<DraftId>) -> Result<BuiltMessage>;
  pub fn total_attachment_size(files: &[OutgoingFile]) -> u64;
  ```
- Rules:
  - The `data:` images of `body_html` become `multipart/related` parts with `cid:illy-img-N@illy`.
  - The plain text is `html_to_text(body_html)`.
  - The envelope contains to + cc + bcc.
  - `raw_for_storage` **never** contains `Bcc`.
  - `Message-ID = <uuid@domaine de from>`.
  - With `reply`: `In-Reply-To` + `References`.
  - With `draft_marker`: header `X-Illy-Draft: <id>`.
  - Invalid address → `InvalidInput("Adresse invalide : <adresse>")`.

- [ ] **Step 1: Write the failing tests** (parse the result with `mail_parser`)
  - `alternative_text_and_html`;
  - `inline_image_becomes_cid` (body contains `data:image/png;base64,AQID` → one related part, the HTML references `cid:`, no `data:` left);
  - `attachment_with_accented_name` (`facture n°1 é.pdf` → mail_parser `attachment_name()` returns it intact);
  - `accented_subject_and_from` (`Élodie` / `Réunion à 14h` intact);
  - `bcc_not_in_stored_message_but_in_envelope`;
  - `reply_headers` (In-Reply-To + References = old references + the original message_id);
  - `draft_marker_header`;
  - `invalid_address_rejected`.
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core --lib compose::mime`. Expected: FAIL.
- [ ] **Step 3: Implement** with `lettre::message::{Message, MultiPart, SinglePart, Attachment, header}`. The envelope is built separately, and `raw_for_storage` is produced from a message built without Bcc.
- [ ] **Step 4: Run them to verify they pass** — same command. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(core): construction MIME des envois et brouillons`

### Task 5: Aides IMAP (APPEND, recherche, drapeaux) et envoi SMTP

**Files:**
- Modify: `crates/illy-core/src/imap/fetch.rs`, `crates/illy-core/src/imap/conn.rs` (`Caps.uidplus`), `crates/illy-core/src/imap/mod.rs`, `crates/illy-core/src/smtp.rs`, `crates/illy-core/tests/common/mod.rs` (second user `bob@localhost`, `gm.session_as(user)`)

**Interfaces:**
- Produces:
  - `imap::append(s, folder, flags: &str, raw: &[u8]) -> Result<()>`;
  - `imap::search_header(s, field: &str, value: &str) -> Result<Vec<u32>>`, where the value is quoted and its `"` / `\` are escaped;
  - `imap::add_flag(s, uid, flag: &str) -> Result<()>`;
  - `imap::delete_uid(s, uid, caps: &Caps) -> Result<()>`: `\Deleted`, then `UID EXPUNGE` if `caps.uidplus`, otherwise `EXPUNGE`;
  - `smtp::send_raw(cfg, user, password, envelope_to: &[String], from: &str, raw: &[u8]) -> Result<()>`, with errors mapped like `test_smtp`. A permanent refusal (5xx) keeps the server message.

- [ ] **Step 1: Write the failing tests** (`tests/imap_it.rs`, `tests/core_it.rs`)
  - `append_search_flag_delete`: append to `Drafts` (created) with `\Draft \Seen` → `search_header("X-Illy-Draft", "42")` returns 1 uid → `add_flag(\Answered)` is visible in the flags → `delete_uid` → the search returns `[]`.
  - `smtp_send_reaches_bob`: `send_raw` from alice to bob@localhost → bob's IMAP session sees 1 mail in INBOX with the right subject.
  - `smtp_unknown_recipient_permanent`: GreenMail accepts every recipient, so instead check that a wrong password gives `Err(Auth)` (the 5xx refusal path is tested by mapping in Task 6).
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core --features insecure-plain --test imap_it --test core_it`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Run them to verify they pass** — same command. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(core): APPEND, recherche d'en-tête, drapeaux, envoi SMTP brut`

### Task 6: Ops d'envoi, de brouillon et d'annulation

**Files:**
- Modify: `crates/illy-core/src/store/ops.rs`, `crates/illy-core/src/sync/ops_exec.rs`, `crates/illy-core/src/sync/worker.rs` (passes `password` and `account` to `run_due_ops`)

**Interfaces:**
- Produces:
  - `OpKind::{Send, AppendDraft, SetAnswered, SentCopy}`, with payloads:
    - `Send` and `AppendDraft`: `{ "draftId" }`;
    - `SetAnswered`: `{ "folderId", "uid" }`;
    - `SentCopy`: `{ "draftId", "messageId" }`.
  - `Store::start_op(id) -> Result<bool>` (`UPDATE … SET started_at = now WHERE id = ? AND started_at IS NULL`; false if absent or already started);
  - `Store::cancel_send_op(draft_id) -> Result<bool>` (deletes the `send` op of this draft if `started_at IS NULL`);
  - `Store::enqueue_op_at(account, kind, payload, at: i64) -> Result<i64>`.
  - `run_due_ops(s, caps, store, events, account: &Account, password: &str, cfg)`.
- `Send` behaviour:
  1. `start_op`; if it returns false, the op is skipped.
  2. Draft → `Sending`, and emit `OutboxChanged`.
  3. Collect the files: `File` read from disk; `Part` through `fetch_attachment`. A missing file → `Failed("Pièce jointe introuvable : <nom>")`.
  4. `build_message`, then `send_raw`.
  5. On success:
     - write `<data>/drafts/<id>/sent.eml` ;
     - enqueue `SentCopy` at now + 15 s and `SetAnswered` if the draft is a reply ;
     - delete the remote draft (`delete_uid`) if `remote_uid` is set ;
     - draft → `Sent`.
  6. On failure:
     - `Network` → `op_retry` with backoff, draft → `Queued` ;
     - `Auth` → `Failed` + `op_done` + account status `AuthError` ;
     - `Smtp`/5xx → `Failed` with the message + `op_done`.
- `SentCopy`: `search_header("Message-ID", id)` in the `sent` folder → `append` with `\Seen` if absent → delete the local draft and its folder.
- `AppendDraft`: `build_message(…, draft_marker)` → `append(drafts, "(\Draft \Seen)")` → delete the old `remote_uid` → find the new one through `search_header("X-Illy-Draft")` → `set_draft_remote_uid`.
- Events: `CoreEvent::OutboxChanged { draft_id, state: DraftState, send_at: Option<i64>, error: Option<String> }`.
- The worker receives the data directory (`drafts_dir`) through `WorkerConfig.data_dir: Option<PathBuf>` (None in old tests means a temp folder).

- [ ] **Step 1: Write the failing tests** (store unit tests + `tests/compose_it.rs` against GreenMail with alice and bob)
  - `start_op_once` and `cancel_only_before_start` (store).
  - `send_reaches_bob_with_accents` (Review Focus 1): draft with `Élodie`, subject `Réunion à 14h`, a PDF attachment named `facture é.pdf` → bob receives it with everything intact.
  - `sent_copy_added_once`: after send + SentCopy, the alice `Sent` folder (created) contains exactly 1 copy. A second `SentCopy` run does not duplicate it.
  - `reply_sets_answered`.
  - `append_draft_replaces_previous`: two `AppendDraft` runs → exactly one draft on the server with the latest subject.
  - `cancel_before_due_sends_nothing`: enqueue a send 3 s ahead, `cancel_send_op` → after 5 s, bob has 0 mails.
  - `auth_failure_marks_failed_without_retry` (Review Focus 3): wrong secret → draft `Failed`, the op is gone, there is a single `OutboxChanged(failed)`.
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core --features insecure-plain --test compose_it`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Run them to verify they pass** — `cargo test -p illy-core --features insecure-plain` (whole crate). Expected: PASS.
- [ ] **Step 5: Commit** — `feat(core): envoi, copie dans Envoyés, brouillons serveur, annulation`

### Task 7: Façade Core pour la rédaction

**Files:**
- Modify: `crates/illy-core/src/core.rs`, `crates/illy-core/src/events.rs`
- Test: `crates/illy-core/tests/compose_it.rs`

**Interfaces:**
- Produces, as `Core` methods:
  - `search_contacts(&str) -> Result<Vec<Address>>`;
  - `create_draft(kind, account_id: Option<AccountId>, source: Option<MessageId>) -> Result<Draft>`;
  - `draft_for_message(MessageId) -> Result<Option<Draft>>`;
  - `draft_from_message(MessageId) -> Result<Draft>`, which reads the stored mail into a new `New`-kind draft with its `remote_uid`;
  - `get_draft(DraftId)`, `update_draft(DraftInput) -> Result<Draft>`;
  - `add_attachments(DraftId, Vec<PathBuf>) -> Result<Draft>`, `remove_attachment(DraftId, usize) -> Result<Draft>`;
  - `close_draft(DraftId)`, which enqueues `AppendDraft` unless the draft is empty (no recipient, empty subject, and body without text);
  - `discard_draft(DraftId)`;
  - `send_draft(DraftId) -> Result<SendTicket>`, with `SendTicket { send_at: i64 }`: validates, sets `Queued` and `send_at = now + undo_send_seconds`, enqueues `Send` at that time, sends `OpsAvailable`;
  - `cancel_send(DraftId) -> Result<bool>`;
  - `list_outbox() -> Result<Vec<Draft>>`;
  - `settings() -> Result<Settings>`, `set_setting(&str, Value) -> Result<Settings>`, which emits `SettingsChanged { key }`.
  - `update_draft` also enqueues an `AppendDraft` when the last server copy is more than 60 s old (kept in memory per draft).
  - The `Core::open` data directory becomes `data_dir: &Path` (it holds `illy.db`, `tmp`, `drafts`). Update `src-tauri` and the tests accordingly.
- Rules for `create_draft`:
  - `New`: account = given, else the first one; body = `<p></p>` + signature.
  - Reply kinds: account = the source mail's account; uses `prefill`.
  - `Forward`: attachments = the source's `Attachment` parts as `AttachmentSource::Part`.

- [ ] **Step 1: Write the failing tests** (`compose_it.rs`, Core against GreenMail)
  - `reply_flow_end_to_end`: alice receives a mail from bob → `create_draft(Reply)` → to = bob and subject "Re: …" → `send_draft` with `undo_send_seconds = 5` (set through `set_setting`) → `list_outbox` shows `queued` → within 20 s bob receives the reply and alice's original is `\Answered`.
  - `restart_during_undo_window_sends_once` (Review Focus 2): `send_draft`, drop the `Core` immediately, re-open `Core::open` on the same folder → bob receives exactly 1 mail within 25 s.
  - `forward_keeps_attachments`: forward of `inline_cid_and_pdf.eml` → bob receives `devis.pdf`.
  - `send_validation`: no recipient → `Err(InvalidInput)`; `"pas une adresse"` → `Err(InvalidInput("Adresse invalide : pas une adresse"))`.
  - `close_empty_draft_does_not_upload`.
- [ ] **Step 2: Run them to verify they fail** — `cargo test -p illy-core --features insecure-plain --test compose_it`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Run them to verify they pass** — whole crate, `cargo clippy -p illy-core --all-targets --features insecure-plain -- -D warnings`. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(core): façade de rédaction (brouillons, envoi, annulation, réglages)`

### Task 8: Tauri — commandes, fenêtre « Nouveau mail », pièces jointes

**Files:**
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src-tauri/capabilities/default.json` (`"windows": ["main", "compose-*"]`, plus the `core:webview:allow-*` drag-drop permissions if needed)

**Interfaces:**
- Produces:
  - one command per new Core method (snake_case names; JS arguments in camelCase);
  - `pick_attachments(app, core, draft_id) -> R<Draft>`: multi-file open dialog, then `add_attachments`; cancelling returns the draft unchanged;
  - `open_compose_window(app, draft_id) -> R<()>`: `WebviewWindowBuilder::new(&app, format!("compose-{draft_id}"), WebviewUrl::App(format!("index.html#compose/{draft_id}").into()))` with title "Nouveau mail", 760×640, min 560×420, `decorations(false)`. If it already exists, it is focused.
  - `Core::open` is called with the `Illy` data folder.
- [ ] **Step 1: Write the failing check** — in `commands.rs`, `compose_label_format`: `compose_label(12) == "compose-12"`, and `compose_url(12) == "index.html#compose/12"` (helpers used by `open_compose_window`).
- [ ] **Step 2: Run it to verify it fails** — `cargo test -p illy-app`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Verify** — `cargo test -p illy-app && cargo clippy --workspace --all-targets --features illy-core/insecure-plain -- -D warnings`. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(app): commandes de rédaction et fenêtre Nouveau mail`

### Task 9: Interface — api, état d'envoi et réglages

**Files:**
- Modify: `src/api/types.ts`, `src/api/index.ts`, `src/api/tauri.ts`, `src/api/mock.ts`, `src/state/store.ts`
- Test: `src/state/store.test.ts`

**Interfaces:**
- Produces:
  - TS mirrors of `Draft`, `DraftInput`, `DraftKind`, `DraftState`, `DraftAttachment`, `AttachmentSource`, `SendTicket`, `Settings`, and the events `outbox_changed` / `settings_changed`.
  - `Api` gains: `searchContacts`, `createDraft(kind, accountId?, sourceMessageId?)`, `draftForMessage`, `draftFromMessage`, `getDraft`, `updateDraft`, `pickAttachments`, `addAttachments(draftId, paths)`, `removeAttachment`, `closeDraft`, `discardDraft`, `sendDraft`, `cancelSend`, `listOutbox`, `getSettings`, `setSetting(key, value)`, `openComposeWindow(draftId)`.
  - The mock implements all of them in memory.
  - The store gains:
    - `outbox: Draft[]` and `settings: Settings`, loaded by `init` ;
    - `applyEvent` handles `outbox_changed` (replace or remove by id; a `sent` state stays 3 s in `recentlySent`, then disappears) and `settings_changed` (reload) ;
    - `replyTarget: { messageId: number; draftId: number } | null` and `openReply(kind)` / `closeReply()`.
- [ ] **Step 1: Write the failing tests**
  - `outbox_changed updates queue`;
  - `sent drafts leave the outbox after 3 s` (fake timers);
  - `openReply creates a draft for the selected message` (calls `createDraft('reply', undefined, id)`);
  - `init loads settings and outbox`.
- [ ] **Step 2: Run them to verify they fail** — `npm test`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Run them to verify they pass** — `npm run typecheck && npm test`. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(ui): api et état de la rédaction`

### Task 10: Composant `Composer`

**Files:**
- Create: `src/features/composer/Composer.tsx`, `RecipientField.tsx`, `EditorToolbar.tsx`, `AttachmentList.tsx`, `useAutosave.ts`, `Composer.module.css`, `Composer.test.tsx`, `RecipientField.test.tsx`
- Modify: `package.json` (TipTap dependencies)

**Interfaces:**
- Consumes: Task 9's api and store.
- Produces:
  - `<Composer draftId={number} variant="inline" | "window" onClosed={() => void} />`;
  - `<RecipientField label value={Address[]} onChange />`;
  - `parseAddress(text) -> Address | null` and `formatSize(bytes)` (reused from the reader).
- Behaviour:
  - the fields and the toolbar of spec § 3.5 ;
  - `useAutosave` calls `updateDraft` 2 s after the last change (debounce) and shows « Enregistrement… » then « Brouillon enregistré » ;
  - changing « De » replaces the `[data-illy-signature]` block with the signature of the new account (from `listAccounts`; no signature in lot A → empty) ;
  - Ctrl+Entrée = Envoyer ;
  - images pasted or dropped into the text become data URLs, and above 5 Mo the message « Image trop lourde (5 Mo au plus) » appears ;
  - files dropped outside the text → `addAttachments` with the paths (Tauri: `getCurrentWebview().onDragDropEvent`) ;
  - attachments over 25 Mo → `window.confirm` with the exact spec text ;
  - « Envoyer » → `sendDraft`, then `onClosed()` ;
  - the trash icon → `discardDraft`, then `onClosed()` ;
  - an `InvalidInput` error appears under the recipients.
- [ ] **Step 1: Write the failing tests**
  - `RecipientField.test.tsx`:
    - `comma and Enter create chips`;
    - `invalid address chip is marked` (`aria-invalid`);
    - `Backspace on empty input removes last chip`;
    - `suggestions from searchContacts are selectable with arrows and Enter`.
  - `Composer.test.tsx`:
    - `prefilled draft renders subject and recipients`;
    - `Ctrl+Enter sends and closes`;
    - `send error shows message under recipients`;
    - `over 25 MB asks confirmation and does not send when refused`;
    - `autosave calls updateDraft after 2 s` (fake timers);
    - `discard calls discardDraft`.

  TipTap in jsdom: test the fields and buttons; the editor content is checked through the `editor.getHTML()` exposed in a `data-testid="composer-editor"` element.
- [ ] **Step 2: Run them to verify they fail** — `npm test`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Run them to verify they pass** — `npm run typecheck && npm test`. Expected: PASS. Visual check with `npm run dev` (mock).
- [ ] **Step 5: Commit** — `feat(ui): composant de rédaction (TipTap, destinataires, pièces jointes)`

### Task 11: Intégration — réponses, fenêtre, bandeau d'envoi, clic droit

**Files:**
- Create: `src/features/compose-window/ComposeWindow.tsx`, `src/features/outbox/SendBanner.tsx` (+ css, test), `src/ui/ContextMenu.tsx` (+ test)
- Modify: `src/main.tsx` (route `#compose/<id>`), `src/features/reader/Reader.tsx` (+ test), `src/features/sidebar/Sidebar.tsx`, `src/features/message-list/MessageRow.tsx`, `MessageList.tsx`, `src/ui/TitleBar.tsx` (+ test)

**Interfaces:**
- Consumes: Tasks 9 and 10.
- Produces:
  - `<ContextMenu items={{label, icon, onSelect}[]} position onClose />`;
  - `<SendBanner />`, shown in the main window;
  - `ComposeWindow`: its own title bar (« Nouveau mail » + minimize/close) + `<Composer variant="window">`. Closing the window calls `closeDraft` (via `onCloseRequested`).
- Behaviour:
  - Reader:
    - Répondre, Répondre à tous and Transférer are enabled and call `openReply` ; the inline `Composer` sits under the mail card ;
    - when `draftForMessage` finds a draft: a button « Reprendre la réponse » ;
    - in a `drafts`-role folder: « Modifier ce brouillon » → `draftFromMessage`, then `openComposeWindow`.
  - Sidebar: « Nouveau mail » is enabled → `createDraft('new', accountIdOfScope)` → `openComposeWindow`.
  - MessageRow: `onContextMenu` → selects the message and opens the menu Répondre / Répondre à tous / Transférer.
  - SendBanner:
    - for each queued draft whose `sendAt` is in the future: « Envoi dans {n} s · Annuler » (counting down every second) ;
    - after `sent`: « Mail envoyé » (3 s) ;
    - `failed`: « Échec de l'envoi à {premier destinataire} : {erreur} · Réessayer · Modifier » ;
    - Annuler → `cancelSend` → when it returns true, opens the draft (inline if it is the selected message, otherwise in a window).
  - TitleBar: a pill « {n} envoi(s) en attente » when queued drafts have `sendAt` in the past.
- [ ] **Step 1: Write the failing tests**
  - `ContextMenu`:
    - `arrow keys and Enter select`;
    - `Escape and outside click close`.
  - `SendBanner`:
    - `countdown and cancel`;
    - `failure shows retry and edit`;
    - `sent message disappears after 3 s`.
  - `Reader`:
    - `reply buttons enabled and open inline composer`;
    - `resume reply shown when a draft exists`;
    - `edit draft button in drafts folder`.
  - `TitleBar`: `pending sends pill`.
  - `MessageList`: `right click opens menu with reply actions`.
- [ ] **Step 2: Run them to verify they fail** — `npm test`. Expected: FAIL.
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Run them to verify they pass** — `npm run typecheck && npm test`. Expected: PASS. Visual check (Playwright on the mock): inline reply, compose window, send banner, context menu.
- [ ] **Step 5: Commit** — `feat(ui): réponses, fenêtre Nouveau mail, bandeau d'envoi, clic droit`

### Task 12: Version 0.2.0, vérification de bout en bout, checklist

**Files:**
- Create: `scripts/bump-version.mjs`, `scripts/bump-version.test.mjs`, `docs/checklists/lot-a.md`
- Modify: `package.json`, `Cargo.toml` (workspace version), `src-tauri/tauri.conf.json` (through the script), `README.md`

**Interfaces:**
- Produces: `node scripts/bump-version.mjs <x.y.z>`, which validates semver and writes the 3 files.

- [ ] **Step 1: Write the failing test** — `scripts/bump-version.test.mjs` (run by Vitest: add `scripts/**/*.test.mjs` to `test.include`): on copies of the 3 files in a temp folder, `bump('0.2.0', dir)` updates the 3 versions, and `bump('abc')` throws.
- [ ] **Step 2: Run it to verify it fails** — `npm test`. Expected: FAIL.
- [ ] **Step 3: Implement** the script, then run it: `node scripts/bump-version.mjs 0.2.0`.
- [ ] **Step 4: Verify**
  - full suite (`cargo test -p illy-core --features insecure-plain`, `npm test`, clippy) ;
  - Linux smoke test of the real application (Xvfb + xdotool: opening « Nouveau mail » creates the window) ;
  - push → CI green (tests + Windows installer).
- [ ] **Step 5: Write `docs/checklists/lot-a.md`**: new mail, reply, reply all, forward with attachment, cancel within 10 s, sent copy (Gmail: only once), draft visible in the webmail, offline send then automatic sending, wrong password at send time, attachment > 25 MB, right click, accents in subject and attachment. Commit: `chore: version 0.2.0 et checklist du lot A`.
