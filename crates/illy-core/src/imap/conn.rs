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
}

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

pub fn imap_err(e: async_imap::error::Error) -> CoreError {
    use async_imap::error::Error as E;
    match e {
        // async-imap signale une réponse non analysable par une erreur d'E/S ;
        // la session est alors inutilisable (les octets fautifs restent en tampon).
        E::Io(io) if io.to_string().contains("during parsing of") => {
            CoreError::Parse(io.to_string().chars().take(300).collect())
        }
        E::Io(io) => CoreError::Network(io.to_string()),
        E::ConnectionLost => CoreError::Network("connexion perdue".into()),
        E::No(s) | E::Bad(s) => CoreError::Imap(s),
        other => CoreError::Imap(other.to_string()),
    }
}

fn tls_config() -> Arc<rustls::ClientConfig> {
    static CFG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CFG.get_or_init(|| {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .expect("protocoles TLS par défaut")
            .with_root_certificates(roots)
            .with_no_client_auth();
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
        async_imap::error::Error::No(s) | async_imap::error::Error::Bad(s) => CoreError::Auth(s),
        other => imap_err(other),
    })?;
    let c = session.capabilities().await.map_err(imap_err)?;
    let caps = Caps {
        idle: c.has_str("IDLE"),
        condstore: c.has_str("CONDSTORE"),
        r#move: c.has_str("MOVE"),
        special_use: c.has_str("SPECIAL-USE"),
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
    #[cfg(not(feature = "insecure-plain"))]
    #[tokio::test]
    async fn plain_refused_without_feature() {
        use super::*;
        let cfg = ServerConfig { host: "127.0.0.1".into(), port: 1, tls: TlsMode::Plain };
        assert!(matches!(connect(&cfg, "u", "p").await, Err(CoreError::InvalidInput(_))));
    }
}
