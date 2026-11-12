//! Synchronisation IMAP → base locale.

mod diff;
mod body;
mod folder_sync;
mod manager;
mod ops_exec;
mod worker;

pub use diff::{diff_uids, uidvalidity_changed, UidDiff};
pub use body::{fetch_attachment, fetch_body};
pub use folder_sync::{backfill_step, sync_folder, FolderBacklog};
pub use manager::SyncManager;
pub use ops_exec::{backoff, run_due_ops};
pub use worker::WorkerRequest;

use std::time::Duration;

/// Réglages de synchro (valeurs par défaut : contraintes globales du plan).
#[derive(Clone, Debug)]
pub struct WorkerConfig {
    pub header_batch: u32,
    pub recent_body_days: i64,
    pub poll_interval: Duration,
    pub other_folders_interval: Duration,
    pub idle_renew: Duration,
    pub flags_recent_window: usize,
    pub backoff_min: Duration,
    pub backoff_max: Duration,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        WorkerConfig {
            header_batch: 500,
            recent_body_days: 30,
            poll_interval: Duration::from_secs(60),
            other_folders_interval: Duration::from_secs(5 * 60),
            idle_renew: Duration::from_secs(25 * 60),
            flags_recent_window: 2000,
            backoff_min: Duration::from_secs(5),
            backoff_max: Duration::from_secs(5 * 60),
        }
    }
}
