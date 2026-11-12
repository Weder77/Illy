use crate::error::{CoreError, Result};
use crate::model::{ServerConfig, TlsMode};
use std::fmt::Debug;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::rustls;

pub trait ImapIo: AsyncRead + AsyncWrite + Unpin + Send + Debug {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + Debug> ImapIo for T {}

pub type ImapStream = Box<dyn ImapIo>;
pub type ImapSession = async_imap::Session<ImapStream>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Caps {
    pub idle: bool,
    pub condstore: bool,
    pub r#move: bool,
    pub special_use: bool,
    pub uidplus: bool,
}

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Texte lisible d'une réponse NO/BAD d'async-imap (`code: …, info: Some("…")` → `…`).
pub fn clean_imap_text(raw: &str) -> String {
    static INFO: OnceLock<regex::Regex> = OnceLock::new();
    let re = INFO.get_or_init(|| regex::Regex::new(r#"info: Some\("((?:[^"\\]|\\.)*)"\)"#).unwrap());
    match re.captures(raw) {
        Some(c) => c[1].replace("\\\"", "\"").trim().to_string(),
        None if raw.starts_with("outcome:") || raw.starts_with("code:") => "le serveur a refusé la demande".into(),
        None => raw.trim().to_string(),
    }
}

/// Refus de connexion : mot de passe (définitif) ou indisponibilité passagère (on réessaiera).
pub(crate) fn login_error(raw: String) -> CoreError {
    let text = clean_imap_text(&raw);
    let lower = raw.to_ascii_lowercase();
    let transient = ["unavailable", "[limit]", "inuse", "serverbug", "temporar", "try again", "too many"]
        .iter()
        .any(|k| lower.contains(k));
    if transient {
        CoreError::Network(text)
    } else {
        CoreError::Auth(text)
    }
}

pub fn imap_err(e: async_imap::error::Error) -> CoreError {
    use async_imap::error::Error as E;
    match e {
        // async-imap signale une réponse non analysable par une erreur d'E/S ;
        // la session est alors inutilisable (les octets fautifs restent en tampon).
        // Le message brut contient la réponse du serveur (donc du contenu de mail) : on ne le garde pas.
        E::Io(io) if io.to_string().contains("during parsing of") => {
            CoreError::Parse("réponse du serveur non comprise".into())
        }
        E::Io(io) => CoreError::Network(io.to_string()),
        E::ConnectionLost => CoreError::Network("connexion perdue".into()),
        E::No(s) | E::Bad(s) => CoreError::Imap(clean_imap_text(&s)),
        other => CoreError::Imap(other.to_string()),
    }
}

/// Vérification TLS par le système (magasin de certificats Windows) : les antivirus qui
/// inspectent les connexions chiffrées y installent leur autorité. Repli : racines Mozilla.
fn tls_config() -> Arc<rustls::ClientConfig> {
    static CFG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CFG.get_or_init(|| {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let builder = rustls::ClientConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .expect("protocoles TLS par défaut");
        let cfg = match rustls_platform_verifier::Verifier::new(provider) {
            Ok(v) => builder.dangerous().with_custom_certificate_verifier(Arc::new(v)).with_no_client_auth(),
            Err(e) => {
                tracing::warn!("vérificateur TLS du système indisponible ({e}), racines Mozilla utilisées");
                let mut roots = rustls::RootCertStore::empty();
                roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                builder.with_root_certificates(roots).with_no_client_auth()
            }
        };
        Arc::new(cfg)
    })
    .clone()
}

async fn tls_wrap(host: &str, tcp: TcpStream) -> Result<ImapStream> {
    let name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|e| CoreError::InvalidInput(format!("nom de serveur invalide : {e}")))?;
    let tls = tokio_rustls::TlsConnector::from(tls_config())
        .connect(name, tcp)
        .await
        .map_err(|e| CoreError::Tls(e.to_string()))?;
    Ok(Box::new(tls))
}

async fn tcp(cfg: &ServerConfig) -> Result<TcpStream> {
    let s = TcpStream::connect((cfg.host.as_str(), cfg.port))
        .await
        .map_err(|e| CoreError::Network(format!("{}:{} — {e}", cfg.host, cfg.port)))?;
    // Petites commandes IMAP : sans TCP_NODELAY, Nagle + ACK différé ajoutent ~40-200 ms par échange.
    let _ = s.set_nodelay(true);
    let keepalive = socket2::TcpKeepalive::new().with_time(Duration::from_secs(60)).with_interval(Duration::from_secs(20));
    let _ = socket2::SockRef::from(&s).set_tcp_keepalive(&keepalive);
    Ok(s)
}

async fn greeting(client: &mut async_imap::Client<ImapStream>) -> Result<()> {
    match client.read_response().await {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(CoreError::Network("le serveur a fermé la connexion".into())),
        Err(e) => Err(CoreError::Network(e.to_string())),
    }
}

async fn open(cfg: &ServerConfig) -> Result<async_imap::Client<ImapStream>> {
    match cfg.tls {
        TlsMode::Plain if !cfg!(feature = "insecure-plain") => {
            Err(CoreError::InvalidInput("connexion non chiffrée interdite".into()))
        }
        TlsMode::Plain => {
            let mut c = async_imap::Client::new(Box::new(tcp(cfg).await?) as ImapStream);
            greeting(&mut c).await?;
            Ok(c)
        }
        TlsMode::Implicit => {
            let stream = tls_wrap(&cfg.host, tcp(cfg).await?).await?;
            let mut c = async_imap::Client::new(stream);
            greeting(&mut c).await?;
            Ok(c)
        }
        TlsMode::StartTls => {
            let mut plain = async_imap::Client::new(tcp(cfg).await?);
            match plain.read_response().await {
                Ok(Some(_)) => {}
                _ => return Err(CoreError::Network("pas de message d'accueil du serveur".into())),
            }
            plain.run_command_and_check_ok("STARTTLS", None).await.map_err(imap_err)?;
            let stream = tls_wrap(&cfg.host, plain.into_inner()).await?;
            Ok(async_imap::Client::new(stream))
        }
    }
}

async fn connect_inner(cfg: &ServerConfig, user: &str, password: &str) -> Result<(ImapSession, Caps)> {
    let client = open(cfg).await?;
    let mut session = client.login(user, password).await.map_err(|(e, _)| match e {
        async_imap::error::Error::No(s) | async_imap::error::Error::Bad(s) => login_error(s),
        other => imap_err(other),
    })?;
    let c = session.capabilities().await.map_err(imap_err)?;
    let caps = Caps {
        idle: c.has_str("IDLE"),
        condstore: c.has_str("CONDSTORE"),
        r#move: c.has_str("MOVE"),
        special_use: c.has_str("SPECIAL-USE"),
        uidplus: c.has_str("UIDPLUS"),
    };
    Ok((session, caps))
}

/// Ouvre une session IMAP authentifiée (délai maximal : 30 s).
pub async fn connect(cfg: &ServerConfig, user: &str, password: &str) -> Result<(ImapSession, Caps)> {
    if cfg.tls == TlsMode::Plain && !cfg!(feature = "insecure-plain") {
        return Err(CoreError::InvalidInput("connexion non chiffrée interdite".into()));
    }
    match tokio::time::timeout(CONNECT_TIMEOUT, connect_inner(cfg, user, password)).await {
        Ok(r) => r,
        Err(_) => Err(CoreError::Network(format!("délai dépassé en se connectant à {}", cfg.host))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_refusal_classification() {
        let auth = login_error(r#"code: None, info: Some("[AUTHENTICATIONFAILED] Invalid credentials (Failure)")"#.into());
        assert!(matches!(&auth, CoreError::Auth(m) if m == "[AUTHENTICATIONFAILED] Invalid credentials (Failure)"), "{auth:?}");
        assert!(matches!(login_error(r#"code: Some(Alert), info: Some("Please log in via your web browser")"#.into()), CoreError::Auth(_)));
        assert!(matches!(login_error("code: None, info: Some(\"Login failed\")".into()), CoreError::Auth(_)));
        for transient in ["[UNAVAILABLE] Temporary authentication failure", "[LIMIT] Too many connections", "[INUSE] busy", "[SERVERBUG] oops"] {
            let e = login_error(format!("code: None, info: Some(\"{transient}\")"));
            assert!(matches!(e, CoreError::Network(_)), "{transient} → {e:?}");
        }
    }

    #[test]
    fn parse_errors_never_leak_mail_content() {
        let io = std::io::Error::other("Error(...) during parsing of \"* 1 FETCH (BODY[HEADER] {30}\r\nSubject: Secret\")");
        let e = imap_err(async_imap::error::Error::Io(io));
        assert!(matches!(&e, CoreError::Parse(m) if !m.contains("Secret")), "{e:?}");
    }

    #[cfg(not(feature = "insecure-plain"))]
    #[tokio::test]
    async fn plain_refused_without_feature() {
        use super::*;
        let cfg = ServerConfig { host: "127.0.0.1".into(), port: 1, tls: TlsMode::Plain };
        assert!(matches!(connect(&cfg, "u", "p").await, Err(CoreError::InvalidInput(_))));
    }
}
