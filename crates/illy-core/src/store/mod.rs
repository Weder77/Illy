//! Base locale SQLite : miroir des serveurs IMAP.

mod accounts;
mod contacts;
mod drafts;
mod allowlist;
mod folders;
mod messages;
mod ops;
mod schema;
mod settings;

pub use folders::{FolderInfo, FolderSyncState};
pub use messages::{NewHeader, ReplySource};
pub use ops::{Op, OpKind};

use crate::error::{CoreError, Result};
use rusqlite::Connection;
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Accès SQLite : une connexion d'écriture (synchro) et une connexion de lecture (interface),
/// pour que l'affichage ne bloque jamais pendant une écriture (mode WAL).
#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
    read: Arc<Mutex<Connection>>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let mut store = Self::init(conn)?;
        let read = Connection::open(path)?;
        read.pragma_update(None, "query_only", "ON")?;
        read.busy_timeout(std::time::Duration::from_secs(5))?;
        store.read = Arc::new(Mutex::new(read));
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Store> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Store> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let steps: [(i64, &str); 3] = [(1, schema::V1), (2, schema::V2), (3, schema::V3)];
        for (target, sql) in steps {
            if version < target {
                conn.execute_batch(&format!("BEGIN; {sql} PRAGMA user_version = {target}; COMMIT;"))?;
            }
        }
        debug_assert_eq!(steps.len() as i64, schema::SCHEMA_VERSION);
        let conn = Arc::new(Mutex::new(conn));
        Ok(Store { read: conn.clone(), conn })
    }

    /// Exécute `f` avec la connexion d'écriture (verrou tenu pendant l'appel).
    pub(crate) fn with<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        let mut guard = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut guard)
    }

    /// Lecture seule, sur la connexion de lecture (n'attend pas les écritures en cours).
    pub(crate) fn with_read<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let guard = self.read.lock().unwrap_or_else(|p| p.into_inner());
        f(&guard)
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(e: serde_json::Error) -> Self {
        CoreError::InvalidInput(format!("JSON : {e}"))
    }
}

/// Encode une énumération serde (`rename_all = "snake_case"`) en texte.
pub(crate) fn enc<T: Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => s,
        other => panic!("enum attendu, obtenu {other:?}"),
    }
}

/// Décode une énumération stockée en texte.
pub(crate) fn dec<T: DeserializeOwned>(s: &str) -> rusqlite::Result<T> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

pub fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_v1_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("illy.db");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 1; COMMIT;", schema::V1)).unwrap();
            c.execute(
                "INSERT INTO accounts (email, display_name, provider, imap_host, imap_port, imap_tls, smtp_host, smtp_port, smtp_tls, color, sort_order) \
                 VALUES ('a@x.fr', 'A', 'custom', 'h', 993, 'implicit', 'h', 465, 'implicit', '#8B5CF6', 0)",
                [],
            )
            .unwrap();
        }
        let s = Store::open(&path).unwrap();
        let v: i64 = s.with(|c| Ok(c.query_row("PRAGMA user_version", [], |r| r.get(0))?)).unwrap();
        assert_eq!(v, 3);
        assert_eq!(s.list_accounts().unwrap().len(), 1);
        assert_eq!(s.settings().unwrap(), crate::model::Settings::default());
        let cols: i64 = s
            .with(|c| Ok(c.query_row("SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name = 'reply_to_json'", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(cols, 1);
    }

    #[test]
    fn migrates_a_v2_database_from_an_early_build() {
        // Une base V2 créée par un installateur de test (avant la colonne `edited`) reçoit V3.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("illy.db");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(&format!("BEGIN; {} {} PRAGMA user_version = 2; COMMIT;", schema::V1, schema::V2)).unwrap();
        }
        let s = Store::open(&path).unwrap();
        let cols: i64 = s
            .with(|c| Ok(c.query_row("SELECT COUNT(*) FROM pragma_table_info('drafts') WHERE name = 'edited'", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(cols, 1);
    }

    #[test]
    fn open_sets_wal_and_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("illy.db");
        let s = Store::open(&path).unwrap();
        let (mode, ver): (String, i64) = s
            .with(|c| {
                let m: String = c.query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
                let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0))?;
                Ok((m, v))
            })
            .unwrap();
        assert_eq!(mode, "wal");
        assert_eq!(ver, 3);
        drop(s);
        Store::open(&path).unwrap();
    }
}
