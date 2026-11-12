//! Façade utilisée par l'application (commandes Tauri).

use crate::error::{CoreError, Result};
use crate::events::{CoreEvent, EventSink};
use crate::imap::connect;
use crate::model::*;
use crate::providers::detect_provider;
use crate::secrets::SecretStore;
use crate::smtp::test_smtp;
use crate::store::{OpKind, Store};
use crate::sync::{SyncManager, WorkerConfig, WorkerRequest};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;

const OPEN_TIMEOUT: Duration = Duration::from_secs(30);

/// Délai d'attente d'une pièce jointe : 30 s plus le temps de la télécharger à 50 Ko/s, au plus 10 min.
pub fn attachment_timeout(size: u64) -> Duration {
    Duration::from_secs((30 + size / 50_000).min(600))
}

/// Au-delà, le brouillon n'est copié sur le serveur qu'à la fermeture de l'éditeur.
const PERIODIC_UPLOAD_MAX_BYTES: u64 = 2 * 1024 * 1024;

/// Taille maximale d'un fichier joint (il est lu en entier pour construire le message).
const MAX_ATTACHMENT_BYTES: u64 = 100 * 1024 * 1024;

/// `n-nom` dans `dir`, avec le premier `n` libre : deux pièces jointes n'ont jamais le même fichier.
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    (1u32..)
        .map(|n| dir.join(format!("{n}-{name}")))
        .find(|p| !p.exists())
        .expect("un nom libre existe")
}

pub struct Core {
    store: Store,
    secrets: Arc<dyn SecretStore>,
    events: Arc<dyn EventSink>,
    manager: SyncManager,
    tmp_dir: PathBuf,
    drafts_dir: PathBuf,
    /// Dernière copie serveur de chaque brouillon (au plus une par minute pendant la rédaction).
    last_upload: std::sync::Mutex<std::collections::HashMap<DraftId, std::time::Instant>>,
}

const DRAFT_UPLOAD_EVERY: Duration = Duration::from_secs(60);

/// Nom de fichier sûr pour l'écrire sur le disque (voir `providers::safe_filename`).
pub fn sanitize_filename(name: &str) -> String {
    crate::providers::safe_filename(name)
}

/// Marque le fichier comme venant d'Internet (Windows : SmartScreen, mode protégé d'Office).
#[cfg(windows)]
fn mark_of_the_web(path: &Path) {
    let mut ads = path.as_os_str().to_owned();
    ads.push(":Zone.Identifier");
    if let Err(e) = std::fs::write(PathBuf::from(ads), b"[ZoneTransfer]\r\nZoneId=3\r\n") {
        tracing::warn!("marque « fichier Internet » non posée : {e}");
    }
}

#[cfg(not(windows))]
fn mark_of_the_web(_path: &Path) {}

fn validate(input: &NewAccount) -> Result<NewAccount> {
    let email = input.email.trim().to_string();
    let (local, domain) = email.split_once('@').unwrap_or(("", ""));
    if local.is_empty() || domain.is_empty() || email.contains(char::is_whitespace) {
        return Err(CoreError::InvalidInput("adresse mail invalide".into()));
    }
    for s in [&input.imap, &input.smtp] {
        if s.host.trim().is_empty() || s.port == 0 {
            return Err(CoreError::InvalidInput("serveur ou port manquant".into()));
        }
        if s.tls == TlsMode::Plain && !cfg!(feature = "insecure-plain") {
            return Err(CoreError::InvalidInput("connexion non chiffrée interdite".into()));
        }
    }
    let display_name = match input.display_name.trim() {
        "" => local.to_string(),
        n => n.to_string(),
    };
    Ok(NewAccount { email, display_name, ..input.clone() })
}

impl Core {
    /// Ouvre le cœur sur le dossier de données (`illy.db`, `tmp\`, `drafts\`).
    pub fn open(
        data_dir: &Path,
        secrets: Arc<dyn SecretStore>,
        events: Arc<dyn EventSink>,
        mut cfg: WorkerConfig,
        runtime: tokio::runtime::Handle,
    ) -> Result<Core> {
        let tmp_dir = data_dir.join("tmp");
        if tmp_dir.exists() {
            let _ = std::fs::remove_dir_all(&tmp_dir);
        }
        std::fs::create_dir_all(&tmp_dir)?;
        cfg.data_dir = Some(data_dir.to_path_buf());
        let drafts_dir = cfg.drafts_dir();
        std::fs::create_dir_all(&drafts_dir)?;
        let store = Store::open(&data_dir.join("illy.db"))?;
        let manager = SyncManager::new(store.clone(), secrets.clone(), events.clone(), cfg, runtime);
        for a in store.list_accounts()? {
            manager.start(a.id);
        }
        Ok(Core { store, secrets, events, manager, tmp_dir, drafts_dir, last_upload: Default::default() })
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn sync_manager(&self) -> &SyncManager {
        &self.manager
    }

    pub fn detect_provider(&self, email: &str) -> ProviderGuess {
        detect_provider(email)
    }

    /// Teste IMAP et SMTP, puis enregistre le compte. Rien n'est enregistré en cas d'échec.
    pub async fn add_account(&self, input: NewAccount, password: String) -> Result<Account> {
        let input = validate(&input)?;
        let (mut session, _) = connect(&input.imap, &input.email, &password).await?;
        let _ = session.logout().await;
        test_smtp(&input.smtp, &input.email, &password).await?;
        let account = self.store.insert_account(&input)?;
        if let Err(e) = self.secrets.set(account.id, &password) {
            let _ = self.store.delete_account(account.id);
            return Err(e);
        }
        self.manager.start(account.id);
        Ok(account)
    }

    pub async fn update_password(&self, account: AccountId, password: String) -> Result<()> {
        let acc = self.store.get_account(account)?;
        let (mut session, _) = connect(&acc.imap, &acc.email, &password).await?;
        let _ = session.logout().await;
        self.secrets.set(account, &password)?;
        self.manager.restart(account);
        Ok(())
    }

    /// Supprime le compte de ce PC : mails, dossiers, brouillons (et leurs fichiers), actions en attente.
    /// Rien n'est effacé sur le serveur.
    pub fn remove_account(&self, account: AccountId) -> Result<()> {
        self.manager.stop(account);
        let drafts = self.store.drafts_of_account(account)?;
        self.secrets.delete(account)?;
        self.store.delete_account(account)?;
        for d in drafts {
            self.last_upload.lock().unwrap().remove(&d.id);
            let _ = std::fs::remove_dir_all(self.drafts_dir.join(d.id.to_string()));
            if matches!(d.state, DraftState::Queued | DraftState::Sending | DraftState::Failed) {
                self.events.emit(CoreEvent::OutboxChanged { draft_id: d.id, state: DraftState::Editing, send_at: None, error: None });
            }
        }
        Ok(())
    }

    pub fn reorder_accounts(&self, ids: Vec<AccountId>) -> Result<Vec<Account>> {
        self.store.reorder_accounts(&ids)
    }

    /// Nom affiché, couleur et signature (nettoyée comme un mail ; les images collées sont gardées).
    pub fn update_account(&self, id: AccountId, patch: AccountPatch) -> Result<Account> {
        let display_name = match patch.display_name {
            Some(n) if n.trim().is_empty() => return Err(CoreError::InvalidInput("Le nom affiché ne peut pas être vide.".into())),
            Some(n) => Some(n.trim().to_string()),
            None => None,
        };
        let valid_color = |c: &str| c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit());
        if let Some(c) = &patch.color {
            if !valid_color(c) {
                return Err(CoreError::InvalidInput(format!("couleur invalide : {c}")));
            }
        }
        let signature_html = patch
            .signature_html
            .map(|h| crate::mime::sanitize_html(&h, &std::collections::HashMap::new()).html);
        self.store.update_account(id, &AccountPatch { display_name, color: patch.color, signature_html })
    }

    /// Nouveaux serveurs (OVH, comptes manuels) : testés avec le mot de passe enregistré avant d'être gardés.
    pub async fn update_servers(&self, id: AccountId, imap: ServerConfig, smtp: ServerConfig) -> Result<Account> {
        let acc = self.store.get_account(id)?;
        if matches!(acc.provider, Provider::Gmail | Provider::Yahoo) {
            return Err(CoreError::InvalidInput("Les serveurs de ce fournisseur ne se modifient pas.".into()));
        }
        let candidate = validate(&NewAccount {
            email: acc.email.clone(),
            display_name: acc.display_name.clone(),
            provider: acc.provider,
            imap,
            smtp,
        })?;
        let password = self.secrets.get(id)?.ok_or_else(|| CoreError::Auth("mot de passe absent du trousseau".into()))?;
        let (mut session, _) = connect(&candidate.imap, &acc.email, &password).await?;
        let _ = session.logout().await;
        test_smtp(&candidate.smtp, &acc.email, &password).await?;
        let updated = self.store.update_servers(id, &candidate.imap, &candidate.smtp)?;
        self.manager.restart(id);
        Ok(updated)
    }

    pub fn list_accounts(&self) -> Result<Vec<Account>> {
        self.store.list_accounts()
    }

    pub fn list_folders(&self, account: AccountId) -> Result<Vec<Folder>> {
        self.store.list_folders(account)
    }

    pub fn list_messages(&self, scope: ListScope, offset: u32, limit: u32) -> Result<Vec<MessageSummary>> {
        self.store.list_messages(scope, offset, limit.min(500))
    }

    pub fn count_messages(&self, scope: ListScope) -> Result<u32> {
        self.store.count_messages(scope)
    }

    /// Demande une synchro immédiate du dossier affiché.
    pub async fn select_folder(&self, folder: FolderId) -> Result<()> {
        let account = self.store.folder_account(folder)?;
        match self.manager.send(account, WorkerRequest::SyncFolder(folder)).await {
            Ok(()) | Err(CoreError::NotFound) => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn ask_worker<T>(
        &self,
        account: AccountId,
        timeout: Duration,
        make: impl FnOnce(oneshot::Sender<Result<T>>) -> WorkerRequest,
    ) -> Result<T> {
        let (tx, rx) = oneshot::channel();
        self.manager
            .send(account, make(tx))
            .await
            .map_err(|_| CoreError::Network("ce compte n'est pas connecté".into()))?;
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err(CoreError::Network("connexion interrompue".into())),
            Err(_) => Err(CoreError::Network("délai dépassé".into())),
        }
    }

    /// Ouvre un mail : télécharge le corps si besoin et le marque lu (localement puis sur le serveur).
    pub async fn open_message(&self, id: MessageId, show_images: bool) -> Result<MessageDetail> {
        let (account, folder, uid) = self.store.message_location(id)?;
        let mut detail = match self.store.message_detail(id, show_images)? {
            Some(d) => d,
            None => {
                self.ask_worker(account, OPEN_TIMEOUT, |tx| WorkerRequest::FetchBody(id, tx)).await?;
                self.store.message_detail(id, show_images)?.ok_or(CoreError::NotFound)?
            }
        };
        if self.store.set_seen_local(id, true)? {
            self.store.enqueue_op(account, OpKind::SetSeen, serde_json::json!({"folderId": folder, "uid": uid, "seen": true}))?;
            let _ = self.manager.send(account, WorkerRequest::OpsAvailable).await;
            self.events.emit(CoreEvent::FolderChanged { account_id: account, folder_id: folder });
            detail.summary.seen = true;
        }
        Ok(detail)
    }

    pub fn allow_images_from(&self, sender: &str) -> Result<()> {
        self.store.allow_images_from(sender)
    }

    async fn attachment_bytes(&self, id: MessageId, part_path: &str) -> Result<(Attachment, Vec<u8>)> {
        let (account, _, _) = self.store.message_location(id)?;
        let detail = self.store.message_detail(id, false)?.ok_or(CoreError::NotFound)?;
        let att = detail.attachments.into_iter().find(|a| a.part_path == part_path).ok_or(CoreError::NotFound)?;
        let bytes = match self.store.part(id, part_path)?.1 {
            Some(b) => b,
            None => {
                let part = part_path.to_string();
                self.ask_worker(account, attachment_timeout(att.size), |tx| WorkerRequest::FetchAttachment(id, part, tx)).await?
            }
        };
        Ok((att, bytes))
    }

    /// Écrit la pièce jointe dans le dossier temporaire pour l'ouvrir avec l'application Windows associée.
    pub async fn attachment_to_tmp(&self, id: MessageId, part_path: &str) -> Result<PathBuf> {
        let (att, bytes) = self.attachment_bytes(id, part_path).await?;
        if !att.can_open {
            return Err(CoreError::InvalidInput(
                "ce type de fichier ne s'ouvre pas directement : utilise « Enregistrer sous… »".into(),
            ));
        }
        let path = self.tmp_dir.join(format!("{id}-{}", sanitize_filename(&att.filename)));
        tokio::fs::write(&path, bytes).await?;
        mark_of_the_web(&path);
        Ok(path)
    }

    pub async fn save_attachment(&self, id: MessageId, part_path: &str, dest: &Path) -> Result<()> {
        let (_, bytes) = self.attachment_bytes(id, part_path).await?;
        tokio::fs::write(dest, bytes).await?;
        mark_of_the_web(dest);
        Ok(())
    }

    // ---- Rédaction ----------------------------------------------------------------------------

    pub fn search_contacts(&self, query: &str) -> Result<Vec<Address>> {
        self.store.search_contacts(query, 8)
    }

    /// HTML d'origine (nettoyé) d'un mail, en téléchargeant son corps si besoin.
    async fn original_detail(&self, id: MessageId) -> Result<MessageDetail> {
        if let Some(d) = self.store.message_detail(id, false)? {
            return Ok(d);
        }
        let (account, _, _) = self.store.message_location(id)?;
        self.ask_worker(account, OPEN_TIMEOUT, |tx| WorkerRequest::FetchBody(id, tx)).await?;
        self.store.message_detail(id, false)?.ok_or(CoreError::NotFound)
    }

    pub async fn create_draft(&self, kind: DraftKind, account_id: Option<AccountId>, source: Option<MessageId>) -> Result<Draft> {
        use crate::compose::prefill::{quoted_body, reply_recipients, signature_block, subject_with_prefix};
        let mut draft = Draft {
            id: 0,
            account_id: 0,
            kind,
            source_message_id: source,
            to: vec![],
            cc: vec![],
            bcc: vec![],
            subject: String::new(),
            body_html: String::new(),
            attachments: vec![],
            state: DraftState::Editing,
            last_error: None,
            send_at: None,
            remote_uid: None,
            updated_at: 0,
        };
        match (kind, source) {
            (DraftKind::New, _) => {
                let account = match account_id {
                    Some(a) => self.store.get_account(a)?,
                    None => self.store.list_accounts()?.into_iter().next().ok_or(CoreError::InvalidInput("aucun compte".into()))?,
                };
                draft.account_id = account.id;
                draft.body_html = format!("<p></p>{}", signature_block(&self.store.account_signature(account.id)?));
            }
            (_, None) => return Err(CoreError::InvalidInput("mail d'origine manquant".into())),
            (_, Some(src_id)) if self.store.draft_for_message(src_id)?.is_some_and(|d| d.kind == kind) => {
                // Deuxième clic sur « Répondre » : on reprend la réponse déjà commencée.
                return Ok(self.store.draft_for_message(src_id)?.expect("vérifié juste avant"));
            }
            (_, Some(src_id)) => {
                let src = self.store.message_headers_for_reply(src_id)?;
                let detail = self.original_detail(src_id).await?;
                let original = match &detail.body {
                    MessageBody::Html(h) | MessageBody::Text(h) => h.clone(),
                };
                draft.account_id = src.account_id;
                let signature = self.store.account_signature(src.account_id)?;
                if kind == DraftKind::Forward {
                    draft.subject = subject_with_prefix("Fwd: ", &src.subject);
                    draft.attachments = detail
                        .attachments
                        .iter()
                        .map(|a| DraftAttachment {
                            filename: a.filename.clone(),
                            mime_type: a.mime_type.clone(),
                            size: a.size,
                            source: AttachmentSource::Part { message_id: src_id, part_path: a.part_path.clone() },
                        })
                        .collect();
                } else {
                    let own = self.store.own_addresses()?;
                    let (to, cc) = reply_recipients(&src, kind == DraftKind::ReplyAll, &own);
                    draft.to = to;
                    draft.cc = cc;
                    draft.subject = subject_with_prefix("Re: ", &src.subject);
                }
                draft.body_html = quoted_body(kind, &src, &original, &signature);
            }
        }
        let id = self.store.insert_draft(&draft)?;
        self.store.get_draft(id)
    }

    pub fn draft_for_message(&self, message_id: MessageId) -> Result<Option<Draft>> {
        self.store.draft_for_message(message_id)
    }

    /// Reprend un mail du dossier Brouillons pour le modifier (il remplacera la copie serveur).
    /// Si c'est la copie serveur d'un brouillon d'Illy, c'est ce brouillon local qui est rouvert
    /// (il garde les Cci, absents des copies serveur, et les pièces jointes transférées).
    pub async fn draft_from_message(&self, message_id: MessageId) -> Result<Draft> {
        let (account, folder, uid) = self.store.message_location(message_id)?;
        let in_drafts = self.store.folder_by_role(account, FolderRole::Drafts)?.is_some_and(|(f, _)| f == folder);
        if in_drafts {
            if let Some(local) = self.store.draft_by_remote_uid(account, uid)? {
                return Ok(local);
            }
        }
        let detail = self.original_detail(message_id).await?;
        let body = match detail.body {
            MessageBody::Html(h) | MessageBody::Text(h) => h,
        };
        let draft = Draft {
            id: 0,
            account_id: account,
            kind: DraftKind::New,
            source_message_id: None,
            to: detail.to,
            cc: detail.cc,
            bcc: vec![],
            subject: detail.summary.subject,
            body_html: body,
            attachments: detail
                .attachments
                .iter()
                .map(|a| DraftAttachment {
                    filename: a.filename.clone(),
                    mime_type: a.mime_type.clone(),
                    size: a.size,
                    source: AttachmentSource::Part { message_id, part_path: a.part_path.clone() },
                })
                .collect(),
            state: DraftState::Editing,
            last_error: None,
            send_at: None,
            remote_uid: in_drafts.then_some(uid),
            updated_at: 0,
        };
        let id = self.store.insert_draft(&draft)?;
        // La copie serveur sera remplacée (et supprimée) : ses pièces jointes sont copiées ici d'abord.
        self.copy_parts_locally(id).await?;
        self.store.get_draft(id)
    }

    /// Remplace les pièces jointes « partie d'un mail » d'un brouillon par des copies dans `drafts/<id>/`.
    /// Une partie impossible à télécharger (hors ligne) reste une référence au mail.
    async fn copy_parts_locally(&self, id: DraftId) -> Result<()> {
        let mut d = self.store.get_draft(id)?;
        let dir = self.drafts_dir.join(id.to_string());
        let mut changed = false;
        for att in &mut d.attachments {
            let AttachmentSource::Part { message_id, part_path } = &att.source else { continue };
            match self.attachment_bytes(*message_id, part_path).await {
                Ok((_, bytes)) => {
                    std::fs::create_dir_all(&dir)?;
                    let dest = unique_path(&dir, &sanitize_filename(&att.filename));
                    std::fs::write(&dest, &bytes)?;
                    att.size = bytes.len() as u64;
                    att.source = AttachmentSource::File { path: dest.to_string_lossy().to_string() };
                    changed = true;
                }
                Err(e) => tracing::warn!(draft = id, "pièce jointe gardée en référence : {e}"),
            }
        }
        if changed {
            self.store.set_draft_attachments(id, &d.attachments)?;
        }
        Ok(())
    }

    pub fn get_draft(&self, id: DraftId) -> Result<Draft> {
        self.store.get_draft(id)
    }

    pub fn update_draft(&self, input: DraftInput) -> Result<Draft> {
        let d = self.store.update_draft(&input)?;
        // Pendant la rédaction, la copie serveur ne repart pas chaque minute avec de grosses pièces jointes
        // (elle est faite à la fermeture de l'éditeur).
        let heavy = d.attachments.iter().map(|a| a.size).sum::<u64>() > PERIODIC_UPLOAD_MAX_BYTES;
        let due = !heavy && {
            let mut map = self.last_upload.lock().unwrap();
            let entry = map.entry(d.id).or_insert_with(std::time::Instant::now);
            if entry.elapsed() >= DRAFT_UPLOAD_EVERY {
                *entry = std::time::Instant::now();
                true
            } else {
                false
            }
        };
        if due {
            self.store.enqueue_op(d.account_id, OpKind::AppendDraft, serde_json::json!({"draftId": d.id}))?;
            self.manager.try_notify(d.account_id);
        }
        Ok(d)
    }

    pub fn add_attachments(&self, id: DraftId, paths: Vec<PathBuf>) -> Result<Draft> {
        let mut d = self.store.get_draft(id)?;
        let dir = self.drafts_dir.join(id.to_string());
        std::fs::create_dir_all(&dir)?;
        for p in paths {
            let meta = std::fs::metadata(&p)?;
            if !meta.is_file() {
                continue;
            }
            if meta.len() > MAX_ATTACHMENT_BYTES {
                return Err(CoreError::InvalidInput(format!(
                    "Fichier trop volumineux (100 Mo au plus) : {}",
                    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
                )));
            }
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "piece-jointe".into());
            let dest = unique_path(&dir, &sanitize_filename(&name));
            std::fs::copy(&p, &dest)?;
            d.attachments.push(DraftAttachment {
                mime_type: mime_guess::from_path(&name).first_or_octet_stream().essence_str().to_string(),
                filename: name,
                size: meta.len(),
                source: AttachmentSource::File { path: dest.to_string_lossy().to_string() },
            });
        }
        self.store.set_draft_attachments(id, &d.attachments)?;
        self.store.mark_draft_edited(id)?;
        self.store.get_draft(id)
    }

    pub fn remove_attachment(&self, id: DraftId, index: usize) -> Result<Draft> {
        let mut d = self.store.get_draft(id)?;
        if index >= d.attachments.len() {
            return Err(CoreError::NotFound);
        }
        let removed = d.attachments.remove(index);
        if let AttachmentSource::File { path } = &removed.source {
            if Path::new(path).starts_with(&self.drafts_dir) {
                let _ = std::fs::remove_file(path);
            }
        }
        self.store.set_draft_attachments(id, &d.attachments)?;
        self.store.mark_draft_edited(id)?;
        self.store.get_draft(id)
    }

    fn is_empty_draft(&self, d: &Draft) -> Result<bool> {
        let signature = crate::compose::prefill::signature_block(&self.store.account_signature(d.account_id)?);
        let text = crate::mime::html_to_text(&d.body_html);
        let sig_text = crate::mime::html_to_text(&signature);
        Ok(d.to.is_empty()
            && d.cc.is_empty()
            && d.bcc.is_empty()
            && d.subject.trim().is_empty()
            && d.attachments.is_empty()
            && (text.trim().is_empty() || text.trim() == sig_text.trim()))
    }

    /// Fermeture de l'éditeur : un brouillon vide est jeté, sinon il est copié sur le serveur.
    pub async fn close_draft(&self, id: DraftId) -> Result<()> {
        let d = self.store.get_draft(id)?;
        if !matches!(d.state, DraftState::Editing | DraftState::Failed) {
            return Ok(());
        }
        self.last_upload.lock().unwrap().remove(&id);
        // Jamais modifié (Répondre puis rien) ou vide : rien à garder, ni ici ni sur le serveur.
        if d.remote_uid.is_none() && (!self.store.draft_edited(id)? || self.is_empty_draft(&d)?) {
            let _ = std::fs::remove_dir_all(self.drafts_dir.join(id.to_string()));
            return self.store.delete_draft(id);
        }
        self.store.enqueue_op(d.account_id, OpKind::AppendDraft, serde_json::json!({"draftId": id}))?;
        let _ = self.manager.send(d.account_id, WorkerRequest::OpsAvailable).await;
        Ok(())
    }

    pub async fn discard_draft(&self, id: DraftId) -> Result<()> {
        let d = self.store.get_draft(id)?;
        if d.state == DraftState::Sending || (d.state == DraftState::Queued && !self.store.cancel_send_op(id)?) {
            return Err(CoreError::InvalidInput("ce mail est en cours d'envoi".into()));
        }
        if let (Some(uid), Some((folder, _))) = (d.remote_uid, self.store.folder_by_role(d.account_id, FolderRole::Drafts)?) {
            self.store.enqueue_op(d.account_id, OpKind::DeleteRemote, serde_json::json!({"folderId": folder, "uid": uid}))?;
            let _ = self.manager.send(d.account_id, WorkerRequest::OpsAvailable).await;
        }
        self.last_upload.lock().unwrap().remove(&id);
        let _ = std::fs::remove_dir_all(self.drafts_dir.join(id.to_string()));
        self.store.delete_draft(id)?;
        if matches!(d.state, DraftState::Queued | DraftState::Failed) {
            // Le bandeau (« Échec de l'envoi… ») le retire de la file.
            self.events.emit(CoreEvent::OutboxChanged { draft_id: id, state: DraftState::Editing, send_at: None, error: None });
        }
        Ok(())
    }

    /// Met l'envoi en file, avec le délai d'annulation des réglages.
    pub async fn send_draft(&self, id: DraftId) -> Result<SendTicket> {
        let d = self.store.get_draft(id)?;
        if !matches!(d.state, DraftState::Editing | DraftState::Failed) {
            return Err(CoreError::InvalidInput("ce mail est déjà en cours d'envoi".into()));
        }
        if d.to.is_empty() && d.cc.is_empty() && d.bcc.is_empty() {
            return Err(CoreError::InvalidInput("Ajoute au moins un destinataire.".into()));
        }
        for a in d.to.iter().chain(&d.cc).chain(&d.bcc) {
            if a.email.trim().parse::<lettre::Address>().is_err() {
                return Err(CoreError::InvalidInput(format!("Adresse invalide : {}", a.email.trim())));
            }
        }
        let delay = self.store.settings()?.undo_send_seconds as i64;
        let send_at = crate::store::now_ts() + delay;
        // Deux clics (ou deux fenêtres) ne peuvent pas mettre deux envois en file.
        if !self.store.queue_draft(id, send_at)? {
            return Err(CoreError::InvalidInput("ce mail est déjà en cours d'envoi".into()));
        }
        self.store.enqueue_op_at(d.account_id, OpKind::Send, serde_json::json!({"draftId": id}), send_at)?;
        self.last_upload.lock().unwrap().remove(&id);
        let _ = self.manager.send(d.account_id, WorkerRequest::OpsAvailable).await;
        self.events.emit(CoreEvent::OutboxChanged { draft_id: id, state: DraftState::Queued, send_at: Some(send_at), error: None });
        Ok(SendTicket { send_at })
    }

    /// Annule un envoi pas encore parti ; le brouillon redevient modifiable.
    pub fn cancel_send(&self, id: DraftId) -> Result<bool> {
        if !self.store.cancel_send_op(id)? {
            return Ok(false);
        }
        self.store.set_draft_state(id, DraftState::Editing, None, None)?;
        self.events.emit(CoreEvent::OutboxChanged { draft_id: id, state: DraftState::Editing, send_at: None, error: None });
        Ok(true)
    }

    pub fn list_outbox(&self) -> Result<Vec<Draft>> {
        self.store.outbox()
    }

    pub fn settings(&self) -> Result<Settings> {
        self.store.settings()
    }

    pub fn set_setting(&self, key: &str, value: serde_json::Value) -> Result<Settings> {
        let s = self.store.set_setting(key, value)?;
        self.events.emit(CoreEvent::SettingsChanged { key: key.to_string() });
        Ok(s)
    }

    pub fn attachment_filename(&self, id: MessageId, part_path: &str) -> Result<String> {
        let (info, _) = self.store.part(id, part_path)?;
        Ok(sanitize_filename(info.filename.as_deref().unwrap_or("piece-jointe")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_timeout_scales_with_size() {
        assert_eq!(attachment_timeout(0), Duration::from_secs(30));
        assert!(attachment_timeout(20_000_000) >= Duration::from_secs(400));
        assert_eq!(attachment_timeout(5_000_000_000), Duration::from_secs(600));
    }

    #[test]
    fn sanitize_filename_blocks_traversal() {
        assert_eq!(sanitize_filename("devis.pdf"), "devis.pdf");
        assert_eq!(sanitize_filename("../../evil.pdf"), "_.._evil.pdf");
        assert!(!sanitize_filename("..\\..\\x.pdf").contains('\\'));
        assert_eq!(sanitize_filename("a:b*c?.txt"), "a_b_c_.txt");
        assert_eq!(sanitize_filename(""), "piece-jointe");
        assert_eq!(sanitize_filename("..."), "piece-jointe");
        assert!(sanitize_filename(&"x".repeat(300)).chars().count() <= 120);
        assert_eq!(sanitize_filename("facture\u{202E}fdp.exe"), "facturefdp.exe");
        assert_eq!(sanitize_filename("a.pdf::$DATA"), "a.pdf__$DATA");
        assert!(!crate::providers::is_dangerous_filename("a.exe::$DATA"), "written as a.exe__$DATA: not executable");
    }
}
