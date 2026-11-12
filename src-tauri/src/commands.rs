//! Commandes Tauri : fine couche au-dessus de `illy_core::Core`.

use illy_core::model::*;
use illy_core::{Core, CoreError};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

type R<T> = Result<T, CoreError>;

/// Seuls les liens web s'ouvrent depuis un mail.
pub fn check_url(url: &str) -> R<()> {
    let lower = url.trim().to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        Ok(())
    } else {
        Err(CoreError::InvalidInput("seuls les liens http(s) peuvent être ouverts".into()))
    }
}

// Les commandes « sync » tournent sur un fil de travail (`async`), jamais sur le fil de la fenêtre.

#[tauri::command(async)]
pub fn detect_provider(core: State<'_, Core>, email: String) -> R<ProviderGuess> {
    Ok(core.detect_provider(&email))
}

#[tauri::command]
pub async fn add_account(core: State<'_, Core>, input: NewAccount, password: String) -> R<Account> {
    core.add_account(input, password).await
}

#[tauri::command]
pub async fn update_password(core: State<'_, Core>, account_id: AccountId, password: String) -> R<()> {
    core.update_password(account_id, password).await
}

#[tauri::command(async)]
pub fn list_accounts(core: State<'_, Core>) -> R<Vec<Account>> {
    core.list_accounts()
}

#[tauri::command(async)]
pub fn list_folders(core: State<'_, Core>, account_id: AccountId) -> R<Vec<Folder>> {
    core.list_folders(account_id)
}

#[tauri::command(async)]
pub fn list_messages(core: State<'_, Core>, scope: ListScope, offset: u32, limit: u32) -> R<Vec<MessageSummary>> {
    core.list_messages(scope, offset, limit)
}

#[tauri::command(async)]
pub fn count_messages(core: State<'_, Core>, scope: ListScope) -> R<u32> {
    core.count_messages(scope)
}

#[tauri::command]
pub async fn select_folder(core: State<'_, Core>, folder_id: FolderId) -> R<()> {
    core.select_folder(folder_id).await
}

#[tauri::command]
pub async fn open_message(core: State<'_, Core>, id: MessageId, show_images: bool) -> R<MessageDetail> {
    core.open_message(id, show_images).await
}

#[tauri::command(async)]
pub fn allow_images_from(core: State<'_, Core>, sender: String) -> R<()> {
    core.allow_images_from(&sender)
}

#[tauri::command]
pub async fn open_attachment(app: AppHandle, core: State<'_, Core>, id: MessageId, part_path: String) -> R<()> {
    let path = core.attachment_to_tmp(id, &part_path).await?;
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| CoreError::InvalidInput(format!("impossible d'ouvrir le fichier : {e}")))
}

#[tauri::command]
pub async fn save_attachment(app: AppHandle, core: State<'_, Core>, id: MessageId, part_path: String) -> R<()> {
    let name = core.attachment_filename(id, &part_path)?;
    let Some(choice) = app.dialog().file().set_file_name(&name).blocking_save_file() else {
        return Ok(());
    };
    let dest = choice.into_path().map_err(|e| CoreError::InvalidInput(e.to_string()))?;
    core.save_attachment(id, &part_path, &dest).await
}

#[tauri::command(async)]
pub fn open_url(app: AppHandle, url: String) -> R<()> {
    check_url(&url)?;
    app.opener()
        .open_url(url.trim(), None::<&str>)
        .map_err(|e| CoreError::InvalidInput(format!("impossible d'ouvrir le lien : {e}")))
}

// --- Rédaction ---

#[tauri::command(async)]
pub fn search_contacts(core: State<'_, Core>, query: String) -> R<Vec<Address>> {
    core.search_contacts(&query)
}

#[tauri::command]
pub async fn create_draft(
    core: State<'_, Core>,
    kind: DraftKind,
    account_id: Option<AccountId>,
    source_message_id: Option<MessageId>,
) -> R<Draft> {
    core.create_draft(kind, account_id, source_message_id).await
}

#[tauri::command(async)]
pub fn draft_for_message(core: State<'_, Core>, message_id: MessageId) -> R<Option<Draft>> {
    core.draft_for_message(message_id)
}

#[tauri::command]
pub async fn draft_from_message(core: State<'_, Core>, message_id: MessageId) -> R<Draft> {
    core.draft_from_message(message_id).await
}

#[tauri::command(async)]
pub fn get_draft(core: State<'_, Core>, draft_id: DraftId) -> R<Draft> {
    core.get_draft(draft_id)
}

#[tauri::command(async)]
pub fn update_draft(core: State<'_, Core>, input: DraftInput) -> R<Draft> {
    core.update_draft(input)
}

#[tauri::command(async)]
pub fn add_attachments(core: State<'_, Core>, draft_id: DraftId, paths: Vec<PathBuf>) -> R<Draft> {
    core.add_attachments(draft_id, paths)
}

/// Sélecteur de fichiers puis ajout ; annuler le sélecteur rend le brouillon tel quel.
#[tauri::command(async)]
pub fn pick_attachments(app: AppHandle, core: State<'_, Core>, draft_id: DraftId) -> R<Draft> {
    let Some(files) = app.dialog().file().set_title("Joindre des fichiers").blocking_pick_files() else {
        return core.get_draft(draft_id);
    };
    let paths = files.into_iter().filter_map(|f| f.into_path().ok()).collect();
    core.add_attachments(draft_id, paths)
}

#[tauri::command(async)]
pub fn remove_attachment(core: State<'_, Core>, draft_id: DraftId, index: usize) -> R<Draft> {
    core.remove_attachment(draft_id, index)
}

#[tauri::command]
pub async fn close_draft(core: State<'_, Core>, draft_id: DraftId) -> R<()> {
    core.close_draft(draft_id).await
}

#[tauri::command]
pub async fn discard_draft(core: State<'_, Core>, draft_id: DraftId) -> R<()> {
    core.discard_draft(draft_id).await
}

#[tauri::command]
pub async fn send_draft(core: State<'_, Core>, draft_id: DraftId) -> R<SendTicket> {
    core.send_draft(draft_id).await
}

#[tauri::command(async)]
pub fn cancel_send(core: State<'_, Core>, draft_id: DraftId) -> R<bool> {
    core.cancel_send(draft_id)
}

#[tauri::command(async)]
pub fn list_outbox(core: State<'_, Core>) -> R<Vec<Draft>> {
    core.list_outbox()
}

#[tauri::command(async)]
pub fn get_settings(core: State<'_, Core>) -> R<Settings> {
    core.settings()
}

#[tauri::command(async)]
pub fn set_setting(core: State<'_, Core>, key: String, value: serde_json::Value) -> R<Settings> {
    core.set_setting(&key, value)
}

pub fn compose_label(draft_id: DraftId) -> String {
    format!("compose-{draft_id}")
}

pub fn compose_url(draft_id: DraftId) -> String {
    format!("index.html#compose/{draft_id}")
}

/// Ouvre (ou ramène au premier plan) la fenêtre « Nouveau mail » d'un brouillon.
#[tauri::command(async)]
pub fn open_compose_window(app: AppHandle, draft_id: DraftId) -> R<()> {
    let label = compose_label(draft_id);
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(());
    }
    WebviewWindowBuilder::new(&app, label, WebviewUrl::App(compose_url(draft_id).into()))
        .title("Nouveau mail")
        .inner_size(760.0, 640.0)
        .min_inner_size(560.0, 420.0)
        .center()
        .decorations(false)
        .build()
        .map(|_| ())
        .map_err(|e| CoreError::InvalidInput(format!("impossible d'ouvrir la fenêtre : {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_url_rejects_non_http() {
        for bad in ["file:///C:/x", "javascript:x", "mailto:a@b", "ms-settings:", "", "  "] {
            assert!(check_url(bad).is_err(), "{bad}");
        }
        assert!(check_url("https://ex.fr").is_ok());
        assert!(check_url("HTTP://ex.fr/a?b=c").is_ok());
    }

    #[test]
    fn compose_label_format() {
        assert_eq!(compose_label(12), "compose-12");
        assert_eq!(compose_url(12), "index.html#compose/12");
    }
}
