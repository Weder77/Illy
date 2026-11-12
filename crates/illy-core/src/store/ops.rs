use super::{dec, enc, now_ts, Store};
use crate::error::Result;
use crate::model::AccountId;
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// Durée après laquelle une action réservée (`started_at`) est considérée comme abandonnée (plantage).
pub const OP_LOCK_SECS: i64 = 600;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpKind {
    SetSeen,
    Send,
    AppendDraft,
    SetAnswered,
    SentCopy,
    DeleteRemote,
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

    pub fn enqueue_op_at(&self, account: AccountId, kind: OpKind, payload: serde_json::Value, at: i64) -> Result<i64> {
        let now = now_ts();
        self.with(|c| {
            c.execute(
                "INSERT INTO ops (account_id, kind, payload_json, next_attempt_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![account, enc(&kind), payload.to_string(), at, now],
            )?;
            Ok(c.last_insert_rowid())
        })
    }

    /// Réserve une action avant de l'exécuter ; `false` si elle est absente ou déjà en cours
    /// (une réservation de plus de 10 min est considérée comme abandonnée après un plantage).
    pub fn start_op(&self, id: i64) -> Result<bool> {
        let now = now_ts();
        self.with(|c| {
            Ok(c.execute(
                "UPDATE ops SET started_at = ?1 WHERE id = ?2 AND (started_at IS NULL OR started_at < ?1 - ?3)",
                params![now, id, OP_LOCK_SECS],
            )? > 0)
        })
    }

    /// Annule l'envoi d'un brouillon s'il n'a pas commencé.
    pub fn cancel_send_op(&self, draft_id: i64) -> Result<bool> {
        self.with(|c| {
            Ok(c.execute(
                "DELETE FROM ops WHERE kind = 'send' AND started_at IS NULL AND json_extract(payload_json, '$.draftId') = ?1",
                [draft_id],
            )? > 0)
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
                "UPDATE ops SET attempts = attempts + 1, next_attempt_at = ?1, last_error = ?2, started_at = NULL WHERE id = ?3",
                params![next_attempt_at, error, id],
            )?;
            Ok(())
        })
    }

    /// Échéance de la prochaine action en attente d'un compte.
    /// Une action en cours (`started_at`) n'est pas due avant l'expiration de son verrou.
    pub fn next_op_at(&self, account: AccountId) -> Result<Option<i64>> {
        let stale = now_ts() - OP_LOCK_SECS;
        self.with(|c| {
            Ok(c.query_row(
                "SELECT MIN(CASE WHEN started_at IS NOT NULL AND started_at >= ?2 THEN started_at + ?3 ELSE next_attempt_at END) \
                 FROM ops WHERE account_id = ?1",
                params![account, stale, OP_LOCK_SECS],
                |r| r.get(0),
            )?)
        })
    }

    pub fn pending_ops_count(&self, account: AccountId) -> Result<u32> {
        self.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM ops WHERE account_id = ?1", [account], |r| r.get(0))?))
    }

    /// Retour du réseau : les actions en échec sont réessayées tout de suite.
    /// Un envoi jamais tenté garde son délai d'annulation, et la copie dans Envoyés ses 15 s.
    pub fn ops_due_now(&self, account: AccountId) -> Result<()> {
        let now = now_ts();
        self.with(|c| {
            c.execute(
                "UPDATE ops SET next_attempt_at = ?1 WHERE account_id = ?2 AND next_attempt_at > ?1 \
                 AND NOT (kind IN ('send', 'sent_copy') AND attempts = 0)",
                params![now, account],
            )?;
            Ok(())
        })
    }

    /// Rend toutes les actions d'un compte dues immédiatement, délais compris (tests).
    #[doc(hidden)]
    pub fn force_ops_due(&self, account: AccountId) -> Result<()> {
        self.with(|c| {
            c.execute("UPDATE ops SET next_attempt_at = ?1 WHERE account_id = ?2", params![now_ts(), account])?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_op_once() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert_account(&new_account("a@x.fr")).unwrap().id;
        let id = s.enqueue_op(a, OpKind::Send, serde_json::json!({"draftId": 1})).unwrap();
        assert!(s.start_op(id).unwrap());
        assert!(!s.start_op(id).unwrap(), "already running");
        s.op_retry(id, now_ts(), "réseau").unwrap();
        assert!(s.start_op(id).unwrap(), "a retried op can start again");
        assert!(!s.start_op(999).unwrap());
    }

    #[test]
    fn reconnect_keeps_the_undo_window() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert_account(&new_account("a@x.fr")).unwrap().id;
        let now = now_ts();
        let send = s.enqueue_op_at(a, OpKind::Send, serde_json::json!({"draftId": 1}), now + 30).unwrap();
        let copy = s.enqueue_op_at(a, OpKind::SentCopy, serde_json::json!({"draftId": 2}), now + 15).unwrap();
        let retry = s.enqueue_op(a, OpKind::Send, serde_json::json!({"draftId": 3})).unwrap();
        s.op_retry(retry, now + 300, "réseau").unwrap();
        let seen = s.enqueue_op(a, OpKind::SetSeen, serde_json::json!({"folderId": 1, "uid": 2, "seen": true})).unwrap();
        s.op_retry(seen, now + 300, "réseau").unwrap();
        s.ops_due_now(a).unwrap();
        let due: Vec<i64> = s.due_ops(a, now_ts()).unwrap().into_iter().map(|o| o.id).collect();
        assert!(!due.contains(&send), "an untried send keeps its undo window");
        assert!(!due.contains(&copy), "the sent copy keeps its 15 s delay");
        assert!(due.contains(&retry) && due.contains(&seen), "failed ops are retried at once");
    }

    #[test]
    fn locked_op_does_not_wake_the_worker_in_a_loop() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert_account(&new_account("a@x.fr")).unwrap().id;
        let id = s.enqueue_op(a, OpKind::Send, serde_json::json!({"draftId": 1})).unwrap();
        assert!(s.start_op(id).unwrap());
        let next = s.next_op_at(a).unwrap().unwrap();
        assert!(next >= now_ts() + 590, "a running send is not due again before its lock expires ({next})");
    }

    #[test]
    fn cancel_only_before_start() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert_account(&new_account("a@x.fr")).unwrap().id;
        let id = s.enqueue_op_at(a, OpKind::Send, serde_json::json!({"draftId": 5}), now_ts() + 60).unwrap();
        assert!(s.due_ops(a, now_ts()).unwrap().is_empty(), "not due before its time");
        assert!(s.cancel_send_op(5).unwrap());
        assert!(!s.cancel_send_op(5).unwrap());
        let id2 = s.enqueue_op(a, OpKind::Send, serde_json::json!({"draftId": 6})).unwrap();
        assert!(s.start_op(id2).unwrap());
        assert!(!s.cancel_send_op(6).unwrap(), "too late once started");
        let _ = id;
    }
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
