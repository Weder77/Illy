use super::{dec, enc, now_ts, Store};
use crate::error::Result;
use crate::model::AccountId;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpKind {
    SetSeen,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Op {
    pub id: i64,
    pub account_id: AccountId,
    pub kind: OpKind,
    pub payload: serde_json::Value,
    pub attempts: u32,
}

impl Store {
    pub fn enqueue_op(&self, account: AccountId, kind: OpKind, payload: serde_json::Value) -> Result<i64> {
        let now = now_ts();
        self.with(|c| {
            c.execute(
                "INSERT INTO ops (account_id, kind, payload_json, next_attempt_at, created_at) VALUES (?1, ?2, ?3, ?4, ?4)",
                params![account, enc(&kind), payload.to_string(), now],
            )?;
            Ok(c.last_insert_rowid())
        })
    }

    pub fn due_ops(&self, account: AccountId, now: i64) -> Result<Vec<Op>> {
        self.with(|c| {
            let mut st = c.prepare(
                "SELECT id, account_id, kind, payload_json, attempts FROM ops \
                 WHERE account_id = ?1 AND next_attempt_at <= ?2 ORDER BY id",
            )?;
            let v = st
                .query_map(params![account, now], |r| {
                    Ok(Op {
                        id: r.get(0)?,
                        account_id: r.get(1)?,
                        kind: dec(&r.get::<_, String>(2)?)?,
                        payload: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
                        attempts: r.get(4)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(v)
        })
    }

    pub fn op_done(&self, id: i64) -> Result<()> {
        self.with(|c| {
            c.execute("DELETE FROM ops WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    pub fn op_retry(&self, id: i64, next_attempt_at: i64, error: &str) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE ops SET attempts = attempts + 1, next_attempt_at = ?1, last_error = ?2 WHERE id = ?3",
                params![next_attempt_at, error, id],
            )?;
            Ok(())
        })
    }

    pub fn pending_ops_count(&self, account: AccountId) -> Result<u32> {
        self.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM ops WHERE account_id = ?1", [account], |r| r.get(0))?))
    }

    /// Avance l'échéance de toutes les actions d'un compte (retour du réseau).
    pub fn ops_due_now(&self, account: AccountId) -> Result<()> {
        let now = now_ts();
        self.with(|c| {
            c.execute("UPDATE ops SET next_attempt_at = ?1 WHERE account_id = ?2 AND next_attempt_at > ?1", params![now, account])?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::accounts::tests::new_account;
    use crate::store::Store;

    #[test]
    fn ops_queue_lifecycle() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert_account(&new_account("a@x.fr")).unwrap().id;
        let id = s.enqueue_op(a, OpKind::SetSeen, serde_json::json!({"folderId": 1, "uid": 2, "seen": true})).unwrap();
        let now = crate::store::now_ts();
        let due = s.due_ops(a, now).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].kind, OpKind::SetSeen);
        assert_eq!(due[0].payload["uid"], 2);
        s.op_retry(id, now + 60, "réseau").unwrap();
        assert!(s.due_ops(a, now).unwrap().is_empty());
        let later = s.due_ops(a, now + 60).unwrap();
        assert_eq!(later[0].attempts, 1);
        s.op_done(id).unwrap();
        assert!(s.due_ops(a, now + 1000).unwrap().is_empty());
        assert_eq!(s.pending_ops_count(a).unwrap(), 0);
    }
}
