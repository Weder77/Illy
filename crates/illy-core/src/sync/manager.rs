//! Un worker par compte.

use super::worker::{run_worker, WorkerCtx, WorkerRequest};
use super::WorkerConfig;
use crate::error::{CoreError, Result};
use crate::events::EventSink;
use crate::model::AccountId;
use crate::secrets::SecretStore;
use crate::store::Store;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

struct WorkerHandle {
    tx: mpsc::Sender<WorkerRequest>,
    task: tokio::task::JoinHandle<()>,
}

pub struct SyncManager {
    store: Store,
    secrets: Arc<dyn SecretStore>,
    events: Arc<dyn EventSink>,
    cfg: WorkerConfig,
    runtime: tokio::runtime::Handle,
    workers: Mutex<HashMap<AccountId, WorkerHandle>>,
}

impl SyncManager {
    pub fn new(
        store: Store,
        secrets: Arc<dyn SecretStore>,
        events: Arc<dyn EventSink>,
        cfg: WorkerConfig,
        runtime: tokio::runtime::Handle,
    ) -> Self {
        SyncManager { store, secrets, events, cfg, runtime, workers: Mutex::new(HashMap::new()) }
    }

    /// Démarre le worker du compte (sans effet s'il tourne déjà).
    pub fn start(&self, account: AccountId) {
        let mut w = self.workers.lock().unwrap();
        if w.get(&account).is_some_and(|h| !h.task.is_finished()) {
            return;
        }
        let (tx, rx) = mpsc::channel(64);
        let ctx = WorkerCtx {
            account,
            store: self.store.clone(),
            secrets: self.secrets.clone(),
            events: self.events.clone(),
            cfg: self.cfg.clone(),
        };
        let task = self.runtime.spawn(run_worker(ctx, rx));
        w.insert(account, WorkerHandle { tx, task });
    }

    pub fn stop(&self, account: AccountId) {
        if let Some(h) = self.workers.lock().unwrap().remove(&account) {
            h.task.abort();
        }
    }

    pub fn restart(&self, account: AccountId) {
        self.stop(account);
        self.start(account);
    }

    pub async fn send(&self, account: AccountId, req: WorkerRequest) -> Result<()> {
        let tx = self.workers.lock().unwrap().get(&account).map(|h| h.tx.clone()).ok_or(CoreError::NotFound)?;
        tx.send(req).await.map_err(|_| CoreError::NotFound)
    }
}

impl Drop for SyncManager {
    fn drop(&mut self) {
        for (_, h) in self.workers.lock().unwrap().drain() {
            h.task.abort();
        }
    }
}
