//! Harnais de tests d'intégration : serveur GreenMail local (IMAP + SMTP sans TLS).
#![allow(dead_code)]

use illy_core::imap::{connect, ImapSession};
use illy_core::model::{ServerConfig, TlsMode};
use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub const USER: &str = "alice";
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
            .arg(format!("-Dgreenmail.users={USER}:{PASSWORD}@localhost"))
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
