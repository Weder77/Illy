//! Worker d'un compte : connexion, synchro, IDLE, priorités, file d'actions.

use super::{backfill_step, backoff, fetch_attachment, fetch_body, run_due_ops, sync_folder, FolderBacklog, WorkerConfig};
use crate::error::{CoreError, Result};
use crate::events::{CoreEvent, EventSink};
use crate::imap::{connect, imap_err, list_folders, select, Caps, ImapSession};
use crate::model::{Account, AccountId, AccountStatus, FolderId, FolderRole, MessageId};
use crate::secrets::SecretStore;
use crate::store::{now_ts, Store};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

/// Demandes adressées au worker d'un compte (traitées avant tout travail de fond).
pub enum WorkerRequest {
    SyncFolder(FolderId),
    FetchBody(MessageId, oneshot::Sender<Result<()>>),
    FetchAttachment(MessageId, String, oneshot::Sender<Result<Vec<u8>>>),
    OpsAvailable,
}

/// Délai maximal d'une opération IMAP avant de considérer la connexion comme perdue.
const OP_TIMEOUT: Duration = Duration::from_secs(300);

pub(crate) struct WorkerCtx {
    pub account: AccountId,
    pub store: Store,
    pub secrets: Arc<dyn SecretStore>,
    pub events: Arc<dyn EventSink>,
    pub cfg: WorkerConfig,
}

/// État conservé d'une connexion à l'autre.
#[derive(Default)]
struct State {
    backlogs: HashMap<FolderId, FolderBacklog>,
    safe_mode: HashSet<FolderId>,
    failed_bodies: HashSet<MessageId>,
    last_status: Option<(AccountStatus, Option<String>)>,
}

enum Exit {
    Shutdown,
}

async fn timed<T>(f: impl Future<Output = Result<T>>) -> Result<T> {
    match tokio::time::timeout(OP_TIMEOUT, f).await {
        Ok(r) => r,
        Err(_) => Err(CoreError::Network("délai dépassé".into())),
    }
}

fn is_fatal(e: &CoreError) -> bool {
    matches!(e, CoreError::Network(_) | CoreError::Parse(_) | CoreError::Tls(_) | CoreError::Auth(_))
}

impl WorkerCtx {
    fn set_status(&self, st: &mut State, status: AccountStatus, error: Option<String>) {
        let next = (status, error.clone());
        if st.last_status.as_ref() == Some(&next) {
            return;
        }
        st.last_status = Some(next);
        if let Err(e) = self.store.set_account_status(self.account, status, error.as_deref()) {
            tracing::error!("statut du compte non enregistré : {e}");
        }
        self.events.emit(CoreEvent::AccountStatus { account_id: self.account, status, error });
    }
}

/// Répond aux demandes avec une erreur tant que le compte n'est pas connecté.
fn refuse(req: WorkerRequest, err: &CoreError) {
    let e = || match err {
        CoreError::Auth(m) => CoreError::Auth(m.clone()),
        other => CoreError::Network(other.to_string()),
    };
    match req {
        WorkerRequest::FetchBody(_, tx) => {
            let _ = tx.send(Err(e()));
        }
        WorkerRequest::FetchAttachment(_, _, tx) => {
            let _ = tx.send(Err(e()));
        }
        WorkerRequest::SyncFolder(_) | WorkerRequest::OpsAvailable => {}
    }
}

/// Attend `dur` en refusant les demandes ; `false` si le canal est fermé (arrêt).
async fn wait_refusing(rx: &mut mpsc::Receiver<WorkerRequest>, dur: Duration, err: &CoreError) -> bool {
    let deadline = tokio::time::Instant::now() + dur;
    loop {
        tokio::select! {
            r = rx.recv() => match r {
                Some(req) => refuse(req, err),
                None => return false,
            },
            _ = tokio::time::sleep_until(deadline) => return true,
        }
    }
}

pub(crate) async fn run_worker(ctx: WorkerCtx, mut rx: mpsc::Receiver<WorkerRequest>) {
    let mut st = State::default();
    let mut attempt: u32 = 0;
    loop {
        let account = match ctx.store.get_account(ctx.account) {
            Ok(a) => a,
            Err(_) => return,
        };
        let password = match ctx.secrets.get(ctx.account) {
            Ok(Some(p)) => p,
            Ok(None) => {
                let msg = "Mot de passe manquant".to_string();
                ctx.set_status(&mut st, AccountStatus::AuthError, Some(msg.clone()));
                while let Some(req) = rx.recv().await {
                    refuse(req, &CoreError::Auth(msg.clone()));
                }
                return;
            }
            Err(e) => {
                ctx.set_status(&mut st, AccountStatus::Error, Some(e.to_string()));
                while let Some(req) = rx.recv().await {
                    refuse(req, &e);
                }
                return;
            }
        };
        ctx.set_status(&mut st, AccountStatus::Syncing, None);
        let result = match connect(&account.imap, &account.email, &password).await {
            Ok((session, caps)) => {
                attempt = 0;
                connected(&ctx, &mut st, &mut rx, session, caps, &account, &password).await
            }
            Err(e) => Err(e),
        };
        match result {
            Ok(Exit::Shutdown) => return,
            Err(CoreError::Auth(msg)) => {
                tracing::warn!(account = ctx.account, "mot de passe refusé");
                ctx.set_status(&mut st, AccountStatus::AuthError, Some(msg.clone()));
                while let Some(req) = rx.recv().await {
                    refuse(req, &CoreError::Auth(msg.clone()));
                }
                return;
            }
            Err(CoreError::Parse(msg)) => {
                // Réponse illisible : la session est inutilisable, on se reconnecte tout de suite
                // (le dossier fautif est passé en mode sûr par `connected`).
                tracing::warn!(account = ctx.account, "réponse IMAP illisible, reconnexion : {msg}");
                attempt += 1;
                if attempt > 20 && !wait_refusing(&mut rx, backoff(attempt, &ctx.cfg), &CoreError::Parse(msg)).await {
                    return;
                }
            }
            Err(e) => {
                attempt += 1;
                tracing::info!(account = ctx.account, attempt, "hors ligne : {e}");
                ctx.set_status(&mut st, AccountStatus::Offline, Some(e.to_string()));
                if !wait_refusing(&mut rx, backoff(attempt, &ctx.cfg), &e).await {
                    return;
                }
            }
        }
    }
}

/// Connexion IDLE séparée sur la boîte de réception ; signale chaque changement.
async fn idle_loop(account: Account, password: String, cfg: WorkerConfig, notify: mpsc::Sender<()>) {
    let mut attempt = 0;
    loop {
        let r: Result<()> = async {
            let (mut session, _) = connect(&account.imap, &account.email, &password).await?;
            select(&mut session, "INBOX", false).await?;
            attempt = 0;
            loop {
                let mut handle = session.idle();
                handle.init().await.map_err(imap_err)?;
                let res = {
                    let (fut, _stop) = handle.wait_with_timeout(cfg.idle_renew);
                    fut.await
                };
                session = handle.done().await.map_err(imap_err)?;
                match res {
                    Ok(async_imap::extensions::idle::IdleResponse::NewData(_)) => {
                        if notify.send(()).await.is_err() {
                            return Ok(());
                        }
                    }
                    Ok(_) => {}
                    Err(e) => return Err(imap_err(e)),
                }
            }
        }
        .await;
        if notify.is_closed() {
            return;
        }
        attempt += 1;
        tracing::debug!("IDLE interrompu ({r:?}), nouvelle tentative");
        tokio::time::sleep(backoff(attempt, &cfg)).await;
    }
}

struct AbortOnDrop(tokio::task::JoinHandle<()>);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[allow(clippy::too_many_arguments)]
async fn sync_one(
    ctx: &WorkerCtx,
    st: &mut State,
    s: &mut ImapSession,
    caps: &Caps,
    folder: FolderId,
    path: &str,
) -> Result<()> {
    let structure = !st.safe_mode.contains(&folder);
    let r = timed(sync_folder(s, caps, &ctx.store, ctx.events.as_ref(), ctx.account, folder, path, &ctx.cfg, structure)).await;
    match r {
        Ok(backlog) => {
            st.safe_mode.remove(&folder);
            st.backlogs.insert(folder, backlog);
            Ok(())
        }
        Err(CoreError::Parse(m)) => {
            st.safe_mode.insert(folder);
            Err(CoreError::Parse(m))
        }
        Err(e) if is_fatal(&e) => Err(e),
        Err(e) => {
            tracing::warn!(folder, "dossier ignoré pour ce tour : {e}");
            Ok(())
        }
    }
}

async fn handle_request(ctx: &WorkerCtx, st: &mut State, s: &mut ImapSession, caps: &Caps, req: WorkerRequest) -> Result<()> {
    match req {
        WorkerRequest::FetchBody(id, tx) => {
            let r = timed(fetch_body(s, &ctx.store, ctx.events.as_ref(), id)).await;
            let fatal = r.as_ref().err().filter(|e| is_fatal(e)).map(|e| e.to_string());
            if let Err(CoreError::Parse(_)) = &r {
                st.failed_bodies.insert(id);
            }
            let _ = tx.send(r);
            match fatal {
                Some(m) => Err(CoreError::Network(m)),
                None => Ok(()),
            }
        }
        WorkerRequest::FetchAttachment(id, part, tx) => {
            let r = timed(fetch_attachment(s, &ctx.store, id, &part)).await;
            let fatal = r.as_ref().err().filter(|e| is_fatal(e)).map(|e| e.to_string());
            let _ = tx.send(r);
            match fatal {
                Some(m) => Err(CoreError::Network(m)),
                None => Ok(()),
            }
        }
        WorkerRequest::SyncFolder(folder) => {
            let path = match ctx.store.folder_path(folder) {
                Ok(p) => p,
                Err(_) => return Ok(()),
            };
            sync_one(ctx, st, s, caps, folder, &path).await
        }
        WorkerRequest::OpsAvailable => Ok(()),
    }
}

async fn connected(
    ctx: &WorkerCtx,
    st: &mut State,
    rx: &mut mpsc::Receiver<WorkerRequest>,
    mut s: ImapSession,
    caps: Caps,
    account: &Account,
    password: &str,
) -> Result<Exit> {
    // Dossiers.
    let folders = timed(list_folders(&mut s)).await?;
    ctx.store.upsert_folders(ctx.account, &folders)?;
    ctx.events.emit(CoreEvent::FoldersChanged { account_id: ctx.account });
    let all = ctx.store.syncable_folders(ctx.account)?;
    let inbox = all.iter().find(|f| f.2 == FolderRole::Inbox).map(|f| (f.0, f.1.clone()));

    // Premier tour : boîte de réception d'abord, puis les autres.
    for (id, path, _) in &all {
        if let Ok(req) = rx.try_recv() {
            handle_request(ctx, st, &mut s, &caps, req).await?;
        }
        sync_one(ctx, st, &mut s, &caps, *id, path).await?;
    }
    ctx.set_status(st, AccountStatus::Ok, None);
    ctx.store.ops_due_now(ctx.account)?;

    // IDLE (ou sondage si le serveur ne le gère pas).
    let (idle_tx, mut idle_rx) = mpsc::channel::<()>(8);
    let _idle = caps
        .idle
        .then(|| AbortOnDrop(tokio::spawn(idle_loop(account.clone(), password.to_string(), ctx.cfg.clone(), idle_tx))));

    let mut inbox_dirty = false;
    let mut next_poll = Instant::now() + ctx.cfg.poll_interval;
    let mut next_other = Instant::now() + ctx.cfg.other_folders_interval;
    loop {
        // 1. Demandes de l'utilisateur.
        while let Ok(req) = rx.try_recv() {
            handle_request(ctx, st, &mut s, &caps, req).await?;
        }
        // 2. Actions en attente.
        timed(run_due_ops(&mut s, &ctx.store, ctx.events.as_ref(), ctx.account, &ctx.cfg)).await?;
        // 3. Signaux et minuteries.
        while idle_rx.try_recv().is_ok() {
            inbox_dirty = true;
        }
        let now = Instant::now();
        if !caps.idle && now >= next_poll {
            inbox_dirty = true;
            next_poll = now + ctx.cfg.poll_interval;
        }
        if inbox_dirty {
            inbox_dirty = false;
            if let Some((id, path)) = &inbox {
                sync_one(ctx, st, &mut s, &caps, *id, path).await?;
            }
            continue;
        }
        if now >= next_other {
            next_other = now + ctx.cfg.other_folders_interval;
            for (id, path, role) in ctx.store.syncable_folders(ctx.account)? {
                if role != FolderRole::Inbox {
                    sync_one(ctx, st, &mut s, &caps, id, &path).await?;
                }
            }
            continue;
        }
        // 4. Historique des en-têtes (boîte de réception d'abord).
        let pending = inbox
            .as_ref()
            .map(|i| i.0)
            .filter(|id| st.backlogs.get(id).is_some_and(|b| !b.pending_desc.is_empty()))
            .or_else(|| st.backlogs.iter().find(|(_, b)| !b.pending_desc.is_empty()).map(|(id, _)| *id));
        if let Some(folder) = pending {
            let path = ctx.store.folder_path(folder)?;
            let structure = !st.safe_mode.contains(&folder);
            let mut backlog = st.backlogs.remove(&folder).unwrap_or_default();
            let r = timed(backfill_step(&mut s, &ctx.store, ctx.events.as_ref(), ctx.account, &path, &mut backlog, &ctx.cfg, structure)).await;
            st.backlogs.insert(folder, backlog);
            match r {
                Ok(()) => {
                    st.safe_mode.remove(&folder);
                }
                Err(CoreError::Parse(m)) => {
                    st.safe_mode.insert(folder);
                    return Err(CoreError::Parse(m));
                }
                Err(e) => return Err(e),
            }
            continue;
        }
        // 5. Corps : récents d'abord, puis le reste.
        let since = now_ts() - ctx.cfg.recent_body_days * 86_400;
        let extra = st.failed_bodies.len() as u32;
        let mut ids: Vec<MessageId> = ctx.store.recent_without_body(ctx.account, since, 20 + extra)?;
        if ids.iter().all(|id| st.failed_bodies.contains(id)) {
            ids = ctx.store.any_without_body(ctx.account, 20 + extra)?;
        }
        ids.retain(|id| !st.failed_bodies.contains(id));
        if !ids.is_empty() {
            for id in ids.into_iter().take(20) {
                if let Ok(req) = rx.try_recv() {
                    handle_request(ctx, st, &mut s, &caps, req).await?;
                }
                match timed(fetch_body(&mut s, &ctx.store, ctx.events.as_ref(), id)).await {
                    Ok(()) | Err(CoreError::NotFound) => {}
                    Err(CoreError::Parse(m)) => {
                        st.failed_bodies.insert(id);
                        return Err(CoreError::Parse(m));
                    }
                    Err(e) if is_fatal(&e) => return Err(e),
                    Err(e) => {
                        tracing::warn!(message = id, "corps non récupéré : {e}");
                        st.failed_bodies.insert(id);
                    }
                }
            }
            continue;
        }
        // 6. Rien à faire : on attend un signal.
        let mut wake = next_other;
        if !caps.idle {
            wake = wake.min(next_poll);
        }
        if let Some(t) = ctx.store.next_op_at(ctx.account)? {
            let secs = (t - now_ts()).max(0) as u64;
            wake = wake.min(Instant::now() + Duration::from_secs(secs));
        }
        tokio::select! {
            r = rx.recv() => match r {
                Some(req) => handle_request(ctx, st, &mut s, &caps, req).await?,
                None => {
                    let _ = s.logout().await;
                    return Ok(Exit::Shutdown);
                }
            },
            Some(()) = idle_rx.recv() => inbox_dirty = true,
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(wake)) => {}
        }
    }
}
