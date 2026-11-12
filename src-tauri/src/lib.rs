//! Application Illy : démarrage, journalisation, pont d'événements.

pub mod commands;

use illy_core::events::{CoreEvent, EventSink};
use illy_core::secrets::SecretStore;
use illy_core::sync::WorkerConfig;
use illy_core::Core;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

struct TauriSink(AppHandle);

impl EventSink for TauriSink {
    fn emit(&self, event: CoreEvent) {
        if let Err(e) = self.0.emit("core-event", event) {
            tracing::warn!("événement non transmis : {e}");
        }
    }
}

/// Fichiers réellement déposés sur une fenêtre : les seuls que l'interface peut joindre par leur chemin.
#[derive(Default)]
pub struct DroppedFiles(pub Mutex<HashSet<PathBuf>>);

/// Garde du journal : doit vivre aussi longtemps que l'application.
struct LogGuard(#[allow(dead_code)] tracing_appender::non_blocking::WorkerGuard);

pub(crate) fn data_dir() -> Result<PathBuf, String> {
    let base = directories::BaseDirs::new().ok_or("dossier utilisateur introuvable")?;
    Ok(base.data_local_dir().join("Illy"))
}

fn init_logging(dir: &std::path::Path) -> LogGuard {
    // Un fichier par jour, 14 jours gardés.
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("illy")
        .filename_suffix("log")
        .max_log_files(14)
        .build(dir)
        .unwrap_or_else(|_| tracing_appender::rolling::daily(dir, "illy.log"));
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).with_writer(writer).with_ansi(false).try_init();
    LogGuard(guard)
}

#[cfg(windows)]
fn secret_store() -> Result<Arc<dyn SecretStore>, String> {
    Ok(Arc::new(illy_core::secrets::KeyringSecretStore::new().map_err(|e| e.to_string())?))
}

#[cfg(not(windows))]
fn secret_store() -> Result<Arc<dyn SecretStore>, String> {
    tracing::warn!("hors Windows : mots de passe gardés en mémoire uniquement (développement)");
    Ok(Arc::new(illy_core::secrets::MemorySecretStore::default()))
}

pub fn run() {
    tauri::Builder::default()
        // Une seule instance : relancer Illy remet la fenêtre existante au premier plan
        // (deux instances doubleraient les connexions et videraient le dossier temporaire de l'autre).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(DroppedFiles::default())
        .on_webview_event(|webview, event| {
            if let tauri::WebviewEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                if let Some(state) = webview.try_state::<DroppedFiles>() {
                    state.0.lock().unwrap().extend(paths.iter().cloned());
                }
            }
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
                if let Some(state) = window.try_state::<DroppedFiles>() {
                    state.0.lock().unwrap().extend(paths.iter().cloned());
                }
            }
            // Fermer la fenêtre principale ferme aussi les fenêtres « Nouveau mail »,
            // qui enregistrent leur brouillon avant de partir : Illy se termine ensuite.
            tauri::WindowEvent::CloseRequested { .. } if window.label() == "main" => {
                for (label, w) in window.app_handle().webview_windows() {
                    if label != "main" {
                        let _ = w.close();
                    }
                }
            }
            _ => {}
        })
        .setup(|app| {
            let data = data_dir()?;
            std::fs::create_dir_all(data.join("logs"))?;
            app.manage(init_logging(&data.join("logs")));
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "démarrage d'Illy");
            let runtime = tauri::async_runtime::handle().inner().clone();
            let core = Core::open(
                &data,
                secret_store()?,
                Arc::new(TauriSink(app.handle().clone())),
                WorkerConfig::default(),
                runtime,
            )
            .map_err(|e| e.to_string())?;
            app.manage(core);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::detect_provider,
            commands::add_account,
            commands::update_password,
            commands::list_accounts,
            commands::list_folders,
            commands::list_messages,
            commands::count_messages,
            commands::select_folder,
            commands::open_message,
            commands::allow_images_from,
            commands::open_attachment,
            commands::save_attachment,
            commands::open_url,
            commands::search_contacts,
            commands::create_draft,
            commands::draft_for_message,
            commands::draft_from_message,
            commands::get_draft,
            commands::update_draft,
            commands::add_attachments,
            commands::pick_attachments,
            commands::remove_attachment,
            commands::close_draft,
            commands::discard_draft,
            commands::send_draft,
            commands::cancel_send,
            commands::list_outbox,
            commands::get_settings,
            commands::set_setting,
            commands::open_compose_window,
            commands::reorder_accounts,
            commands::update_account,
            commands::update_servers,
            commands::remove_account,
            commands::list_image_allowlist,
            commands::remove_from_image_allowlist,
            commands::storage_usage,
            commands::clear_cache,
            commands::open_logs_folder,
            commands::app_info,
        ])
        .run(tauri::generate_context!())
        .expect("échec du démarrage d'Illy");
}
