//! Exécution de la file d'actions vers le serveur.

use super::WorkerConfig;
use crate::error::{CoreError, Result};
use crate::events::{CoreEvent, EventSink};
use crate::imap::{select, store_seen, Caps, ImapSession};
use crate::model::{Account, FolderId};
use crate::store::{now_ts, Op, OpKind, Store};
use std::time::Duration;

/// Délai avant la tentative n° `attempt` (1 = première reprise) : double à chaque fois, plafonné.
pub fn backoff(attempt: u32, cfg: &WorkerConfig) -> Duration {
    let factor = 2u32.saturating_pow(attempt.saturating_sub(1).min(16));
    cfg.backoff_min.saturating_mul(factor).min(cfg.backoff_max)
}

fn is_transient(e: &CoreError) -> bool {
    matches!(e, CoreError::Network(_) | CoreError::Parse(_) | CoreError::Tls(_))
}

/// Actions à exécuter : toutes, ou seulement « lu / non lu » (avant la première relecture des drapeaux).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpsScope {
    All,
    SeenOnly,
}

/// Tentatives d'une action (hors envoi) avant de l'abandonner.
const MAX_OP_ATTEMPTS: u32 = 10;

/// Temps laissé à une action : 2 min, plus le temps d'envoyer ses pièces jointes à 160 kbit/s.
fn op_budget(store: &Store, op: &Op) -> Duration {
    let bytes = match op.kind {
        OpKind::Send | OpKind::AppendDraft => op.payload["draftId"]
            .as_i64()
            .and_then(|id| store.get_draft(id).ok())
            .map(|d| d.attachments.iter().map(|a| a.size).sum::<u64>() + d.body_html.len() as u64)
            .unwrap_or(0),
        _ => 0,
    };
    // Le MIME (base64) fait 4/3 de la taille ; 20 000 octets/s ≈ 160 kbit/s.
    Duration::from_secs(120 + bytes * 4 / 3 / 20_000)
}

pub async fn run_due_ops(
    s: &mut ImapSession,
    caps: &Caps,
    store: &Store,
    events: &dyn EventSink,
    account_info: &Account,
    password: &str,
    cfg: &WorkerConfig,
) -> Result<()> {
    run_ops(s, caps, store, events, account_info, password, cfg, OpsScope::All).await
}

#[allow(clippy::too_many_arguments)]
pub async fn run_ops(
    s: &mut ImapSession,
    caps: &Caps,
    store: &Store,
    events: &dyn EventSink,
    account_info: &Account,
    password: &str,
    cfg: &WorkerConfig,
    scope: OpsScope,
) -> Result<()> {
    let account = account_info.id;
    for op in store.due_ops(account, now_ts())? {
        if scope == OpsScope::SeenOnly && op.kind != OpKind::SetSeen {
            continue;
        }
        let budget = op_budget(store, &op);
        let run = async {
            match op.kind {
                OpKind::Send => super::send::exec_send(s, caps, store, events, account_info, password, cfg, &op).await,
                OpKind::SentCopy => super::send::exec_sent_copy(s, caps, store, account_info, cfg, &op).await,
                OpKind::SetAnswered => super::send::exec_set_answered(s, store, &op).await,
                OpKind::AppendDraft => super::send::exec_append_draft(s, caps, store, account_info, &op).await,
                OpKind::DeleteRemote => super::send::exec_delete_remote(s, caps, store, &op).await,
                OpKind::SetSeen => Ok(()),
            }
        };
        let r = match tokio::time::timeout(budget, run).await {
            Ok(r) => r,
            Err(_) => {
                if op.kind == OpKind::Send {
                    super::send::send_interrupted(store, events, cfg, &op)?;
                }
                Err(CoreError::Network("délai dépassé".into()))
            }
        };
        match r {
            Ok(()) => {}
            Err(e) if is_transient(&e) => {
                if op.kind != OpKind::Send {
                    if op.attempts + 1 >= MAX_OP_ATTEMPTS && op.kind != OpKind::SetSeen {
                        tracing::warn!(op = op.id, kind = ?op.kind, "action abandonnée après {MAX_OP_ATTEMPTS} essais : {e}");
                        store.op_done(op.id)?;
                    } else {
                        let wait = backoff(op.attempts + 1, cfg).as_secs() as i64;
                        store.op_retry(op.id, now_ts() + wait.max(1), &e.to_string())?;
                    }
                }
                return Err(e);
            }
            Err(e) => {
                tracing::warn!(op = op.id, "action abandonnée : {e}");
                store.op_done(op.id)?;
            }
        }
        if op.kind == OpKind::SetSeen {
            {
                let folder: FolderId = op.payload["folderId"].as_i64().unwrap_or(-1);
                let uid = op.payload["uid"].as_u64().unwrap_or(0) as u32;
                let seen = op.payload["seen"].as_bool().unwrap_or(true);
                let path = match store.folder_path(folder) {
                    Ok(p) => p,
                    Err(CoreError::NotFound) => {
                        store.op_done(op.id)?;
                        continue;
                    }
                    Err(e) => return Err(e),
                };
                let r = async {
                    select(s, &path, false).await?;
                    store_seen(s, uid, seen).await
                }
                .await;
                match r {
                    Ok(()) => store.op_done(op.id)?,
                    Err(e) if is_transient(&e) => {
                        let wait = backoff(op.attempts + 1, cfg).as_secs() as i64;
                        store.op_retry(op.id, now_ts() + wait.max(1), &e.to_string())?;
                        return Err(e);
                    }
                    Err(e) => {
                        tracing::warn!(op = op.id, "action refusée par le serveur : {e}");
                        store.op_done(op.id)?;
                        if let Some(id) = store.message_id_by_uid(folder, uid)? {
                            store.set_seen_local(id, !seen)?;
                            events.emit(CoreEvent::MessageUpdated { message_id: id });
                            events.emit(CoreEvent::FolderChanged { account_id: account, folder_id: folder });
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn backoff_doubles_and_caps() {
        let cfg = WorkerConfig { backoff_min: Duration::from_secs(5), backoff_max: Duration::from_secs(300), ..Default::default() };
        assert_eq!(backoff(1, &cfg), Duration::from_secs(5));
        assert_eq!(backoff(2, &cfg), Duration::from_secs(10));
        assert_eq!(backoff(4, &cfg), Duration::from_secs(40));
        assert_eq!(backoff(20, &cfg), Duration::from_secs(300));
        assert_eq!(backoff(0, &cfg), Duration::from_secs(5));
    }
}
