//! SMTP : vérification des identifiants (l'envoi arrive au jalon 4).

use crate::error::{CoreError, Result};
use crate::model::{ServerConfig, TlsMode};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, Tokio1Executor};
use std::time::Duration;

fn smtp_err(e: lettre::transport::smtp::Error) -> CoreError {
    let code = e.status().map(|c| c.to_string()).unwrap_or_default();
    if e.is_tls() {
        CoreError::Tls(e.to_string())
    } else if code.starts_with("535") || code.starts_with("534") || code.starts_with("530") || e.is_permanent() {
        CoreError::Auth(e.to_string())
    } else if e.is_transient() || e.is_response() {
        CoreError::Smtp(e.to_string())
    } else {
        CoreError::Network(e.to_string())
    }
}

pub(crate) fn transport(cfg: &ServerConfig, user: &str, password: &str) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
    let builder = match cfg.tls {
        TlsMode::Implicit => AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host).map_err(smtp_err)?,
        TlsMode::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host).map_err(smtp_err)?,
        TlsMode::Plain if cfg!(feature = "insecure-plain") => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.host),
        TlsMode::Plain => return Err(CoreError::InvalidInput("connexion non chiffrée interdite".into())),
    };
    Ok(builder
        .port(cfg.port)
        .credentials(Credentials::new(user.to_string(), password.to_string()))
        .timeout(Some(Duration::from_secs(30)))
        .build())
}

/// Se connecte et s'authentifie sur le serveur SMTP, sans rien envoyer.
pub async fn test_smtp(cfg: &ServerConfig, user: &str, password: &str) -> Result<()> {
    let t = transport(cfg, user, password)?;
    match t.test_connection().await {
        Ok(true) => Ok(()),
        Ok(false) => Err(CoreError::Network(format!("le serveur SMTP {} ne répond pas", cfg.host))),
        Err(e) => Err(smtp_err(e)),
    }
}
