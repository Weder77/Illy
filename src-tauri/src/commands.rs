//! Commandes Tauri : fine couche au-dessus de `illy_core::Core`.

use illy_core::model::*;
use illy_core::{Core, CoreError};
use tauri::{AppHandle, State};
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
}
