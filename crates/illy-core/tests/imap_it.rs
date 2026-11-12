#![cfg(feature = "insecure-plain")]

mod common;

use common::*;
use illy_core::imap::*;
use illy_core::mime::{decode_part, synth_mime_header, Decoded, PartKind};
use illy_core::model::{ServerConfig, TlsMode};
use illy_core::CoreError;

#[tokio::test]
async fn login_ok_and_caps() {
    let gm = GreenMail::start();
    let (_s, caps) = connect(&gm.imap(), USER, PASSWORD).await.unwrap();
    eprintln!("GreenMail caps: {caps:?}");
    assert!(caps.idle);
}

#[tokio::test]
async fn login_wrong_password_is_auth() {
    let gm = GreenMail::start();
    let r = connect(&gm.imap(), USER, "wrong").await;
    assert!(matches!(r, Err(CoreError::Auth(_))), "{:?}", r.err());
}

#[tokio::test]
async fn unreachable_is_network() {
    let cfg = ServerConfig { host: "127.0.0.1".into(), port: 1, tls: TlsMode::Plain };
    assert!(matches!(connect(&cfg, "u", "p").await, Err(CoreError::Network(_))));
}

#[tokio::test]
async fn tls_to_plain_server_is_tls_error() {
    let gm = GreenMail::start();
    let cfg = ServerConfig { tls: TlsMode::Implicit, host: "localhost".into(), ..gm.imap() };
    let r = connect(&cfg, USER, PASSWORD).await;
    assert!(matches!(r, Err(CoreError::Tls(_)) | Err(CoreError::Network(_))), "{:?}", r.err());
}

#[tokio::test]
async fn list_has_inbox_and_created_folder() {
    let gm = GreenMail::start();
    gm.append("Factures", &simple_eml(1), false).await;
    let mut s = gm.session().await;
    let folders = list_folders(&mut s).await.unwrap();
    let names: Vec<_> = folders.iter().map(|f| f.display_name.as_str()).collect();
    assert!(names.contains(&"Boîte de réception"), "{names:?}");
    assert!(names.contains(&"Factures"), "{names:?}");
}

#[tokio::test]
async fn headers_and_part_roundtrip() {
    let gm = GreenMail::start();
    gm.append("INBOX", &fixture("inline_cid_and_pdf.eml"), false).await;
    let mut s = gm.session().await;
    let sel = select(&mut s, "INBOX", false).await.unwrap();
    assert!(sel.uidvalidity > 0);
    let uids = search_all_uids(&mut s).await.unwrap();
    assert_eq!(uids.len(), 1);
    let h = fetch_headers(&mut s, &uids, true).await.unwrap();
    assert_eq!(h.len(), 1);
    assert_eq!(h[0].headers.subject, "Devis cuisine");
    assert_eq!(h[0].headers.from.email, "marie@exemple.fr");
    assert!(!h[0].seen);
    let kinds: Vec<_> = h[0].parts.iter().map(|p| (p.part_path.as_str(), p.kind)).collect();
    assert_eq!(kinds, [("1.1", PartKind::BodyHtml), ("1.2", PartKind::Inline), ("2", PartKind::Attachment)]);
    let body = fetch_part(&mut s, uids[0], "1.1").await.unwrap();
    match decode_part(&synth_mime_header(&h[0].parts[0]), &body) {
        Decoded::Text(t) => assert!(t.contains("cid:logo@x"), "{t}"),
        other => panic!("{other:?}"),
    }
    let body = fetch_part(&mut s, uids[0], "1.2").await.unwrap();
    assert_eq!(decode_part(&synth_mime_header(&h[0].parts[1]), &body), Decoded::Bytes(vec![1, 2, 3]));
    let flags = fetch_flags(&mut s, &uid_set(&uids), None).await.unwrap();
    assert_eq!(flags, vec![(uids[0], false, false)], "fetching parts must not mark as read");
}

#[tokio::test]
async fn single_part_message_part_one() {
    let gm = GreenMail::start();
    gm.append("INBOX", &fixture("plain_windows1252.eml"), false).await;
    let mut s = gm.session().await;
    select(&mut s, "INBOX", false).await.unwrap();
    let uids = search_all_uids(&mut s).await.unwrap();
    let h = fetch_headers(&mut s, &uids, true).await.unwrap();
    let body = fetch_part(&mut s, uids[0], "1").await.unwrap();
    match decode_part(&synth_mime_header(&h[0].parts[0]), &body) {
        Decoded::Text(t) => assert_eq!(t.trim_end(), "Votre café vous attend."),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn store_seen_sets_flag() {
    let gm = GreenMail::start();
    gm.append("INBOX", &simple_eml(1), false).await;
    let mut s = gm.session().await;
    select(&mut s, "INBOX", false).await.unwrap();
    let uids = search_all_uids(&mut s).await.unwrap();
    store_seen(&mut s, uids[0], true).await.unwrap();
    assert_eq!(fetch_flags(&mut s, &uid_set(&uids), None).await.unwrap(), vec![(uids[0], true, false)]);
    store_seen(&mut s, uids[0], false).await.unwrap();
    assert_eq!(fetch_flags(&mut s, &uid_set(&uids), None).await.unwrap(), vec![(uids[0], false, false)]);
}

#[tokio::test]
async fn missing_message_part_is_not_found() {
    let gm = GreenMail::start();
    let mut s = gm.session().await;
    select(&mut s, "INBOX", false).await.unwrap();
    assert!(matches!(fetch_part(&mut s, 999, "1").await, Err(CoreError::NotFound)));
}

#[tokio::test]
async fn broken_structure_is_parse_error_then_safe_fetch_works() {
    let gm = GreenMail::start();
    gm.append("INBOX", &fixture("broken_mime.eml"), false).await;
    let mut s = gm.session().await;
    select(&mut s, "INBOX", false).await.unwrap();
    let uids = search_all_uids(&mut s).await.unwrap();
    let r = fetch_headers(&mut s, &uids, true).await;
    assert!(matches!(r, Err(CoreError::Parse(_))), "{:?}", r.err());
    let mut s = gm.session().await;
    select(&mut s, "INBOX", false).await.unwrap();
    let h = fetch_headers(&mut s, &uids, false).await.unwrap();
    assert_eq!(h.len(), 1);
    assert!(h[0].parts.is_empty());
    let raw = fetch_full(&mut s, uids[0]).await.unwrap();
    assert!(raw.starts_with(b"From: broken"), "{:?}", String::from_utf8_lossy(&raw));
}

#[tokio::test]
async fn untrusted_certificate_is_refused() {
    let gm = GreenMail::start();
    // GreenMail présente un certificat auto-signé : il ne doit jamais être accepté.
    let cfg = ServerConfig { host: "localhost".into(), port: gm.imaps_port, tls: TlsMode::Implicit };
    let r = connect(&cfg, USER, PASSWORD).await;
    assert!(matches!(r, Err(CoreError::Tls(_))), "{:?}", r.err());
}
