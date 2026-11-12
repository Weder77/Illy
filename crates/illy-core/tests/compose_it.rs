#![cfg(feature = "insecure-plain")]

mod common;

use common::*;
use illy_core::events::{CoreEvent, RecordingSink};
use illy_core::imap::{connect, fetch_headers, search_all_uids, select};
use illy_core::model::*;
use illy_core::store::{now_ts, OpKind, Store};
use illy_core::sync::{run_due_ops, sync_folder, WorkerConfig};

struct Env {
    gm: GreenMail,
    store: Store,
    acc: Account,
    sink: RecordingSink,
    cfg: WorkerConfig,
    _dir: tempfile::TempDir,
}

async fn env() -> Env {
    let gm = GreenMail::start();
    gm.create_folder("Sent").await;
    gm.create_folder("Drafts").await;
    let (store, acc_id, _) = gm.store_with_folders().await;
    let acc = store.get_account(acc_id).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let cfg = WorkerConfig { data_dir: Some(dir.path().to_path_buf()), ..Default::default() };
    Env { gm, store, acc, sink: RecordingSink::default(), cfg, _dir: dir }
}

fn draft(e: &Env, subject: &str) -> Draft {
    Draft {
        id: 0,
        account_id: e.acc.id,
        kind: DraftKind::New,
        source_message_id: None,
        to: vec![Address { name: Some("Bob".into()), email: BOB.into() }],
        cc: vec![],
        bcc: vec![],
        subject: subject.into(),
        body_html: "<p>Bonjour Bob, voici le récapitulatif.</p>".into(),
        attachments: vec![],
        state: DraftState::Editing,
        last_error: None,
        send_at: None,
        remote_uid: None,
        updated_at: 0,
    }
}

async fn run(e: &Env, password: &str) {
    let (mut s, caps) = connect(&e.gm.imap(), USER, PASSWORD).await.unwrap();
    run_due_ops(&mut s, &caps, &e.store, &e.sink, &e.acc, password, &e.cfg).await.unwrap();
}

fn queue_send(e: &Env, id: DraftId) {
    e.store.set_draft_state(id, DraftState::Queued, None, Some(now_ts())).unwrap();
    e.store.enqueue_op(e.acc.id, OpKind::Send, serde_json::json!({"draftId": id})).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn send_reaches_bob_with_accents() {
    let e = env().await;
    let file = e._dir.path().join("facture é.pdf");
    std::fs::write(&file, b"%PDF-1.4").unwrap();
    let mut d = draft(&e, "Réunion à 14h");
    d.attachments = vec![DraftAttachment {
        filename: "facture é.pdf".into(),
        mime_type: "application/pdf".into(),
        size: 8,
        source: AttachmentSource::File { path: file.to_string_lossy().into() },
    }];
    let id = e.store.insert_draft(&d).unwrap();
    queue_send(&e, id);
    run(&e, PASSWORD).await;
    assert_eq!(e.store.get_draft(id).unwrap().state, DraftState::Sent);
    let mut b = e.gm.session_as(BOB).await;
    select(&mut b, "INBOX", false).await.unwrap();
    let uids = search_all_uids(&mut b).await.unwrap();
    assert_eq!(uids.len(), 1);
    let h = fetch_headers(&mut b, &uids, true).await.unwrap();
    assert_eq!(h[0].headers.subject, "Réunion à 14h");
    assert_eq!(h[0].headers.from.email, USER);
    let att = h[0].parts.iter().find(|p| p.filename.is_some()).unwrap();
    assert_eq!(att.filename.as_deref(), Some("facture é.pdf"));
    assert!(e.sink.events().iter().any(|x| matches!(x, CoreEvent::OutboxChanged { state: DraftState::Sent, .. })));
}

#[tokio::test(flavor = "multi_thread")]
async fn sent_copy_added_once() {
    let e = env().await;
    let id = e.store.insert_draft(&draft(&e, "Copie")).unwrap();
    queue_send(&e, id);
    run(&e, PASSWORD).await;
    assert_eq!(e.gm.count_in(USER, "Sent").await, 0, "sent copy waits 15 s (Gmail may add it itself)");
    e.store.ops_due_now(e.acc.id).unwrap();
    run(&e, PASSWORD).await;
    assert_eq!(e.gm.count_in(USER, "Sent").await, 1);
    assert!(matches!(e.store.get_draft(id), Err(illy_core::CoreError::NotFound)), "local draft cleaned after the copy");
    e.store.enqueue_op(e.acc.id, OpKind::SentCopy, serde_json::json!({"draftId": id, "messageId": "<x@y>"})).unwrap();
    run(&e, PASSWORD).await;
    assert_eq!(e.gm.count_in(USER, "Sent").await, 1, "no duplicate");
}

#[tokio::test(flavor = "multi_thread")]
async fn reply_sets_answered() {
    let e = env().await;
    e.gm.append("INBOX", &simple_eml(1), false).await;
    let inbox = e.store.syncable_folders(e.acc.id).unwrap().into_iter().find(|f| f.2 == FolderRole::Inbox).unwrap();
    let (mut s, caps) = connect(&e.gm.imap(), USER, PASSWORD).await.unwrap();
    sync_folder(&mut s, &caps, &e.store, &e.sink, e.acc.id, inbox.0, "INBOX", &e.cfg, true).await.unwrap();
    let original = e.store.message_id_by_uid(inbox.0, 1).unwrap().unwrap();
    let mut d = draft(&e, "Re: Mail 1");
    d.kind = DraftKind::Reply;
    d.source_message_id = Some(original);
    let id = e.store.insert_draft(&d).unwrap();
    queue_send(&e, id);
    run(&e, PASSWORD).await;
    run(&e, PASSWORD).await;
    assert_eq!(e.gm.server_flags("INBOX").await, vec![(1, false, true)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn append_draft_replaces_previous() {
    let e = env().await;
    let id = e.store.insert_draft(&draft(&e, "Version 1")).unwrap();
    e.store.enqueue_op(e.acc.id, OpKind::AppendDraft, serde_json::json!({"draftId": id})).unwrap();
    run(&e, PASSWORD).await;
    assert_eq!(e.gm.count_in(USER, "Drafts").await, 1);
    let first = e.store.get_draft(id).unwrap().remote_uid.expect("remote uid recorded");
    let d = e.store.get_draft(id).unwrap();
    e.store
        .update_draft(&DraftInput { id, account_id: d.account_id, to: d.to, cc: vec![], bcc: vec![], subject: "Version 2".into(), body_html: d.body_html })
        .unwrap();
    e.store.enqueue_op(e.acc.id, OpKind::AppendDraft, serde_json::json!({"draftId": id})).unwrap();
    run(&e, PASSWORD).await;
    assert_eq!(e.gm.count_in(USER, "Drafts").await, 1, "old copy replaced");
    assert_ne!(e.store.get_draft(id).unwrap().remote_uid, Some(first));
    let mut s = e.gm.session().await;
    select(&mut s, "Drafts", false).await.unwrap();
    let uids = search_all_uids(&mut s).await.unwrap();
    assert_eq!(fetch_headers(&mut s, &uids, true).await.unwrap()[0].headers.subject, "Version 2");
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_before_due_sends_nothing() {
    let e = env().await;
    let id = e.store.insert_draft(&draft(&e, "Annulé")).unwrap();
    e.store.set_draft_state(id, DraftState::Queued, None, Some(now_ts() + 2)).unwrap();
    e.store.enqueue_op_at(e.acc.id, OpKind::Send, serde_json::json!({"draftId": id}), now_ts() + 2).unwrap();
    assert!(e.store.cancel_send_op(id).unwrap());
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    run(&e, PASSWORD).await;
    assert_eq!(e.gm.count_in(BOB, "INBOX").await, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn auth_failure_marks_failed_without_retry() {
    let e = env().await;
    let id = e.store.insert_draft(&draft(&e, "Refusé")).unwrap();
    queue_send(&e, id);
    run(&e, "mauvais").await;
    let d = e.store.get_draft(id).unwrap();
    assert_eq!(d.state, DraftState::Failed);
    assert!(d.last_error.unwrap().contains("Mot de passe refusé"));
    assert_eq!(e.store.pending_ops_count(e.acc.id).unwrap(), 0, "no retry loop");
    let failed = e.sink.events().iter().filter(|x| matches!(x, CoreEvent::OutboxChanged { state: DraftState::Failed, .. })).count();
    assert_eq!(failed, 1);
    assert_eq!(e.gm.count_in(BOB, "INBOX").await, 0);
}
