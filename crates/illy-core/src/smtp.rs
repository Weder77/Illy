//! SMTP : vérification des identifiants et envoi.

use crate::error::{CoreError, Result};
use crate::model::{ServerConfig, TlsMode};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, Tokio1Executor};
use std::time::Duration;

/// Texte de la réponse du serveur, sans le préfixe de lettre (« permanent error (550): »).
fn server_text(e: &lettre::transport::smtp::Error) -> String {
    let text = e.to_string();
    let text = text.split_once("): ").map(|(_, t)| t.to_string()).unwrap_or(text);
    match e.status() {
        Some(code) => format!("{code} {}", text.trim()),
        None => text.trim().to_string(),
    }
}

fn smtp_err(e: lettre::transport::smtp::Error) -> CoreError {
    let code = e.status().map(|c| c.to_string()).unwrap_or_default();
    if e.is_tls() {
        CoreError::Tls(e.to_string())
    } else if code.starts_with("535") || code.starts_with("534") || code.starts_with("530") {
        CoreError::Auth(server_text(&e))
    } else if e.is_permanent() {
        // Destinataire inconnu, mail trop gros, quota, spam… : le serveur ne l'acceptera pas.
        CoreError::Rejected(server_text(&e))
    } else if e.is_transient() || e.is_response() {
        CoreError::Smtp(server_text(&e))
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

/// Envoie un message déjà construit (enveloppe explicite : À + Cc + Cci).
pub async fn send_raw(cfg: &ServerConfig, user: &str, password: &str, envelope_to: &[String], from: &str, raw: &[u8]) -> Result<()> {
    use lettre::AsyncTransport;
    let parse = |e: &str| -> Result<lettre::Address> {
        e.trim().parse().map_err(|_| CoreError::InvalidInput(format!("Adresse invalide : {e}")))
    };
    let to = envelope_to.iter().map(|e| parse(e)).collect::<Result<Vec<_>>>()?;
    let envelope = lettre::address::Envelope::new(Some(parse(from)?), to)
        .map_err(|e| CoreError::InvalidInput(format!("enveloppe invalide : {e}")))?;
    transport(cfg, user, password)?.send_raw(&envelope, raw).await.map_err(smtp_err)?;
    Ok(())
}
