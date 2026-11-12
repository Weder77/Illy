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
    let mut backlog = sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(3)).await.unwrap();
    assert_eq!(store.known_uids(inbox).unwrap(), vec![5, 6, 7]);
    assert_eq!(backlog.pending_desc, vec![4, 3, 2, 1]);
    assert!(sink.events().contains(&CoreEvent::FolderChanged { account_id: acc, folder_id: inbox }));
    let newest = store.list_messages(ListScope::Folder(inbox), 0, 1).unwrap();
    assert_eq!(newest[0].subject, "Mail 7");
    backfill_step(&mut s, &store, &sink, acc, "INBOX", &mut backlog, &cfg(3)).await.unwrap();
    backfill_step(&mut s, &store, &sink, acc, "INBOX", &mut backlog, &cfg(3)).await.unwrap();
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
    sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(500)).await.unwrap();
    gm.append("INBOX", &simple_eml(4), false).await;
    gm.delete_on_server("INBOX", 1).await;
    let mut s = gm.session().await;
    sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(500)).await.unwrap();
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
    sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(500)).await.unwrap();
    assert_eq!(store.list_folders(acc).unwrap()[0].unread_count, 2);
    gm.mark_seen_on_server("INBOX", 1).await;
    sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(500)).await.unwrap();
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
    sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(500)).await.unwrap();
    let st = store.folder_state(inbox).unwrap();
    store.set_folder_state(inbox, &FolderSyncState { uidvalidity: Some(1), ..st }).unwrap();
    sync_folder(&mut s, &Caps::default(), &store, &sink, acc, inbox, "INBOX", &cfg(500)).await.unwrap();
    assert_eq!(store.count_messages(ListScope::Folder(inbox)).unwrap(), 3);
    assert_ne!(store.folder_state(inbox).unwrap().uidvalidity, Some(1));
}
