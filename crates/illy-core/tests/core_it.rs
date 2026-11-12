#![cfg(feature = "insecure-plain")]

mod common;

use common::*;
use illy_core::events::RecordingSink;
use illy_core::model::{AccountPatch, AccountStatus, Address, DraftInput, DraftKind, FolderRole, ListScope, NewAccount, ServerConfig};
use illy_core::secrets::{MemorySecretStore, SecretStore};
use illy_core::sync::WorkerConfig;
use illy_core::{Core, CoreError};
use std::sync::Arc;
use std::time::Duration;

struct Env {
    gm: GreenMail,
    core: Core,
    secrets: Arc<MemorySecretStore>,
    tmp: tempfile::TempDir,
}

fn open_core(dir: &std::path::Path, secrets: Arc<MemorySecretStore>) -> Core {
    let cfg = WorkerConfig { poll_interval: Duration::from_secs(1), backoff_min: Duration::from_millis(200), ..Default::default() };
    Core::open(
        dir,
        secrets,
        Arc::new(RecordingSink::default()),
        cfg,
        tokio::runtime::Handle::current(),
    )
    .unwrap()
}

fn env(gm: GreenMail) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let core = open_core(tmp.path(), secrets.clone());
    Env { gm, core, secrets, tmp }
}

fn inbox(core: &Core, acc: i64) -> Option<i64> {
    core.list_folders(acc).ok()?.into_iter().find(|f| f.role == FolderRole::Inbox).map(|f| f.id)
}

fn inbox_count(core: &Core, acc: i64) -> u32 {
    inbox(core, acc).map(|f| core.count_messages(ListScope::Folder(f)).unwrap()).unwrap_or(0)
}

async fn added(e: &Env) -> i64 {
    let a = e.core.add_account(e.gm.new_account(), PASSWORD.into()).await.unwrap();
    a.id
}

#[tokio::test(flavor = "multi_thread")]
async fn add_account_ok() {
    let gm = GreenMail::start();
    gm.append_many("INBOX", 1, 2).await;
    let e = env(gm);
    let id = added(&e).await;
    assert_eq!(e.secrets.get(id).unwrap().as_deref(), Some(PASSWORD));
    assert!(wait_until(10, || inbox_count(&e.core, id) == 2).await);
    assert_eq!(e.core.list_accounts().unwrap()[0].email, USER);
}

#[tokio::test(flavor = "multi_thread")]
async fn add_account_wrong_password_saves_nothing() {
    let gm = GreenMail::start();
    let e = env(gm);
    let r = e.core.add_account(e.gm.new_account(), "wrong".into()).await;
    assert!(matches!(r, Err(CoreError::Auth(_))), "{:?}", r.err());
    assert!(e.core.list_accounts().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn add_account_rejects_bad_email() {
    let gm = GreenMail::start();
    let e = env(gm);
    let bad = NewAccount { email: "pasunemail".into(), ..e.gm.new_account() };
    assert!(matches!(e.core.add_account(bad, PASSWORD.into()).await, Err(CoreError::InvalidInput(_))));
}

#[tokio::test(flavor = "multi_thread")]
async fn add_account_default_display_name() {
    let gm = GreenMail::start();
    let e = env(gm);
    let a = e.core.add_account(NewAccount { display_name: "  ".into(), ..e.gm.new_account() }, PASSWORD.into()).await.unwrap();
    assert_eq!(a.display_name, "alice");
}

#[tokio::test(flavor = "multi_thread")]
async fn smtp_test_checks_password() {
    let gm = GreenMail::start();
    illy_core::smtp::test_smtp(&gm.smtp(), USER, PASSWORD).await.unwrap();
    let r = illy_core::smtp::test_smtp(&gm.smtp(), USER, "wrong").await;
    assert!(matches!(r, Err(CoreError::Auth(_))), "{r:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn open_message_marks_seen_everywhere() {
    let gm = GreenMail::start();
    gm.append("INBOX", &fixture("inline_cid_and_pdf.eml"), false).await;
    let e = env(gm);
    let id = added(&e).await;
    assert!(wait_until(10, || inbox_count(&e.core, id) == 1).await);
    let f = inbox(&e.core, id).unwrap();
    let m = e.core.list_messages(ListScope::Folder(f), 0, 1).unwrap()[0].clone();
    assert!(!m.seen);
    let d = e.core.open_message(m.id, false).await.unwrap();
    assert!(d.summary.seen);
    assert!(d.images_blocked);
    assert!(e.core.list_messages(ListScope::Folder(f), 0, 1).unwrap()[0].seen);
    let mut ok = false;
    for _ in 0..50 {
        if e.gm.server_flags("INBOX").await == vec![(1, true, false)] {
            ok = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ok, "seen flag never reached the server");
    let shown = e.core.open_message(m.id, true).await.unwrap();
    assert!(!shown.images_blocked);
}

#[tokio::test(flavor = "multi_thread")]
async fn open_message_offline_then_recovers() {
    let gm = GreenMail::start();
    gm.append_many("INBOX", 1, 1).await;
    let e = env(gm);
    let id = added(&e).await;
    assert!(wait_until(10, || inbox_count(&e.core, id) == 1).await);
    let f = inbox(&e.core, id).unwrap();
    let m = e.core.list_messages(ListScope::Folder(f), 0, 1).unwrap()[0].clone();
    // Corps préchargé par le worker.
    assert!(wait_until(10, || e.core.store().message_detail(m.id, false).unwrap().is_some()).await);
    e.core.sync_manager().stop(id);
    let d = e.core.open_message(m.id, false).await.unwrap();
    assert!(d.summary.seen);
    assert_eq!(e.core.store().pending_ops_count(id).unwrap(), 1);
    assert_eq!(e.gm.server_flags("INBOX").await, vec![(1, false, false)]);
    e.core.sync_manager().start(id);
    let mut ok = false;
    for _ in 0..80 {
        if e.gm.server_flags("INBOX").await == vec![(1, true, false)] {
            ok = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ok);
    assert!(wait_until(5, || e.core.store().pending_ops_count(id).unwrap() == 0).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn attachments_open_and_exe_refused() {
    let gm = GreenMail::start();
    gm.append("INBOX", &fixture("inline_cid_and_pdf.eml"), false).await;
    gm.append("INBOX", &fixture("exe_attachment.eml"), false).await;
    let e = env(gm);
    let id = added(&e).await;
    assert!(wait_until(10, || inbox_count(&e.core, id) == 2).await);
    let f = inbox(&e.core, id).unwrap();
    for m in e.core.list_messages(ListScope::Folder(f), 0, 10).unwrap() {
        let d = e.core.open_message(m.id, false).await.unwrap();
        let att = &d.attachments[0];
        if att.filename == "setup.exe" {
            assert!(!att.can_open);
            let r = e.core.attachment_to_tmp(m.id, &att.part_path).await;
            assert!(matches!(r, Err(CoreError::InvalidInput(_))), "{r:?}");
            let dest = e.tmp.path().join("saved.exe");
            e.core.save_attachment(m.id, &att.part_path, &dest).await.unwrap();
            assert_eq!(std::fs::read(dest).unwrap(), vec![0x4d, 0x5a, 0x90, 0x00]);
        } else {
            let p = e.core.attachment_to_tmp(m.id, &att.part_path).await.unwrap();
            assert!(p.starts_with(e.tmp.path().join("tmp")));
            assert!(std::fs::read(&p).unwrap().starts_with(b"%PDF"));
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn open_cleans_tmp_and_restarts_workers() {
    let gm = GreenMail::start();
    gm.append_many("INBOX", 1, 1).await;
    let tmp = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let id = {
        let core = open_core(tmp.path(), secrets.clone());
        core.add_account(gm.new_account(), PASSWORD.into()).await.unwrap().id
    };
    std::fs::write(tmp.path().join("tmp").join("old.pdf"), b"x").unwrap();
    let core = open_core(tmp.path(), secrets.clone());
    assert!(!tmp.path().join("tmp").join("old.pdf").exists());
    assert!(wait_until(10, || core.list_accounts().unwrap()[0].status == AccountStatus::Ok).await);
    assert!(wait_until(10, || inbox_count(&core, id) == 1).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn update_password_and_remove_account() {
    let gm = GreenMail::start();
    gm.append_many("INBOX", 1, 1).await;
    let e = env(gm);
    let id = added(&e).await;
    assert!(matches!(e.core.update_password(id, "wrong".into()).await, Err(CoreError::Auth(_))));
    assert_eq!(e.secrets.get(id).unwrap().as_deref(), Some(PASSWORD), "bad password must not replace the good one");
    e.core.update_password(id, PASSWORD.into()).await.unwrap();
    e.core.remove_account(id).unwrap();
    assert!(e.core.list_accounts().unwrap().is_empty());
    assert_eq!(e.secrets.get(id).unwrap(), None);
    assert_eq!(e.core.count_messages(ListScope::UnifiedInbox).unwrap(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn offline_read_never_reverts_on_reconnect() {
    let gm = GreenMail::start();
    gm.append_many("INBOX", 1, 1).await;
    let e = env(gm);
    let id = added(&e).await;
    assert!(wait_until(10, || inbox_count(&e.core, id) == 1).await);
    let f = inbox(&e.core, id).unwrap();
    let m = e.core.list_messages(ListScope::Folder(f), 0, 1).unwrap()[0].clone();
    assert!(wait_until(10, || e.core.store().message_detail(m.id, false).unwrap().is_some()).await);
    e.core.sync_manager().stop(id);
    e.core.open_message(m.id, false).await.unwrap();
    e.core.sync_manager().start(id);
    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    while std::time::Instant::now() < deadline {
        let seen = e.core.list_messages(ListScope::Folder(f), 0, 1).unwrap()[0].seen;
        assert!(seen, "the mail opened offline went back to unread during the reconnect sync");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(e.gm.server_flags("INBOX").await, vec![(1, true, false)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn update_account_validates() {
    let gm = GreenMail::start();
    let e = env(gm);
    let id = added(&e).await;
    let empty = AccountPatch { display_name: Some("   ".into()), color: None, signature_html: None };
    assert!(matches!(e.core.update_account(id, empty), Err(CoreError::InvalidInput(_))));
    let bad_color = AccountPatch { display_name: None, color: Some("blue".into()), signature_html: None };
    assert!(matches!(e.core.update_account(id, bad_color), Err(CoreError::InvalidInput(_))));
    let sig = AccountPatch { display_name: Some(" Alice M. ".into()), color: Some("#EC4899".into()), signature_html: Some("<p><b>Alice</b></p><script>alert(1)</script>".into()) };
    let a = e.core.update_account(id, sig).unwrap();
    assert_eq!(a.display_name, "Alice M.");
    assert_eq!(a.color, "#EC4899");
    assert!(a.signature_html.contains("<b>Alice</b>") && !a.signature_html.contains("script"), "{}", a.signature_html);
}

#[tokio::test(flavor = "multi_thread")]
async fn update_servers_with_a_bad_host_saves_nothing() {
    let gm = GreenMail::start();
    let e = env(gm);
    let id = added(&e).await;
    let before = e.core.list_accounts().unwrap()[0].clone();
    let bad = ServerConfig { host: "127.0.0.1".into(), port: 1, tls: before.imap.tls };
    assert!(e.core.update_servers(id, bad.clone(), before.smtp.clone()).await.is_err());
    assert!(e.core.update_servers(id, before.imap.clone(), bad).await.is_err());
    let after = e.core.list_accounts().unwrap()[0].clone();
    assert_eq!((after.imap, after.smtp), (before.imap.clone(), before.smtp.clone()), "nothing saved");
    let ok = e.core.update_servers(id, before.imap.clone(), before.smtp.clone()).await.unwrap();
    assert_eq!(ok.imap, before.imap);
    assert!(wait_until(10, || e.core.list_accounts().unwrap()[0].status == AccountStatus::Ok).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn servers_of_gmail_cannot_be_changed() {
    let gm = GreenMail::start();
    let e = env(gm);
    let mut input = e.gm.new_account();
    input.provider = illy_core::model::Provider::Gmail;
    let id = e.core.add_account(input, PASSWORD.into()).await.unwrap().id;
    let a = e.core.list_accounts().unwrap()[0].clone();
    assert!(matches!(e.core.update_servers(id, a.imap, a.smtp).await, Err(CoreError::InvalidInput(_))));
}

#[tokio::test(flavor = "multi_thread")]
async fn remove_account_cleans_drafts_and_ops() {
    let gm = GreenMail::start();
    let e = env(gm);
    let id = added(&e).await;
    let d = e.core.create_draft(DraftKind::New, Some(id), None).await.unwrap();
    let file = e.tmp.path().join("plan.pdf");
    std::fs::write(&file, b"%PDF").unwrap();
    e.core.add_attachments(d.id, vec![file]).unwrap();
    e.core
        .update_draft(DraftInput { id: d.id, account_id: id, to: vec![Address { name: None, email: BOB.into() }], cc: vec![], bcc: vec![], subject: "x".into(), body_html: "<p>x</p>".into() })
        .unwrap();
    e.core.set_setting("undo_send_seconds", serde_json::json!(30)).unwrap();
    e.core.send_draft(d.id).await.unwrap();
    let draft_dir = e.tmp.path().join("drafts").join(d.id.to_string());
    assert!(draft_dir.exists());
    e.core.remove_account(id).unwrap();
    assert!(e.core.list_outbox().unwrap().is_empty());
    assert!(!draft_dir.exists(), "draft files removed");
    assert_eq!(e.core.store().pending_ops_count(id).unwrap(), 0);
    assert!(matches!(e.core.get_draft(d.id), Err(CoreError::NotFound)));
}

