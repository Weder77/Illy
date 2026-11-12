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

/// Signaux de la connexion IDLE vers le worker.
enum IdleMsg {
    Changed,
    AuthFailed(String),
}

/// Une session plus courte que ça ne remet pas le délai de reprise à zéro.
const HEALTHY_SESSION: Duration = Duration::from_secs(120);

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
        if !matches!(st.last_status, Some((AccountStatus::Offline, _))) {
            ctx.set_status(&mut st, AccountStatus::Syncing, None);
        }
        let started = Instant::now();
        let result = match connect(&account.imap, &account.email, &password).await {
            Ok((session, caps)) => connected(&ctx, &mut st, &mut rx, session, caps, &account, &password).await,
            Err(e) => Err(e),
        };
        // Délai de reprise : remis à zéro seulement après une session restée saine assez longtemps.
        attempt = if started.elapsed() >= HEALTHY_SESSION { 1 } else { attempt + 1 };
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
                // Réponse illisible : la session est inutilisable ; on se reconnecte
                // (le dossier fautif est passé en mode sûr par `connected`).
                tracing::warn!(account = ctx.account, attempt, "réponse IMAP illisible, reconnexion : {msg}");
                if !wait_refusing(&mut rx, backoff(attempt, &ctx.cfg), &CoreError::Parse(msg)).await {
                    return;
                }
            }
            Err(e) => {
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
async fn idle_loop(account: Account, password: String, cfg: WorkerConfig, notify: mpsc::Sender<IdleMsg>) {
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
                        if notify.send(IdleMsg::Changed).await.is_err() {
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
        if let Err(CoreError::Auth(m)) = r {
            // Mot de passe changé : on prévient le worker au lieu de réessayer en boucle.
            let _ = notify.send(IdleMsg::AuthFailed(m)).await;
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

/// Traite toutes les demandes en attente : ouvertures de mails d'abord, synchros de dossier dédoublonnées.
async fn drain_requests(
    ctx: &WorkerCtx,
    st: &mut State,
    rx: &mut mpsc::Receiver<WorkerRequest>,
    s: &mut ImapSession,
    caps: &Caps,
    first: Option<WorkerRequest>,
) -> Result<()> {
    let mut pending: Vec<WorkerRequest> = first.into_iter().collect();
    while let Ok(req) = rx.try_recv() {
        pending.push(req);
    }
    let (urgent, rest): (Vec<_>, Vec<_>) = pending
        .into_iter()
        .partition(|r| matches!(r, WorkerRequest::FetchBody(..) | WorkerRequest::FetchAttachment(..)));
    for req in urgent {
        handle_request(ctx, st, s, caps, req).await?;
    }
    let mut seen_folders = HashSet::new();
    for req in rest {
        if let WorkerRequest::SyncFolder(f) = &req {
            if !seen_folders.insert(*f) {
                continue;
            }
        }
        handle_request(ctx, st, s, caps, req).await?;
    }
    Ok(())
}

/// Télécharge un lot de corps en arrière-plan, en laissant passer les demandes de l'utilisateur.
async fn fetch_bodies(
    ctx: &WorkerCtx,
    st: &mut State,
    rx: &mut mpsc::Receiver<WorkerRequest>,
    s: &mut ImapSession,
    caps: &Caps,
    ids: Vec<MessageId>,
) -> Result<()> {
    for id in ids.into_iter().take(20) {
        drain_requests(ctx, st, rx, s, caps, None).await?;
        match timed(fetch_body(s, &ctx.store, ctx.events.as_ref(), id)).await {
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
    Ok(())
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
    // Dossiers d'abord : les actions en attente (envoi, brouillons) ont besoin d'Envoyés et de Brouillons.
    let folders = timed(list_folders(&mut s)).await?;
    ctx.store.upsert_folders(ctx.account, &folders)?;
    ctx.events.emit(CoreEvent::FoldersChanged { account_id: ctx.account });

    // Actions faites hors ligne : envoyées avant toute relecture des drapeaux du serveur.
    ctx.store.ops_due_now(ctx.account)?;
    timed(run_due_ops(&mut s, &caps, &ctx.store, ctx.events.as_ref(), account, password, &ctx.cfg)).await?;
    let all = ctx.store.syncable_folders(ctx.account)?;
    let inbox = all.iter().find(|f| f.2 == FolderRole::Inbox).map(|f| (f.0, f.1.clone()));
    // Les dossiers disparus (renommés, supprimés ailleurs) ne gardent ni historique en attente ni mode sûr.
    let live: HashSet<FolderId> = all.iter().map(|f| f.0).collect();
    st.backlogs.retain(|id, _| live.contains(id));
    st.safe_mode.retain(|id| live.contains(id));

    // Premier tour : boîte de réception d'abord, puis les autres.
    for (id, path, _) in &all {
        drain_requests(ctx, st, rx, &mut s, &caps, None).await?;
        sync_one(ctx, st, &mut s, &caps, *id, path).await?;
    }
    ctx.set_status(st, AccountStatus::Ok, None);

    // IDLE (ou sondage si le serveur ne le gère pas).
    let (idle_tx, mut idle_rx) = mpsc::channel::<IdleMsg>(8);
    let _idle = caps
        .idle
        .then(|| AbortOnDrop(tokio::spawn(idle_loop(account.clone(), password.to_string(), ctx.cfg.clone(), idle_tx))));

    let mut inbox_dirty = false;
    let mut next_poll = Instant::now() + ctx.cfg.poll_interval;
    let mut next_other = Instant::now() + ctx.cfg.other_folders_interval;
    loop {
        // 1. Demandes de l'utilisateur.
        drain_requests(ctx, st, rx, &mut s, &caps, None).await?;
        // 2. Actions en attente.
        timed(run_due_ops(&mut s, &caps, &ctx.store, ctx.events.as_ref(), account, password, &ctx.cfg)).await?;
        // 3. Signaux et minuteries.
        while let Ok(msg) = idle_rx.try_recv() {
            match msg {
                IdleMsg::Changed => inbox_dirty = true,
                IdleMsg::AuthFailed(m) => return Err(CoreError::Auth(m)),
            }
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
        // 4. Corps récents (lecture hors ligne et extraits), avant l'historique.
        let since = now_ts() - ctx.cfg.recent_body_days * 86_400;
        let extra = st.failed_bodies.len() as u32;
        let mut recent: Vec<MessageId> = ctx.store.recent_without_body(ctx.account, since, 20 + extra)?;
        recent.retain(|id| !st.failed_bodies.contains(id));
        if !recent.is_empty() {
            fetch_bodies(ctx, st, rx, &mut s, &caps, recent).await?;
            continue;
        }
        // 5. Historique des en-têtes (boîte de réception d'abord).
        let pending = inbox
            .as_ref()
            .map(|i| i.0)
            .filter(|id| st.backlogs.get(id).is_some_and(|b| !b.pending_desc.is_empty()))
            .or_else(|| st.backlogs.iter().find(|(_, b)| !b.pending_desc.is_empty()).map(|(id, _)| *id));
        if let Some(folder) = pending {
            let structure = !st.safe_mode.contains(&folder);
            let mut backlog = st.backlogs.remove(&folder).unwrap_or_default();
            let r = match ctx.store.folder_path(folder) {
                Ok(path) => {
                    timed(backfill_step(&mut s, &ctx.store, ctx.events.as_ref(), ctx.account, &path, &mut backlog, &ctx.cfg, structure))
                        .await
                }
                Err(e) => Err(e),
            };
            match r {
                Ok(()) => {
                    st.safe_mode.remove(&folder);
                    st.backlogs.insert(folder, backlog);
                }
                Err(CoreError::Parse(m)) => {
                    st.safe_mode.insert(folder);
                    st.backlogs.insert(folder, backlog);
                    return Err(CoreError::Parse(m));
                }
                Err(e) if is_fatal(&e) => {
                    st.backlogs.insert(folder, backlog);
                    return Err(e);
                }
                Err(e) => {
                    // Dossier disparu ou refus du serveur : on abandonne son historique pour cette session.
                    tracing::warn!(folder, "historique du dossier abandonné : {e}");
                }
            }
            continue;
        }
        // 6. Corps plus anciens.
        let mut old: Vec<MessageId> = ctx.store.any_without_body(ctx.account, 20 + extra)?;
        old.retain(|id| !st.failed_bodies.contains(id));
        if !old.is_empty() {
            fetch_bodies(ctx, st, rx, &mut s, &caps, old).await?;
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
                Some(req) => drain_requests(ctx, st, rx, &mut s, &caps, Some(req)).await?,
                None => {
                    let _ = s.logout().await;
                    return Ok(Exit::Shutdown);
                }
            },
            Some(msg) = idle_rx.recv() => match msg {
                IdleMsg::Changed => inbox_dirty = true,
                IdleMsg::AuthFailed(m) => return Err(CoreError::Auth(m)),
            },
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(wake)) => {}
        }
    }
}
