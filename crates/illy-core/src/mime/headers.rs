use crate::model::Address;
use mail_parser::MessageParser;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ParsedHeaders {
    pub subject: String,
    pub from: Address,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub reply_to: Vec<Address>,
    pub date: Option<i64>,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
}

fn addr(name: Option<&str>, email: Option<&str>) -> Address {
    Address {
        name: name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()),
        email: email.unwrap_or("").trim().to_string(),
    }
}

/// Analyse un bloc d'en-têtes ; ne échoue jamais (champs absents → vides).
pub fn parse_header_block(raw: &[u8]) -> ParsedHeaders {
    // En-têtes 8 bits bruts (vieux logiciels) : s'ils ne sont pas de l'UTF-8, c'est presque toujours du windows-1252.
    let converted;
    let raw = if std::str::from_utf8(raw).is_err() {
        converted = encoding_rs::WINDOWS_1252.decode(raw).0.into_owned();
        converted.as_bytes()
    } else {
        raw
    };
    let Some(msg) = MessageParser::default().parse_headers(raw) else {
        return ParsedHeaders::default();
    };
    let list = |l: Option<mail_parser::AddressList<'_>>| -> Vec<Address> {
        l.map(|l| l.mailboxes().map(|m| addr(m.name(), m.address())).collect()).unwrap_or_default()
    };
    ParsedHeaders {
        subject: msg.subject().unwrap_or("").trim().to_string(),
        from: msg.from().and_then(|l| l.first()).map(|m| addr(m.name(), m.address())).unwrap_or_default(),
        to: list(msg.to()),
        cc: list(msg.cc()),
        reply_to: list(msg.reply_to()),
        date: msg.date().map(|d| d.to_timestamp()),
        message_id: msg.message_id().map(str::to_string),
        in_reply_to: msg.in_reply_to().and_then(|l| l.first().map(str::to_string)),
        references: msg.references().map(|l| l.iter().map(str::to_string).collect()).unwrap_or_default(),
    }
}

/// Décode un mot encodé RFC 2047 isolé (noms de fichiers dans BODYSTRUCTURE).
pub(crate) fn decode_encoded_words(v: &str) -> String {
    if !v.contains("=?") {
        return v.to_string();
    }
    let raw = format!("Subject: {v}\r\n\r\n");
    MessageParser::default()
        .parse_headers(raw.as_bytes())
        .and_then(|m| m.subject().map(str::to_string))
        .unwrap_or_else(|| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn header_latin1_encoded_word() {
        let h = parse_header_block(&fixture("latin1_subject.eml"));
        assert_eq!(h.subject, "Réunion à 14h – compte rendu");
        assert_eq!(h.from.name.as_deref(), Some("Élodie Martin"));
        assert_eq!(h.from.email, "elodie@exemple.fr");
        assert_eq!(h.to.len(), 2);
        assert_eq!(h.to[1].email, "autre@exemple.fr");
        assert_eq!(h.cc[0].name.as_deref(), Some("Clément"));
        assert_eq!(h.date, Some(1_794_317_520));
        assert_eq!(h.message_id.as_deref(), Some("abc@exemple.fr"));
        assert_eq!(h.in_reply_to.as_deref(), Some("prev@exemple.fr"));
        assert_eq!(h.references, vec!["root@exemple.fr".to_string(), "prev@exemple.fr".to_string()]);
    }

    #[test]
    fn raw_8bit_latin1_header_keeps_accents() {
        let h = parse_header_block(b"From: \xc9lodie <e@x.fr>\r\nSubject: R\xe9union \xe0 14h\r\n\r\n");
        assert_eq!(h.subject, "Réunion à 14h");
        assert_eq!(h.from.name.as_deref(), Some("Élodie"));
    }

    #[test]
    fn reply_to_parsed() {
        let h = parse_header_block(b"From: a@x.fr\r\nReply-To: Liste <liste@ex.fr>\r\n\r\n");
        assert_eq!(h.reply_to.len(), 1);
        assert_eq!(h.reply_to[0].email, "liste@ex.fr");
        assert_eq!(h.reply_to[0].name.as_deref(), Some("Liste"));
    }

    #[test]
    fn header_missing_fields() {
        let h = parse_header_block(b"X-Foo: bar\r\n\r\n");
        assert_eq!(h.subject, "");
        assert_eq!(h.from.email, "");
        assert_eq!(h.date, None);
        assert!(h.to.is_empty());
    }

    #[test]
    fn header_garbage_does_not_panic() {
        let h = parse_header_block(b"\xff\xfe\x00garbage");
        assert_eq!(h.from.email, "");
    }
}
