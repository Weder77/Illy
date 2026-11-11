//! Base locale SQLite : miroir des serveurs IMAP.

mod accounts;
mod allowlist;
mod folders;
mod schema;

pub use folders::{FolderInfo, FolderSyncState};

use crate::error::{CoreError, Result};
use rusqlite::Connection;
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Store> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Store> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < schema::SCHEMA_VERSION {
            conn.execute_batch(&format!(
                "BEGIN; {} PRAGMA user_version = {}; COMMIT;",
                schema::V1,
                schema::SCHEMA_VERSION
            ))?;
        }
        Ok(Store { conn: Arc::new(Mutex::new(conn)) })
    }

    /// Exécute `f` avec la connexion (verrou tenu pendant l'appel).
    pub(crate) fn with<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        let mut guard = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut guard)
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

pub(crate) fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(ver, 1);
        drop(s);
        Store::open(&path).unwrap();
    }
}
