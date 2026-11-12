//! Actions de rédaction : envoi SMTP, copie dans Envoyés, brouillon serveur, drapeau « répondu ».

use super::{backoff, fetch_attachment, WorkerConfig};
use crate::compose::mime::{build_message, OutgoingFile};
use crate::error::{CoreError, Result};
use crate::events::{CoreEvent, EventSink};
use crate::imap::{add_flag, append, delete_uid, search_message_id, select, Caps, ImapSession};
use crate::model::*;
use crate::store::{now_ts, Op, OpKind, Store};

fn draft_id(op: &Op) -> DraftId {
    op.payload["draftId"].as_i64().unwrap_or(-1)
}

fn sender(account: &Account) -> Address {
    Address { name: Some(account.display_name.clone()).filter(|n| !n.trim().is_empty()), email: account.email.clone() }
}

fn emit(events: &dyn EventSink, d: &Draft, state: DraftState, error: Option<String>) {
    events.emit(CoreEvent::OutboxChanged { draft_id: d.id, state, send_at: d.send_at, error });
}

fn transient(e: &CoreError) -> bool {
    matches!(e, CoreError::Network(_) | CoreError::Tls(_) | CoreError::Parse(_))
}

/// Fichiers d'un brouillon ; `Err(message)` si une pièce jointe est introuvable.
async fn collect_files(
    s: &mut ImapSession,
    store: &Store,
    d: &Draft,
    include_part: impl Fn(MessageId) -> bool,
) -> Result<std::result::Result<Vec<OutgoingFile>, String>> {
    let mut files = Vec::new();
    for att in &d.attachments {
        let data = match &att.source {
            AttachmentSource::File { path } => match tokio::fs::read(path).await {
                Ok(b) => b,
                Err(_) => return Ok(Err(format!("Pièce jointe introuvable : {}", att.filename))),
            },
            AttachmentSource::Part { message_id, part_path } => {
                if !include_part(*message_id) {
                    continue;
                }
                match fetch_attachment(s, store, *message_id, part_path).await {
                    Ok(b) => b,
                    Err(e) if transient(&e) => return Err(e),
                    Err(_) => return Ok(Err(format!("Pièce jointe introuvable : {}", att.filename))),
                }
            }
        };
        files.push(OutgoingFile { filename: att.filename.clone(), mime_type: att.mime_type.clone(), data });
    }
    Ok(Ok(files))
}

fn reply_source(store: &Store, d: &Draft) -> Option<crate::store::ReplySource> {
    match d.kind {
        DraftKind::Reply | DraftKind::ReplyAll => d.source_message_id.and_then(|m| store.message_headers_for_reply(m).ok()),
        _ => None,
    }
}

/// Essais de la copie dans Envoyés avant d'y renoncer (et de nettoyer le brouillon envoyé).
const SENT_COPY_ATTEMPTS: u32 = 6;

/// Refus temporaires (4xx) acceptés avant d'abandonner l'envoi.
const SMTP_TEMPORARY_RETRIES: u32 = 8;

/// Nombre de nouvelles tentatives quand le dossier visé (Envoyés, Brouillons) n'est pas encore connu.
const MISSING_FOLDER_RETRIES: u32 = 5;

/// Les dossiers ne sont peut-être pas encore listés (compte tout juste ajouté) : on réessaie dans 30 s.
fn retry_until_folder_known(store: &Store, op: &Op) -> Result<()> {
    if op.attempts >= MISSING_FOLDER_RETRIES {
        tracing::warn!(op = op.id, "dossier introuvable, action abandonnée");
        return store.op_done(op.id);
    }
    store.op_retry(op.id, now_ts() + 30, "dossier pas encore connu")
}

/// Envoi interrompu (délai dépassé) : libère l'action et remet le brouillon en file.
pub(crate) fn send_interrupted(store: &Store, events: &dyn EventSink, cfg: &WorkerConfig, op: &Op) -> Result<()> {
    let msg = "envoi interrompu (connexion trop lente)";
    store.op_retry(op.id, now_ts() + backoff(op.attempts + 1, cfg).as_secs() as i64, msg)?;
    if let Ok(d) = store.get_draft(draft_id(op)) {
        if d.state == DraftState::Sending {
            store.set_draft_state(d.id, DraftState::Queued, Some(msg), d.send_at)?;
            emit(events, &d, DraftState::Queued, Some(msg.into()));
        }
    }
    Ok(())
}

fn fail(store: &Store, events: &dyn EventSink, op: &Op, d: &Draft, msg: String) -> Result<()> {
    store.op_done(op.id)?;
    store.set_draft_state(d.id, DraftState::Failed, Some(&msg), d.send_at)?;
    emit(events, d, DraftState::Failed, Some(msg));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn exec_send(
    s: &mut ImapSession,
    caps: &Caps,
    store: &Store,
    events: &dyn EventSink,
    account: &Account,
    password: &str,
    cfg: &WorkerConfig,
    op: &Op,
) -> Result<()> {
    if !store.start_op(op.id)? {
        return Ok(());
    }
    let d = match store.get_draft(draft_id(op)) {
        // `Sending` : reprise après un plantage pendant l'envoi (verrou expiré).
        Ok(d) if matches!(d.state, DraftState::Queued | DraftState::Sending) => d,
        Ok(_) | Err(CoreError::NotFound) => return store.op_done(op.id),
        Err(e) => return Err(e),
    };
    store.set_draft_state(d.id, DraftState::Sending, None, d.send_at)?;
    emit(events, &d, DraftState::Sending, None);

    let files = match collect_files(s, store, &d, |_| true).await {
        Ok(Ok(f)) => f,
        Ok(Err(msg)) => return fail(store, events, op, &d, msg),
        Err(e) => {
            store.op_retry(op.id, now_ts() + backoff(op.attempts + 1, cfg).as_secs() as i64, &e.to_string())?;
            store.set_draft_state(d.id, DraftState::Queued, Some(&e.to_string()), d.send_at)?;
            emit(events, &d, DraftState::Queued, None);
            return Err(e);
        }
    };
    let reply = reply_source(store, &d);
    let built = match build_message(&sender(account), &d, reply.as_ref(), &files, None) {
        Ok(b) => b,
        Err(e) => return fail(store, events, op, &d, e.to_string()),
    };
    match crate::smtp::send_raw(&account.smtp, &account.email, password, &built.envelope_to, &account.email, &built.raw_for_storage).await {
        Ok(()) => {
            let dir = cfg.drafts_dir().join(d.id.to_string());
            std::fs::create_dir_all(&dir)?;
            std::fs::write(dir.join("sent.eml"), &built.raw_for_storage)?;
            store.op_done(op.id)?;
            store.enqueue_op_at(
                account.id,
                OpKind::SentCopy,
                serde_json::json!({"draftId": d.id, "messageId": built.message_id}),
                now_ts() + 15,
            )?;
            if reply.is_some() {
                if let Some((_, folder, uid)) = d.source_message_id.and_then(|m| store.message_location(m).ok()) {
                    store.enqueue_op(account.id, OpKind::SetAnswered, serde_json::json!({"folderId": folder, "uid": uid}))?;
                }
            }
            if let (Some(uid), Some((_, path))) = (d.remote_uid, store.folder_by_role(account.id, FolderRole::Drafts)?) {
                let r = async {
                    select(s, &path, false).await?;
                    delete_uid(s, uid, caps).await
                }
                .await;
                if let Err(e) = r {
                    tracing::warn!("copie serveur du brouillon non supprimée : {e}");
                }
                store.set_draft_remote_uid(d.id, None)?;
            }
            store.set_draft_state(d.id, DraftState::Sent, None, None)?;
            emit(events, &d, DraftState::Sent, None);
            Ok(())
        }
        Err(CoreError::Auth(m)) => {
            let msg = "Mot de passe refusé par le serveur d'envoi".to_string();
            store.set_account_status(account.id, AccountStatus::AuthError, Some(&m))?;
            events.emit(CoreEvent::AccountStatus { account_id: account.id, status: AccountStatus::AuthError, error: Some(m) });
            fail(store, events, op, &d, msg)
        }
        Err(CoreError::Smtp(m)) if op.attempts + 1 >= SMTP_TEMPORARY_RETRIES => {
            fail(store, events, op, &d, format!("Le serveur d'envoi refuse ce mail pour l'instant : {m}"))
        }
        Err(e) if transient(&e) || matches!(e, CoreError::Smtp(_)) => {
            // Serveur injoignable ou refus temporaire (4xx) : on réessaie plus tard (la session IMAP reste valable).
            store.op_retry(op.id, now_ts() + backoff(op.attempts + 1, cfg).as_secs() as i64, &e.to_string())?;
            store.set_draft_state(d.id, DraftState::Queued, Some(&e.to_string()), d.send_at)?;
            emit(events, &d, DraftState::Queued, Some(e.to_string()));
            Ok(())
        }
        Err(CoreError::Rejected(m)) => fail(store, events, op, &d, format!("Refusé par le serveur d'envoi : {m}")),
        Err(e) => fail(store, events, op, &d, e.to_string()),
    }
}

pub(crate) async fn exec_sent_copy(
    s: &mut ImapSession,
    caps: &Caps,
    store: &Store,
    account: &Account,
    cfg: &WorkerConfig,
    op: &Op,
) -> Result<()> {
    let id = draft_id(op);
    let message_id = op.payload["messageId"].as_str().unwrap_or("").to_string();
    let dir = cfg.drafts_dir().join(id.to_string());
    let raw = std::fs::read(dir.join("sent.eml")).ok();
    let sent = store.folder_by_role(account.id, FolderRole::Sent)?;
    if raw.is_some() && sent.is_none() && op.attempts < MISSING_FOLDER_RETRIES {
        return retry_until_folder_known(store, op);
    }
    if let (Some(raw), Some((_, path))) = (raw, sent) {
        let r = async {
            select(s, &path, false).await?;
            if search_message_id(s, caps, &message_id).await?.is_empty() {
                append(s, &path, "(\\Seen)", &raw).await?;
            }
            Ok(())
        }
        .await;
        match r {
            Ok(()) => {}
            // Réseau : on réessaiera, sauf après de nombreux essais (le mail est parti, seule la copie manque).
            Err(e) if transient(&e) && op.attempts + 1 < SENT_COPY_ATTEMPTS => return Err(e),
            Err(e) => tracing::warn!(draft = id, "copie dans Envoyés abandonnée : {e}"),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    let _ = store.delete_draft(id);
    store.op_done(op.id)
}

pub(crate) async fn exec_set_answered(s: &mut ImapSession, store: &Store, op: &Op) -> Result<()> {
    let folder = op.payload["folderId"].as_i64().unwrap_or(-1);
    let uid = op.payload["uid"].as_u64().unwrap_or(0) as u32;
    if let Ok(path) = store.folder_path_of(op.account_id, folder) {
        select(s, &path, false).await?;
        add_flag(s, uid, "\\Answered").await?;
    }
    store.op_done(op.id)
}

pub(crate) async fn exec_append_draft(
    s: &mut ImapSession,
    caps: &Caps,
    store: &Store,
    account: &Account,
    op: &Op,
) -> Result<()> {
    let d = match store.get_draft(draft_id(op)) {
        Ok(d) if matches!(d.state, DraftState::Editing | DraftState::Failed) => d,
        Ok(_) | Err(CoreError::NotFound) => return store.op_done(op.id),
        Err(e) => return Err(e),
    };
    let Some((_, path)) = store.folder_by_role(account.id, FolderRole::Drafts)? else {
        return retry_until_folder_known(store, op);
    };
    // Les pièces d'un autre mail (transfert) ne sont pas recopiées à chaque minute de rédaction ;
    // celles d'un mail du dossier Brouillons le sont, car l'ancienne copie va être supprimée.
    let drafts_folder = store.folder_by_role(account.id, FolderRole::Drafts)?.map(|(f, _)| f);
    let in_drafts = |m: MessageId| store.message_location(m).is_ok_and(|(_, f, _)| Some(f) == drafts_folder);
    let files = match collect_files(s, store, &d, in_drafts).await? {
        Ok(f) => f,
        Err(msg) => {
            // Ne jamais remplacer une copie serveur par une version qui perdrait des pièces jointes.
            tracing::warn!(draft = d.id, "brouillon non copié sur le serveur : {msg}");
            return store.op_done(op.id);
        }
    };
    let reply = reply_source(store, &d);
    let built = match build_message(&sender(account), &d, reply.as_ref(), &files, Some(d.id)) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(draft = d.id, "brouillon non copié sur le serveur : {e}");
            return store.op_done(op.id);
        }
    };
    append(s, &path, "(\\Draft \\Seen)", &built.raw_for_storage).await?;
    select(s, &path, false).await?;
    if let Some(old) = d.remote_uid {
        let _ = delete_uid(s, old, caps).await;
    }
    // Message-ID neuf à chaque copie : la recherche ne peut pas confondre deux brouillons.
    let uid = search_message_id(s, caps, &built.message_id).await?.into_iter().max();
    store.set_draft_remote_uid(d.id, uid)?;
    store.op_done(op.id)
}

pub(crate) async fn exec_delete_remote(s: &mut ImapSession, caps: &Caps, store: &Store, account: &Account, op: &Op) -> Result<()> {
    let folder = op.payload["folderId"].as_i64().unwrap_or(-1);
    let uid = op.payload["uid"].as_u64().unwrap_or(0) as u32;
    // Après « Vider le cache », les numéros de dossiers changent : on retrouve Brouillons par son rôle.
    let path = match store.folder_path_of(account.id, folder) {
        Ok(p) => Some(p),
        Err(_) if op.payload["role"] == "drafts" => store.folder_by_role(account.id, FolderRole::Drafts)?.map(|(_, p)| p),
        Err(_) => None,
    };
    if let Some(path) = path {
        select(s, &path, false).await?;
        delete_uid(s, uid, caps).await?;
    }
    store.op_done(op.id)
}
