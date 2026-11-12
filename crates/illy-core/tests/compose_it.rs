#![cfg(feature = "insecure-plain")]

mod common;

use common::*;
use illy_core::events::{CoreEvent, RecordingSink};
use illy_core::imap::{connect, fetch_headers, search_all_uids, select};
use illy_core::model::*;
use illy_core::store::{now_ts, OpKind, Store};
#[allow(unused_imports)]
use illy_core::model::ListScope;
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
    assert_eq!(e.gm.count_in(USER, "Sent").await, 0, "a reconnection does not shorten the 15 s");
    e.store.force_ops_due(e.acc.id).unwrap();
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

#[tokio::test(flavor = "multi_thread")]
async fn smtp_refusal_is_not_a_password_error() {
    let mut e = env().await;
    let smtp = FakeSmtp::start("550 5.1.1 <bob@localhost>: User unknown").await;
    e.acc.smtp = smtp.config();
    let id = e.store.insert_draft(&draft(&e, "Refus")).unwrap();
    queue_send(&e, id);
    run(&e, PASSWORD).await;
    let d = e.store.get_draft(id).unwrap();
    assert_eq!(d.state, DraftState::Failed);
    let err = d.last_error.unwrap();
    assert!(err.contains("User unknown"), "the server's reason is shown: {err}");
    assert!(!err.contains("Mot de passe"), "{err}");
    assert_eq!(e.store.get_account(e.acc.id).unwrap().status, AccountStatus::Ok, "the account is not put in auth error");
    assert_eq!(e.store.pending_ops_count(e.acc.id).unwrap(), 0, "no retry for a permanent refusal");
}

#[tokio::test(flavor = "multi_thread")]
async fn smtp_try_later_is_retried() {
    let mut e = env().await;
    let smtp = FakeSmtp::start("451 4.7.1 Greylisted, try again later").await;
    e.acc.smtp = smtp.config();
    let id = e.store.insert_draft(&draft(&e, "Plus tard")).unwrap();
    queue_send(&e, id);
    run(&e, PASSWORD).await;
    let d = e.store.get_draft(id).unwrap();
    assert_eq!(d.state, DraftState::Queued, "a 4xx answer is temporary");
    let ops = e.store.due_ops(e.acc.id, now_ts() + 10_000).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].attempts, 1);
}

mod core_level {
    use super::*;
    use illy_core::secrets::MemorySecretStore;
    use illy_core::{Core, CoreError};
    use std::sync::Arc;
    use std::time::Duration;

    fn open(dir: &std::path::Path, secrets: Arc<MemorySecretStore>) -> Core {
        let cfg = WorkerConfig { poll_interval: Duration::from_secs(1), backoff_min: Duration::from_millis(200), ..Default::default() };
        Core::open(dir, secrets, Arc::new(RecordingSink::default()), cfg, tokio::runtime::Handle::current()).unwrap()
    }

    async fn setup(gm: &GreenMail) -> (tempfile::TempDir, Arc<MemorySecretStore>, Core, i64) {
        gm.create_folder("Sent").await;
        gm.create_folder("Drafts").await;
        let dir = tempfile::tempdir().unwrap();
        let secrets = Arc::new(MemorySecretStore::default());
        let core = open(dir.path(), secrets.clone());
        let acc = core.add_account(gm.new_account(), PASSWORD.into()).await.unwrap().id;
        core.set_setting("undo_send_seconds", serde_json::json!(5)).unwrap();
        (dir, secrets, core, acc)
    }

    fn eml_from_bob(n: u32) -> Vec<u8> {
        format!(
            "From: Bob <bob@localhost>\r\nTo: alice@localhost\r\nSubject: Question {n}\r\nMessage-ID: <q{n}@localhost>\r\n\
             Date: 01 Nov 2026 10:00:00 +0000\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nTu viens ?\r\n"
        )
        .into_bytes()
    }

    async fn inbox_message(core: &Core, acc: i64, subject: &str) -> i64 {
        for _ in 0..200 {
            if let Some(f) = core.list_folders(acc).unwrap().into_iter().find(|f| f.role == FolderRole::Inbox) {
                if let Some(m) = core.list_messages(ListScope::Folder(f.id), 0, 50).unwrap().into_iter().find(|m| m.subject == subject) {
                    return m.id;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("mail {subject} not synced");
    }

    /// Mail d'un dossier (par rôle) une fois synchronisé ; relance la synchro de ce dossier en attendant.
    async fn folder_message(core: &Core, acc: i64, role: FolderRole, subject: &str) -> i64 {
        for _ in 0..150 {
            if let Some(f) = core.list_folders(acc).unwrap().into_iter().find(|f| f.role == role) {
                if let Some(m) = core.list_messages(ListScope::Folder(f.id), 0, 50).unwrap().into_iter().find(|m| m.subject == subject) {
                    return m.id;
                }
                let _ = core.select_folder(f.id).await;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("mail {subject} not synced");
    }

    async fn wait_bob(gm: &GreenMail, n: usize, secs: u64) -> usize {
        let deadline = std::time::Instant::now() + Duration::from_secs(secs);
        loop {
            let c = gm.count_in(BOB, "INBOX").await;
            if c >= n || std::time::Instant::now() > deadline {
                return c;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reply_flow_end_to_end() {
        let gm = GreenMail::start();
        gm.append("INBOX", &eml_from_bob(1), false).await;
        let (_dir, _s, core, acc) = setup(&gm).await;
        let original = inbox_message(&core, acc, "Question 1").await;
        let d = core.create_draft(DraftKind::Reply, None, Some(original)).await.unwrap();
        assert_eq!(d.to[0].email, BOB);
        assert_eq!(d.subject, "Re: Question 1");
        assert!(d.body_html.contains("a écrit"));
        let ticket = core.send_draft(d.id).await.unwrap();
        assert!(ticket.send_at >= now_ts() + 4);
        assert_eq!(core.list_outbox().unwrap()[0].state, DraftState::Queued);
        assert_eq!(wait_bob(&gm, 1, 25).await, 1);
        let mut ok = false;
        for _ in 0..100 {
            if gm.server_flags("INBOX").await.first().is_some_and(|f| f.2) {
                ok = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        assert!(ok, "original marked answered");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reconnecting_does_not_skip_the_undo_window() {
        let gm = GreenMail::start();
        let (_dir, _s, core, acc) = setup(&gm).await;
        core.set_setting("undo_send_seconds", serde_json::json!(30)).unwrap();
        let d = core.create_draft(DraftKind::New, Some(acc), None).await.unwrap();
        core.update_draft(DraftInput { id: d.id, account_id: acc, to: vec![Address { name: None, email: BOB.into() }], cc: vec![], bcc: vec![], subject: "Délai".into(), body_html: "<p>x</p>".into() }).unwrap();
        core.send_draft(d.id).await.unwrap();
        // Coupure réseau, mise en veille, nouveau mot de passe… : le worker se reconnecte.
        core.sync_manager().restart(acc);
        tokio::time::sleep(Duration::from_secs(6)).await;
        assert_eq!(gm.count_in(BOB, "INBOX").await, 0, "nothing sent during the undo window");
        assert!(core.cancel_send(d.id).unwrap(), "still cancellable");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn restart_during_undo_window_sends_once() {
        let gm = GreenMail::start();
        let (dir, secrets, core, acc) = setup(&gm).await;
        let mut d = core.create_draft(DraftKind::New, Some(acc), None).await.unwrap();
        d.to = vec![Address { name: None, email: BOB.into() }];
        core.update_draft(DraftInput { id: d.id, account_id: acc, to: d.to.clone(), cc: vec![], bcc: vec![], subject: "Avant fermeture".into(), body_html: "<p>x</p>".into() }).unwrap();
        core.send_draft(d.id).await.unwrap();
        drop(core);
        let core2 = open(dir.path(), secrets);
        assert_eq!(wait_bob(&gm, 1, 25).await, 1);
        tokio::time::sleep(Duration::from_secs(3)).await;
        assert_eq!(gm.count_in(BOB, "INBOX").await, 1, "sent exactly once");
        drop(core2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn forward_keeps_attachments() {
        let gm = GreenMail::start();
        gm.append("INBOX", &fixture("inline_cid_and_pdf.eml"), false).await;
        let (_dir, _s, core, acc) = setup(&gm).await;
        let original = inbox_message(&core, acc, "Devis cuisine").await;
        let d = core.create_draft(DraftKind::Forward, None, Some(original)).await.unwrap();
        assert!(d.to.is_empty());
        assert_eq!(d.subject, "Fwd: Devis cuisine");
        assert_eq!(d.attachments.len(), 1);
        core.update_draft(DraftInput { id: d.id, account_id: acc, to: vec![Address { name: None, email: BOB.into() }], cc: vec![], bcc: vec![], subject: d.subject.clone(), body_html: d.body_html.clone() }).unwrap();
        core.send_draft(d.id).await.unwrap();
        assert_eq!(wait_bob(&gm, 1, 25).await, 1);
        let mut b = gm.session_as(BOB).await;
        select(&mut b, "INBOX", false).await.unwrap();
        let uids = search_all_uids(&mut b).await.unwrap();
        let h = fetch_headers(&mut b, &uids, true).await.unwrap();
        assert!(h[0].parts.iter().any(|p| p.filename.as_deref() == Some("devis.pdf")), "{:?}", h[0].parts);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_validation() {
        let gm = GreenMail::start();
        let (_dir, _s, core, acc) = setup(&gm).await;
        let d = core.create_draft(DraftKind::New, Some(acc), None).await.unwrap();
        assert!(matches!(core.send_draft(d.id).await, Err(CoreError::InvalidInput(_))));
        core.update_draft(DraftInput { id: d.id, account_id: acc, to: vec![Address { name: None, email: "pas une adresse".into() }], cc: vec![], bcc: vec![], subject: "x".into(), body_html: "<p>x</p>".into() }).unwrap();
        let r = core.send_draft(d.id).await;
        assert!(matches!(&r, Err(CoreError::InvalidInput(m)) if m == "Adresse invalide : pas une adresse"), "{:?}", r.err());
        assert_eq!(core.get_draft(d.id).unwrap().state, DraftState::Editing);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_send_restores_draft() {
        let gm = GreenMail::start();
        let (_dir, _s, core, acc) = setup(&gm).await;
        let d = core.create_draft(DraftKind::New, Some(acc), None).await.unwrap();
        core.update_draft(DraftInput { id: d.id, account_id: acc, to: vec![Address { name: None, email: BOB.into() }], cc: vec![], bcc: vec![], subject: "Oups".into(), body_html: "<p>x</p>".into() }).unwrap();
        core.send_draft(d.id).await.unwrap();
        assert!(core.cancel_send(d.id).unwrap());
        assert_eq!(core.get_draft(d.id).unwrap().state, DraftState::Editing);
        tokio::time::sleep(Duration::from_secs(7)).await;
        assert_eq!(gm.count_in(BOB, "INBOX").await, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_empty_draft_does_not_upload() {
        let gm = GreenMail::start();
        let (_dir, _s, core, acc) = setup(&gm).await;
        let empty = core.create_draft(DraftKind::New, Some(acc), None).await.unwrap();
        core.close_draft(empty.id).await.unwrap();
        assert!(matches!(core.get_draft(empty.id), Err(CoreError::NotFound)), "empty draft discarded");
        let d = core.create_draft(DraftKind::New, Some(acc), None).await.unwrap();
        core.update_draft(DraftInput { id: d.id, account_id: acc, to: vec![], cc: vec![], bcc: vec![], subject: "Idée".into(), body_html: "<p>à creuser</p>".into() }).unwrap();
        core.close_draft(d.id).await.unwrap();
        let mut n = 0;
        for _ in 0..90 {
            n = gm.count_in(USER, "Drafts").await;
            if n > 0 {
                break;
            }
            // Un brouillon fermé avant la première liste des dossiers est réessayé 30 s plus tard.
            core.store().ops_due_now(acc).unwrap();
            let _ = core.sync_manager().send(acc, illy_core::sync::WorkerRequest::OpsAvailable).await;
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        assert_eq!(n, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn attachments_are_copied_into_the_draft() {
        let gm = GreenMail::start();
        let (dir, _s, core, acc) = setup(&gm).await;
        let src = dir.path().join("rapport.pdf");
        std::fs::write(&src, b"%PDF-1.7").unwrap();
        let d = core.create_draft(DraftKind::New, Some(acc), None).await.unwrap();
        let d = core.add_attachments(d.id, vec![src.clone()]).unwrap();
        assert_eq!(d.attachments[0].filename, "rapport.pdf");
        assert_eq!(d.attachments[0].mime_type, "application/pdf");
        std::fs::remove_file(&src).unwrap();
        let AttachmentSource::File { path } = &d.attachments[0].source else { panic!() };
        assert_eq!(std::fs::read(path).unwrap(), b"%PDF-1.7", "copy survives the original's deletion");
        let d = core.remove_attachment(d.id, 0).unwrap();
        assert!(d.attachments.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn editing_a_webmail_draft_keeps_its_attachments() {
        let gm = GreenMail::start();
        let (_dir, _s, core, acc) = setup(&gm).await;
        gm.append("Drafts", &fixture("inline_cid_and_pdf.eml"), true).await;
        let m = folder_message(&core, acc, FolderRole::Drafts, "Devis cuisine").await;
        let d = core.draft_from_message(m).await.unwrap();
        assert!(!d.attachments.is_empty());
        assert!(
            d.attachments.iter().all(|a| matches!(a.source, AttachmentSource::File { .. })),
            "attachments are copied locally before the server copy is replaced"
        );
        core.close_draft(d.id).await.unwrap();
        let mut last = vec![];
        for _ in 0..100 {
            let mut s = gm.session().await;
            select(&mut s, "Drafts", false).await.unwrap();
            let uids = search_all_uids(&mut s).await.unwrap();
            last = fetch_headers(&mut s, &uids, true).await.unwrap().into_iter().map(|h| (h.uid, h.parts)).collect();
            if last.len() == 1 && last[0].0 != 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        assert_eq!(last.len(), 1, "the old copy is replaced, not duplicated");
        assert_ne!(last[0].0, 1, "the server copy was rewritten");
        assert!(last[0].1.iter().any(|p| p.filename.as_deref() == Some("devis.pdf")), "{:?}", last[0].1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reopening_an_own_server_draft_resumes_the_local_one() {
        let gm = GreenMail::start();
        let (_dir, _s, core, acc) = setup(&gm).await;
        let d = core.create_draft(DraftKind::New, Some(acc), None).await.unwrap();
        let boss = Address { name: None, email: "boss@localhost".into() };
        core.update_draft(DraftInput { id: d.id, account_id: acc, to: vec![Address { name: None, email: BOB.into() }], cc: vec![], bcc: vec![boss.clone()], subject: "Avec Cci".into(), body_html: "<p>x</p>".into() }).unwrap();
        core.close_draft(d.id).await.unwrap();
        for _ in 0..100 {
            if core.get_draft(d.id).unwrap().remote_uid.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        let m = folder_message(&core, acc, FolderRole::Drafts, "Avec Cci").await;
        let again = core.draft_from_message(m).await.unwrap();
        assert_eq!(again.id, d.id, "the local draft is reopened");
        assert_eq!(again.bcc, vec![boss], "Bcc is kept (it is never in the server copy)");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn untouched_reply_is_not_kept_and_a_second_click_reuses_it() {
        let gm = GreenMail::start();
        gm.append("INBOX", &eml_from_bob(3), false).await;
        let (_dir, _s, core, acc) = setup(&gm).await;
        let original = inbox_message(&core, acc, "Question 3").await;
        let d = core.create_draft(DraftKind::Reply, None, Some(original)).await.unwrap();
        assert_eq!(core.create_draft(DraftKind::Reply, None, Some(original)).await.unwrap().id, d.id, "same reply reopened");
        core.close_draft(d.id).await.unwrap();
        assert!(matches!(core.get_draft(d.id), Err(CoreError::NotFound)), "an accidental Reply leaves nothing behind");
        assert!(core.draft_for_message(original).unwrap().is_none());
        let d = core.create_draft(DraftKind::Reply, None, Some(original)).await.unwrap();
        core.update_draft(DraftInput { id: d.id, account_id: acc, to: d.to.clone(), cc: vec![], bcc: vec![], subject: d.subject.clone(), body_html: format!("<p>Oui !</p>{}", d.body_html) }).unwrap();
        core.close_draft(d.id).await.unwrap();
        assert_eq!(core.draft_for_message(original).unwrap().map(|x| x.id), Some(d.id), "an edited reply is kept");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn attachment_copies_never_overwrite_each_other() {
        let gm = GreenMail::start();
        let (dir, _s, core, acc) = setup(&gm).await;
        let file = |name: &str, body: &[u8]| {
            let sub = dir.path().join(format!("src-{}", body.len()));
            std::fs::create_dir_all(&sub).unwrap();
            let p = sub.join(name);
            std::fs::write(&p, body).unwrap();
            p
        };
        let d = core.create_draft(DraftKind::New, Some(acc), None).await.unwrap();
        core.add_attachments(d.id, vec![file("a.pdf", b"A"), file("b.pdf", b"BB")]).unwrap();
        core.remove_attachment(d.id, 0).unwrap();
        let d = core.add_attachments(d.id, vec![file("b.pdf", b"other b")]).unwrap();
        let paths: Vec<String> = d
            .attachments
            .iter()
            .map(|a| match &a.source {
                AttachmentSource::File { path } => path.clone(),
                _ => panic!(),
            })
            .collect();
        assert_ne!(paths[0], paths[1]);
        assert_eq!(std::fs::read(&paths[0]).unwrap(), b"BB");
        assert_eq!(std::fs::read(&paths[1]).unwrap(), b"other b");
        let d = core.remove_attachment(d.id, 1).unwrap();
        let AttachmentSource::File { path } = &d.attachments[0].source else { panic!() };
        assert_eq!(std::fs::read(path).unwrap(), b"BB", "removing one entry keeps the other's file");
    }
}
