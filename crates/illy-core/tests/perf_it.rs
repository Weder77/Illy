//! Mesures de performance sur 100 000 mails (ignoré par défaut) :
//! cargo test -p illy-core --release --test perf_it -- --ignored --nocapture

use illy_core::mime::{ParsedHeaders, SanitizedHtml};
use illy_core::model::{Address, FolderRole, ListScope, NewAccount, Provider, ServerConfig, TlsMode};
use illy_core::store::{FolderInfo, NewHeader, Store};
use std::time::{Duration, Instant};

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

fn time<T>(n: usize, mut f: impl FnMut() -> T) -> Duration {
    median((0..n).map(|_| {
        let t = Instant::now();
        std::hint::black_box(f());
        t.elapsed()
    }).collect())
}

#[test]
#[ignore]
fn perf_100k() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("illy.db")).unwrap();
    let sc = ServerConfig { host: "h".into(), port: 993, tls: TlsMode::Implicit };
    let roles = [FolderRole::Inbox, FolderRole::Sent, FolderRole::Drafts, FolderRole::Trash, FolderRole::Custom];
    let mut inboxes = vec![];
    let mut all_folders = vec![];
    for a in 0..2 {
        let acc = store
            .insert_account(&NewAccount { email: format!("a{a}@x.fr"), display_name: "A".into(), provider: Provider::Custom, imap: sc.clone(), smtp: sc.clone() })
            .unwrap();
        let infos: Vec<FolderInfo> = roles
            .iter()
            .enumerate()
            .map(|(i, r)| FolderInfo { imap_path: format!("F{i}"), display_name: format!("F{i}"), role: *r, hidden: false })
            .collect();
        store.upsert_folders(acc.id, &infos).unwrap();
        for (id, _, role) in store.syncable_folders(acc.id).unwrap() {
            all_folders.push((acc.id, id));
            if role == FolderRole::Inbox {
                inboxes.push((acc.id, id));
            }
        }
    }
    let t = Instant::now();
    // 80 000 mails dans la boîte de réception du compte 1, le reste réparti.
    let mut uid = 0u32;
    let mut insert = |acc: i64, folder: i64, n: u32| {
        for chunk in 0..n.div_ceil(5000) {
            let batch: Vec<NewHeader> = (0..5000.min(n - chunk * 5000))
                .map(|_| {
                    uid += 1;
                    NewHeader {
                        uid,
                        headers: ParsedHeaders {
                            subject: format!("Sujet numéro {uid} avec un peu de texte"),
                            from: Address { name: Some(format!("Expéditeur {}", uid % 500)), email: format!("e{}@x.fr", uid % 500) },
                            date: Some(1_700_000_000 + uid as i64 * 37),
                            message_id: Some(format!("<m{uid}@x>")),
                            ..Default::default()
                        },
                        internal_date: 0,
                        size: 4000,
                        seen: uid % 7 != 0,
                        answered: false,
                        parts: vec![],
                    }
                })
                .collect();
            store.upsert_headers(acc, folder, &batch).unwrap();
        }
    };
    insert(inboxes[0].0, inboxes[0].1, 80_000);
    for (acc, f) in all_folders.iter().filter(|x| **x != inboxes[0]) {
        insert(*acc, *f, 20_000 / 9);
    }
    eprintln!("insertion de 100 000 en-têtes : {:?}", t.elapsed());

    let inbox = ListScope::Folder(inboxes[0].1);
    let ids: Vec<i64> = store.list_messages(inbox, 0, 10_000).unwrap().iter().map(|m| m.id).collect();
    for id in ids.iter().step_by(1) {
        let san = SanitizedHtml { html: "<p>Bonjour</p>".repeat(50), has_remote_images: true };
        store.store_body(*id, Some("Bonjour"), Some(&san), "Bonjour", &[], false).unwrap();
    }
    assert_eq!(store.count_messages(ListScope::UnifiedInbox).unwrap(), 80_000 + 20_000 / 9);

    let folder_switch = time(20, || (store.count_messages(inbox).unwrap(), store.list_messages(inbox, 0, 100).unwrap()));
    let deep_page = time(20, || store.list_messages(inbox, 70_000, 100).unwrap());
    let unified = time(20, || (store.count_messages(ListScope::UnifiedInbox).unwrap(), store.list_messages(ListScope::UnifiedInbox, 0, 100).unwrap()));
    let detail = time(20, || store.message_detail(ids[5], false).unwrap());
    let folders = time(20, || store.list_folders(inboxes[0].0).unwrap());
    eprintln!("changement de boîte : {folder_switch:?}");
    eprintln!("page profonde (70 000) : {deep_page:?}");
    eprintln!("vue unifiée : {unified:?}");
    eprintln!("détail d'un mail : {detail:?}");
    eprintln!("liste des dossiers (compteurs) : {folders:?}");
    assert!(folder_switch < Duration::from_millis(150), "changement de boîte {folder_switch:?}");
    assert!(unified < Duration::from_millis(150), "vue unifiée {unified:?}");
    assert!(detail < Duration::from_millis(20), "détail {detail:?}");
    assert!(deep_page < Duration::from_millis(150), "page profonde {deep_page:?}");
    assert!(folders < Duration::from_millis(50), "dossiers {folders:?}");
}
