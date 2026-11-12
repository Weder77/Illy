#![cfg(feature = "insecure-plain")]

mod common;

use common::*;
use illy_core::events::{CoreEvent, RecordingSink};
use illy_core::imap::Caps;
use illy_core::model::ListScope;
use illy_core::store::FolderSyncState;
use illy_core::sync::*;

fn cfg(batch: u32) -> WorkerConfig {
    WorkerConfig { header_batch: batch, ..Default::default() }
}

#[tokio::test]
async fn initial_sync_newest_first_with_backlog() {
    let gm = GreenMail::start();
    for n in 1..=7 {
        gm.append("INBOX", &simple_eml(n), false).await;
    }
    let (store, acc, inbox) = gm.store_with_folders().await;
    let sink = RecordingSink::default();
    let mut s = gm.session().await;
    let mut backlog = sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(3), true).await.unwrap();
    assert_eq!(store.known_uids(inbox).unwrap(), vec![5, 6, 7]);
    assert_eq!(backlog.pending_desc, vec![4, 3, 2, 1]);
    assert!(sink.events().contains(&CoreEvent::FolderChanged { account_id: acc, folder_id: inbox }));
    let newest = store.list_messages(ListScope::Folder(inbox), 0, 1).unwrap();
    assert_eq!(newest[0].subject, "Mail 7");
    backfill_step(&mut s, &store, &sink, acc, "INBOX", &mut backlog, &cfg(3), true).await.unwrap();
    backfill_step(&mut s, &store, &sink, acc, "INBOX", &mut backlog, &cfg(3), true).await.unwrap();
    assert_eq!(store.count_messages(ListScope::Folder(inbox)).unwrap(), 7);
    assert!(backlog.pending_desc.is_empty());
}

#[tokio::test]
async fn incremental_new_and_deleted() {
    let gm = GreenMail::start();
    for n in 1..=3 {
        gm.append("INBOX", &simple_eml(n), false).await;
    }
    let (store, acc, inbox) = gm.store_with_folders().await;
    let sink = RecordingSink::default();
    let mut s = gm.session().await;
    sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(500), true).await.unwrap();
    gm.append("INBOX", &simple_eml(4), false).await;
    gm.delete_on_server("INBOX", 1).await;
    let mut s = gm.session().await;
    sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(500), true).await.unwrap();
    assert_eq!(store.known_uids(inbox).unwrap(), vec![2, 3, 4]);
}

#[tokio::test]
async fn flags_from_elsewhere() {
    let gm = GreenMail::start();
    for n in 1..=2 {
        gm.append("INBOX", &simple_eml(n), false).await;
    }
    let (store, acc, inbox) = gm.store_with_folders().await;
    let sink = RecordingSink::default();
    let mut s = gm.session().await;
    sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(500), true).await.unwrap();
    assert_eq!(store.list_folders(acc).unwrap()[0].unread_count, 2);
    gm.mark_seen_on_server("INBOX", 1).await;
    sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(500), true).await.unwrap();
    assert_eq!(store.list_folders(acc).unwrap()[0].unread_count, 1);
}

#[tokio::test]
async fn uidvalidity_reset() {
    let gm = GreenMail::start();
    for n in 1..=3 {
        gm.append("INBOX", &simple_eml(n), false).await;
    }
    let (store, acc, inbox) = gm.store_with_folders().await;
    let sink = RecordingSink::default();
    let mut s = gm.session().await;
    sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(500), true).await.unwrap();
    let st = store.folder_state(inbox).unwrap();
    store.set_folder_state(inbox, &FolderSyncState { uidvalidity: Some(1), ..st }).unwrap();
    sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(500), true).await.unwrap();
    assert_eq!(store.count_messages(ListScope::Folder(inbox)).unwrap(), 3);
    assert_ne!(store.folder_state(inbox).unwrap().uidvalidity, Some(1));
}

mod bodies {
    use super::*;
    use illy_core::model::MessageBody;
    use illy_core::CoreError;

    async fn synced(gm: &GreenMail, fixture_name: &str) -> (illy_core::store::Store, i64, i64, illy_core::imap::ImapSession) {
        gm.append("INBOX", &fixture(fixture_name), false).await;
        let (store, acc, inbox) = gm.store_with_folders().await;
        let mut s = gm.session().await;
        sync_folder(&mut s, &Caps::default(), &store, &RecordingSink::default(), acc, inbox, "INBOX", &cfg(500), true).await.unwrap();
        let id = store.list_messages(ListScope::Folder(inbox), 0, 1).unwrap()[0].id;
        (store, inbox, id, s)
    }

    #[tokio::test]
    async fn body_html_with_cid_is_inlined() {
        let gm = GreenMail::start();
        let (store, _, id, mut s) = synced(&gm, "inline_cid_and_pdf.eml").await;
        let sink = RecordingSink::default();
        fetch_body(&mut s, &store, &sink, id).await.unwrap();
        let d = store.message_detail(id, false).unwrap().unwrap();
        match &d.body {
            MessageBody::Html(h) => {
                assert!(h.contains("data:image/png;base64,AQID"), "{h}");
                assert!(h.contains("data-blocked-src"), "{h}");
            }
            other => panic!("{other:?}"),
        }
        assert!(d.images_blocked);
        assert_eq!(d.attachments.len(), 1);
        assert_eq!(d.attachments[0].filename, "devis.pdf");
        assert!(d.attachments[0].can_open);
        assert!(!d.degraded);
        assert!(d.summary.snippet.starts_with("Bonjour, voici le devis."), "{}", d.summary.snippet);
        assert!(sink.events().contains(&CoreEvent::MessageUpdated { message_id: id }));
    }

    #[tokio::test]
    async fn body_plain_windows1252() {
        let gm = GreenMail::start();
        let (store, _, id, mut s) = synced(&gm, "plain_windows1252.eml").await;
        fetch_body(&mut s, &store, &RecordingSink::default(), id).await.unwrap();
        match store.message_detail(id, false).unwrap().unwrap().body {
            MessageBody::Text(t) => assert!(t.contains("café"), "{t}"),
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn alternative_prefers_html_and_text_snippet() {
        let gm = GreenMail::start();
        let (store, _, id, mut s) = synced(&gm, "alternative_html.eml").await;
        fetch_body(&mut s, &store, &RecordingSink::default(), id).await.unwrap();
        let d = store.message_detail(id, false).unwrap().unwrap();
        assert!(matches!(&d.body, MessageBody::Html(h) if h.contains("<b>HTML</b>")));
        assert_eq!(d.summary.snippet, "Version texte.");
    }

    #[tokio::test]
    async fn broken_structure_falls_back_to_full_message() {
        let gm = GreenMail::start();
        gm.append("INBOX", &fixture("broken_mime.eml"), false).await;
        gm.append("INBOX", &fixture("inline_cid_and_pdf.eml"), false).await;
        let (store, acc, inbox) = gm.store_with_folders().await;
        let sink = RecordingSink::default();
        let mut s = gm.session().await;
        let r = sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(500), true).await;
        assert!(matches!(r, Err(CoreError::Parse(_))), "{r:?}");
        let mut s = gm.session().await;
        sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(500), false).await.unwrap();
        assert_eq!(store.count_messages(ListScope::Folder(inbox)).unwrap(), 2);
        for m in store.list_messages(ListScope::Folder(inbox), 0, 10).unwrap() {
            fetch_body(&mut s, &store, &sink, m.id).await.unwrap();
            let d = store.message_detail(m.id, false).unwrap().unwrap();
            if m.subject == "Devis cuisine" {
                assert!(matches!(&d.body, MessageBody::Html(h) if h.contains("data:image/png;base64,AQID")));
                assert_eq!(d.attachments.len(), 1);
                assert_eq!(d.attachments[0].filename, "devis.pdf");
                assert!(d.summary.has_attachments);
                let pdf = fetch_attachment(&mut s, &store, m.id, &d.attachments[0].part_path).await.unwrap();
                assert!(pdf.starts_with(b"%PDF"));
            }
        }
    }

    #[tokio::test]
    async fn fetch_body_does_not_mark_seen() {
        let gm = GreenMail::start();
        let (store, _, id, mut s) = synced(&gm, "inline_cid_and_pdf.eml").await;
        fetch_body(&mut s, &store, &RecordingSink::default(), id).await.unwrap();
        assert!(gm.server_flags("INBOX").await.iter().all(|f| !f.1));
    }

    #[tokio::test]
    async fn attachment_bytes_and_cache() {
        let gm = GreenMail::start();
        let (store, _, id, mut s) = synced(&gm, "inline_cid_and_pdf.eml").await;
        let pdf = fetch_attachment(&mut s, &store, id, "2").await.unwrap();
        assert!(pdf.starts_with(b"%PDF-1.4"), "{pdf:?}");
        drop(s);
        assert_eq!(store.part(id, "2").unwrap().1, Some(pdf));
    }

    #[tokio::test]
    async fn vanished_message_not_found() {
        let gm = GreenMail::start();
        let (store, inbox, id, _s) = synced(&gm, "plain_windows1252.eml").await;
        gm.delete_on_server("INBOX", 1).await;
        let mut s = gm.session().await;
        let r = fetch_body(&mut s, &store, &RecordingSink::default(), id).await;
        assert!(matches!(r, Err(CoreError::NotFound)), "{r:?}");
        assert_eq!(store.count_messages(ListScope::Folder(inbox)).unwrap(), 0);
    }
}
