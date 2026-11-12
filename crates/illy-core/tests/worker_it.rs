#![cfg(feature = "insecure-plain")]

mod common;

use common::*;
use illy_core::events::{CoreEvent, RecordingSink};
use illy_core::model::{AccountStatus, FolderRole, ListScope};
use illy_core::secrets::{MemorySecretStore, SecretStore};
use illy_core::store::{OpKind, Store};
use illy_core::sync::{SyncManager, WorkerConfig, WorkerRequest};
use std::sync::Arc;
use std::time::Duration;

fn fast_cfg() -> WorkerConfig {
    WorkerConfig {
        poll_interval: Duration::from_secs(1),
        other_folders_interval: Duration::from_secs(2),
        backoff_min: Duration::from_millis(200),
        backoff_max: Duration::from_secs(1),
        ..Default::default()
    }
}

struct Env {
    gm: GreenMail,
    store: Store,
    secrets: Arc<MemorySecretStore>,
    sink: Arc<RecordingSink>,
    mgr: SyncManager,
    acc: i64,
}

fn env(gm: GreenMail, password: &str, cfg: WorkerConfig) -> Env {
    let store = Store::open_in_memory().unwrap();
    let acc = store.insert_account(&gm.new_account()).unwrap().id;
    let secrets = Arc::new(MemorySecretStore::default());
    secrets.set(acc, password).unwrap();
    let sink = Arc::new(RecordingSink::default());
    let mgr = SyncManager::new(store.clone(), secrets.clone(), sink.clone(), cfg, tokio::runtime::Handle::current());
    Env { gm, store, secrets, sink, mgr, acc }
}

fn inbox(store: &Store, acc: i64) -> Option<i64> {
    store.syncable_folders(acc).ok()?.into_iter().find(|f| f.2 == FolderRole::Inbox).map(|f| f.0)
}

fn count(store: &Store, acc: i64) -> u32 {
    inbox(store, acc).map(|f| store.count_messages(ListScope::Folder(f)).unwrap()).unwrap_or(0)
}

#[tokio::test(flavor = "multi_thread")]
async fn worker_syncs_and_emits() {
    let gm = GreenMail::start();
    gm.append_many("INBOX", 1, 3).await;
    let e = env(gm, PASSWORD, fast_cfg());
    e.mgr.start(e.acc);
    assert!(wait_until(10, || count(&e.store, e.acc) == 3).await);
    assert!(wait_until(5, || e.store.get_account(e.acc).unwrap().status == AccountStatus::Ok).await);
    let ev = e.sink.events();
    assert!(ev.contains(&CoreEvent::FoldersChanged { account_id: e.acc }));
    assert!(ev.iter().any(|x| matches!(x, CoreEvent::FolderChanged { .. })));
    // Les corps récents sont préchargés en arrière-plan.
    let f = inbox(&e.store, e.acc).unwrap();
    let ids: Vec<i64> = e.store.list_messages(ListScope::Folder(f), 0, 10).unwrap().iter().map(|m| m.id).collect();
    assert!(wait_until(10, || ids.iter().all(|id| e.store.message_detail(*id, false).unwrap().is_some())).await);
    e.mgr.stop(e.acc);
}

#[tokio::test(flavor = "multi_thread")]
async fn new_mail_arrives() {
    let gm = GreenMail::start();
    gm.append_many("INBOX", 1, 1).await;
    let e = env(gm, PASSWORD, fast_cfg());
    e.mgr.start(e.acc);
    assert!(wait_until(10, || count(&e.store, e.acc) == 1).await);
    tokio::time::sleep(Duration::from_millis(500)).await;
    e.gm.append_many("INBOX", 2, 1).await;
    assert!(wait_until(10, || count(&e.store, e.acc) == 2).await, "new mail not synced");
    e.mgr.stop(e.acc);
}

#[tokio::test(flavor = "multi_thread")]
async fn auth_error_stops_retrying() {
    let gm = GreenMail::start();
    let e = env(gm, "wrong", fast_cfg());
    e.mgr.start(e.acc);
    assert!(wait_until(10, || e.store.get_account(e.acc).unwrap().status == AccountStatus::AuthError).await);
    tokio::time::sleep(Duration::from_secs(3)).await;
    let auth_events = e
        .sink
        .events()
        .iter()
        .filter(|x| matches!(x, CoreEvent::AccountStatus { status: AccountStatus::AuthError, .. }))
        .count();
    assert_eq!(auth_events, 1, "must not retry in a loop");
    e.secrets.set(e.acc, PASSWORD).unwrap();
    e.mgr.restart(e.acc);
    assert!(wait_until(10, || e.store.get_account(e.acc).unwrap().status == AccountStatus::Ok).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn fetch_body_request_has_priority() {
    let gm = GreenMail::start();
    gm.append_many("INBOX", 1, 2000).await;
    let e = env(gm, PASSWORD, WorkerConfig { header_batch: 100, ..fast_cfg() });
    e.mgr.start(e.acc);
    assert!(wait_until(15, || count(&e.store, e.acc) >= 100).await);
    let f = inbox(&e.store, e.acc).unwrap();
    let newest = e.store.list_messages(ListScope::Folder(f), 0, 1).unwrap()[0].id;
    let (tx, rx) = tokio::sync::oneshot::channel();
    let t0 = std::time::Instant::now();
    e.mgr.send(e.acc, WorkerRequest::FetchBody(newest, tx)).await.unwrap();
    rx.await.unwrap().unwrap();
    assert!(t0.elapsed() < Duration::from_secs(5), "took {:?}", t0.elapsed());
    e.mgr.stop(e.acc);
}

#[tokio::test(flavor = "multi_thread")]
async fn set_seen_op_reaches_server() {
    let gm = GreenMail::start();
    gm.append_many("INBOX", 1, 1).await;
    let e = env(gm, PASSWORD, fast_cfg());
    e.mgr.start(e.acc);
    assert!(wait_until(10, || count(&e.store, e.acc) == 1).await);
    let f = inbox(&e.store, e.acc).unwrap();
    e.store.enqueue_op(e.acc, OpKind::SetSeen, serde_json::json!({"folderId": f, "uid": 1, "seen": true})).unwrap();
    e.mgr.send(e.acc, WorkerRequest::OpsAvailable).await.unwrap();
    let mut ok = false;
    for _ in 0..50 {
        if e.gm.server_flags("INBOX").await == vec![(1, true, false)] {
            ok = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ok, "seen flag not on server");
    assert!(wait_until(5, || e.store.pending_ops_count(e.acc).unwrap() == 0).await);
    e.mgr.stop(e.acc);
}

#[tokio::test(flavor = "multi_thread")]
async fn broken_mail_does_not_block_sync() {
    let gm = GreenMail::start();
    gm.append_many("INBOX", 1, 2).await;
    gm.append("INBOX", &fixture("broken_mime.eml"), false).await;
    gm.append_many("INBOX", 3, 2).await;
    let e = env(gm, PASSWORD, fast_cfg());
    e.mgr.start(e.acc);
    assert!(wait_until(15, || count(&e.store, e.acc) == 5).await, "count = {}", count(&e.store, e.acc));
    assert!(wait_until(10, || e.store.get_account(e.acc).unwrap().status == AccountStatus::Ok).await);
    e.mgr.stop(e.acc);
}
