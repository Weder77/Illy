//! Harnais de tests d'intégration : serveur GreenMail local (IMAP + SMTP sans TLS).
#![allow(dead_code)]

use illy_core::imap::{connect, ImapSession};
use illy_core::model::{ServerConfig, TlsMode};
use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub const USER: &str = "alice@localhost";
pub const PASSWORD: &str = "secret";
pub const EMAIL: &str = "alice@localhost";

pub struct GreenMail {
    child: Child,
    pub imap_port: u16,
    pub smtp_port: u16,
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn jar() -> String {
    format!("{}/../../target/greenmail/greenmail-standalone.jar", env!("CARGO_MANIFEST_DIR"))
}

impl GreenMail {
    pub fn start() -> GreenMail {
        let (imap_port, smtp_port) = (free_port(), free_port());
        let child = Command::new("java")
            .arg(format!("-Dgreenmail.imap.port={imap_port}"))
            .arg("-Dgreenmail.imap.hostname=127.0.0.1")
            .arg(format!("-Dgreenmail.smtp.port={smtp_port}"))
            .arg("-Dgreenmail.smtp.hostname=127.0.0.1")
            .arg(format!("-Dgreenmail.users=alice:{PASSWORD}@localhost"))
            .arg("-Dgreenmail.users.login=email")
            .arg("-jar")
            .arg(jar())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("java + target/greenmail/greenmail-standalone.jar (scripts/fetch-greenmail.sh)");
        let gm = GreenMail { child, imap_port, smtp_port };
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Ok(s) = TcpStream::connect(("127.0.0.1", imap_port)) {
                let mut line = String::new();
                s.set_read_timeout(Some(Duration::from_secs(2))).ok();
                if BufReader::new(s).read_line(&mut line).is_ok() && line.starts_with("* OK")
                    && TcpStream::connect(("127.0.0.1", smtp_port)).is_ok()
                {
                    return gm;
                }
            }
            assert!(Instant::now() < deadline, "GreenMail n'a pas démarré");
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn imap(&self) -> ServerConfig {
        ServerConfig { host: "127.0.0.1".into(), port: self.imap_port, tls: TlsMode::Plain }
    }

    pub fn smtp(&self) -> ServerConfig {
        ServerConfig { host: "127.0.0.1".into(), port: self.smtp_port, tls: TlsMode::Plain }
    }

    pub async fn session(&self) -> ImapSession {
        connect(&self.imap(), USER, PASSWORD).await.expect("login").0
    }

    /// Dépose un mail dans un dossier (créé si besoin) via une session séparée.
    pub async fn append(&self, folder: &str, eml: &[u8], seen: bool) {
        let mut s = self.session().await;
        if !folder.eq_ignore_ascii_case("INBOX") {
            let _ = s.create(folder).await;
        }
        s.append(folder, if seen { Some("(\\Seen)") } else { None }, None, eml).await.expect("append");
        let _ = s.logout().await;
    }
}

impl Drop for GreenMail {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// Mail texte minimal ; `n` sert d'identifiant et fixe la date (n minutes après une base).
pub fn simple_eml(n: u32) -> Vec<u8> {
    let minute = n % 60;
    let hour = (n / 60) % 24;
    let day = 1 + (n / 1440) % 28;
    format!(
        "From: Expéditeur {n} <exp{n}@exemple.fr>\r\nTo: {EMAIL}\r\nSubject: Mail {n}\r\n\
         Date: {day:02} Oct 2026 {hour:02}:{minute:02}:00 +0000\r\nMessage-ID: <m{n}@exemple.fr>\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\r\nCorps du mail {n}.\r\n"
    )
    .into_bytes()
}

use illy_core::model::{AccountId, FolderId, FolderRole, NewAccount, Provider};
use illy_core::store::Store;

impl GreenMail {
    pub fn new_account(&self) -> NewAccount {
        NewAccount {
            email: USER.into(),
            display_name: "Alice".into(),
            provider: Provider::Custom,
            imap: self.imap(),
            smtp: self.smtp(),
        }
    }

    /// Base en mémoire avec le compte GreenMail et ses dossiers ; renvoie l'id de la boîte de réception.
    pub async fn store_with_folders(&self) -> (Store, AccountId, FolderId) {
        let store = Store::open_in_memory().unwrap();
        let acc = store.insert_account(&self.new_account()).unwrap();
        let mut s = self.session().await;
        let folders = illy_core::imap::list_folders(&mut s).await.unwrap();
        store.upsert_folders(acc.id, &folders).unwrap();
        let inbox = store.syncable_folders(acc.id).unwrap().into_iter().find(|f| f.2 == FolderRole::Inbox).unwrap().0;
        (store, acc.id, inbox)
    }

    /// Supprime définitivement un mail côté serveur (helper « autre client »).
    pub async fn delete_on_server(&self, folder: &str, uid: u32) {
        use futures::TryStreamExt;
        let mut s = self.session().await;
        s.select(folder).await.unwrap();
        let _: Vec<_> = s.uid_store(uid.to_string(), "+FLAGS (\\Deleted)").await.unwrap().try_collect().await.unwrap();
        let _: Vec<_> = s.expunge().await.unwrap().try_collect().await.unwrap();
        let _ = s.logout().await;
    }

    /// Marque lu côté serveur (helper « autre client »).
    pub async fn mark_seen_on_server(&self, folder: &str, uid: u32) {
        use futures::TryStreamExt;
        let mut s = self.session().await;
        s.select(folder).await.unwrap();
        let _: Vec<_> = s.uid_store(uid.to_string(), "+FLAGS (\\Seen)").await.unwrap().try_collect().await.unwrap();
        let _ = s.logout().await;
    }

    pub async fn server_flags(&self, folder: &str) -> Vec<(u32, bool, bool)> {
        let mut s = self.session().await;
        illy_core::imap::select(&mut s, folder, false).await.unwrap();
        let uids = illy_core::imap::search_all_uids(&mut s).await.unwrap();
        illy_core::imap::fetch_flags(&mut s, &illy_core::imap::uid_set(&uids), None).await.unwrap()
    }
}

impl GreenMail {
    /// Dépose `n` mails simples (numérotés à partir de `from`) avec une seule session.
    pub async fn append_many(&self, folder: &str, from: u32, n: u32) {
        let mut s = self.session().await;
        for i in from..from + n {
            s.append(folder, None, None, simple_eml(i)).await.expect("append");
        }
        let _ = s.logout().await;
    }
}

/// Attend qu'une condition devienne vraie (sondage toutes les 50 ms).
pub async fn wait_until(timeout_secs: u64, mut f: impl FnMut() -> bool) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    while std::time::Instant::now() < deadline {
        if f() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    f()
}
