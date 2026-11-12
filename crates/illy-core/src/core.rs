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

pub struct Core {
    store: Store,
    secrets: Arc<dyn SecretStore>,
    events: Arc<dyn EventSink>,
    manager: SyncManager,
    tmp_dir: PathBuf,
}

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
    pub fn open(
        db_path: &Path,
        tmp_dir: &Path,
        secrets: Arc<dyn SecretStore>,
        events: Arc<dyn EventSink>,
        cfg: WorkerConfig,
        runtime: tokio::runtime::Handle,
    ) -> Result<Core> {
        if tmp_dir.exists() {
            let _ = std::fs::remove_dir_all(tmp_dir);
        }
        std::fs::create_dir_all(tmp_dir)?;
        let store = Store::open(db_path)?;
        let manager = SyncManager::new(store.clone(), secrets.clone(), events.clone(), cfg, runtime);
        for a in store.list_accounts()? {
            manager.start(a.id);
        }
        Ok(Core { store, secrets, events, manager, tmp_dir: tmp_dir.to_path_buf() })
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

    pub fn remove_account(&self, account: AccountId) -> Result<()> {
        self.manager.stop(account);
        self.secrets.delete(account)?;
        self.store.delete_account(account)
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
        make: impl FnOnce(oneshot::Sender<Result<T>>) -> WorkerRequest,
    ) -> Result<T> {
        let (tx, rx) = oneshot::channel();
        self.manager
            .send(account, make(tx))
            .await
            .map_err(|_| CoreError::Network("ce compte n'est pas connecté".into()))?;
        match tokio::time::timeout(OPEN_TIMEOUT, rx).await {
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
                self.ask_worker(account, |tx| WorkerRequest::FetchBody(id, tx)).await?;
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
                self.ask_worker(account, |tx| WorkerRequest::FetchAttachment(id, part, tx)).await?
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

    pub fn attachment_filename(&self, id: MessageId, part_path: &str) -> Result<String> {
        let (info, _) = self.store.part(id, part_path)?;
        Ok(sanitize_filename(info.filename.as_deref().unwrap_or("piece-jointe")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
