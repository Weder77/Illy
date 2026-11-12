//! Erreurs du cœur, sérialisées vers l'interface sous la forme `{ kind, message }`.

use serde::ser::SerializeStruct;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("base de données : {0}")]
    Db(#[from] rusqlite::Error),
    #[error("mot de passe refusé : {0}")]
    Auth(String),
    #[error("serveur injoignable : {0}")]
    Network(String),
    #[error("erreur TLS : {0}")]
    Tls(String),
    #[error("erreur IMAP : {0}")]
    Imap(String),
    #[error("réponse illisible du serveur : {0}")]
    Parse(String),
    /// Refus temporaire du serveur d'envoi (4xx) : à réessayer plus tard.
    #[error("erreur SMTP : {0}")]
    Smtp(String),
    /// Refus définitif du serveur d'envoi (5xx autre qu'un mot de passe refusé).
    #[error("refusé par le serveur d'envoi : {0}")]
    Rejected(String),
    #[error("introuvable")]
    NotFound,
    #[error("{0}")]
    InvalidInput(String),
    #[error("trousseau : {0}")]
    Secret(String),
    #[error("fichier : {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;

impl CoreError {
    pub fn kind(&self) -> &'static str {
        match self {
            CoreError::Db(_) => "db",
            CoreError::Auth(_) => "auth",
            CoreError::Network(_) => "network",
            CoreError::Tls(_) => "tls",
            CoreError::Imap(_) => "imap",
            CoreError::Parse(_) => "parse",
            CoreError::Smtp(_) => "smtp",
            CoreError::Rejected(_) => "rejected",
            CoreError::NotFound => "not_found",
            CoreError::InvalidInput(_) => "invalid_input",
            CoreError::Secret(_) => "secret",
            CoreError::Io(_) => "io",
        }
    }
}

impl serde::Serialize for CoreError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("CoreError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_error_serializes_with_kind() {
        let v = serde_json::to_value(CoreError::Auth("NO".into())).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"kind": "auth", "message": "mot de passe refusé : NO"})
        );
    }
}
