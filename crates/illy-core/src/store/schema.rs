//! Schéma SQLite v1 (spec § 4).

pub const SCHEMA_VERSION: i64 = 1;

pub const V1: &str = r#"
CREATE TABLE accounts (
    id INTEGER PRIMARY KEY,
    email TEXT NOT NULL,
    display_name TEXT NOT NULL,
    provider TEXT NOT NULL,
    imap_host TEXT NOT NULL,
    imap_port INTEGER NOT NULL,
    imap_tls TEXT NOT NULL,
    smtp_host TEXT NOT NULL,
    smtp_port INTEGER NOT NULL,
    smtp_tls TEXT NOT NULL,
    signature_html TEXT NOT NULL DEFAULT '',
    color TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'ok',
    last_error TEXT
);

CREATE TABLE folders (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    imap_path TEXT NOT NULL,
    display_name TEXT NOT NULL,
    role TEXT NOT NULL,
    hidden INTEGER NOT NULL DEFAULT 0,
    uidvalidity INTEGER,
    uidnext INTEGER,
    highest_modseq INTEGER,
    last_sync_at INTEGER,
    UNIQUE(account_id, imap_path)
);

CREATE TABLE messages (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    folder_id INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    uid INTEGER NOT NULL,
    message_id TEXT,
    in_reply_to TEXT,
    refs TEXT,
    subject TEXT NOT NULL DEFAULT '',
    from_name TEXT,
    from_addr TEXT NOT NULL DEFAULT '',
    to_json TEXT NOT NULL DEFAULT '[]',
    cc_json TEXT NOT NULL DEFAULT '[]',
    date INTEGER NOT NULL,
    seen INTEGER NOT NULL DEFAULT 0,
    answered INTEGER NOT NULL DEFAULT 0,
    has_attachments INTEGER NOT NULL DEFAULT 0,
    snippet TEXT NOT NULL DEFAULT '',
    size INTEGER NOT NULL DEFAULT 0,
    body_state TEXT NOT NULL DEFAULT 'none',
    body_text TEXT,
    body_html TEXT,
    has_remote_images INTEGER NOT NULL DEFAULT 0,
    degraded INTEGER NOT NULL DEFAULT 0,
    UNIQUE(folder_id, uid)
);
CREATE INDEX messages_folder_date ON messages(folder_id, date DESC, id DESC);
CREATE INDEX messages_account_msgid ON messages(account_id, message_id);
CREATE INDEX messages_account_body ON messages(account_id, body_state, date DESC);
CREATE INDEX messages_date ON messages(date DESC, id DESC);
CREATE INDEX messages_folder_seen ON messages(folder_id, seen);

CREATE TABLE parts (
    id INTEGER PRIMARY KEY,
    message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    part_path TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    filename TEXT,
    size INTEGER NOT NULL DEFAULT 0,
    content_id TEXT,
    encoding TEXT NOT NULL DEFAULT '7bit',
    charset TEXT,
    kind TEXT NOT NULL,
    data BLOB,
    UNIQUE(message_id, part_path)
);

CREATE VIRTUAL TABLE messages_fts USING fts5(
    subject, from_text, to_text, body_text, attachment_names,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TABLE contacts (
    email TEXT PRIMARY KEY,
    name TEXT,
    last_seen_at INTEGER,
    count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE image_allowlist (
    sender_email TEXT PRIMARY KEY
);

CREATE TABLE drafts (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    mode TEXT NOT NULL,
    source_message_id INTEGER,
    to_json TEXT NOT NULL DEFAULT '[]',
    cc_json TEXT NOT NULL DEFAULT '[]',
    bcc_json TEXT NOT NULL DEFAULT '[]',
    subject TEXT NOT NULL DEFAULT '',
    body_html TEXT NOT NULL DEFAULT '',
    attachments_json TEXT NOT NULL DEFAULT '[]',
    remote_uid INTEGER,
    updated_at INTEGER NOT NULL
);

CREATE TABLE ops (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at INTEGER NOT NULL,
    last_error TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX ops_due ON ops(account_id, next_attempt_at);
"#;
