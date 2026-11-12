//! Construction des messages à envoyer (et des copies de brouillon).

use crate::error::{CoreError, Result};
use crate::mime::html_to_text;
use crate::model::{Address, Draft, DraftId};
use crate::store::ReplySource;
use base64::Engine;
use lettre::message::header::{ContentType, HeaderName, HeaderValue};
use lettre::message::{Attachment, Mailbox, MultiPart, SinglePart};
use lettre::Message;
use std::sync::OnceLock;

pub struct OutgoingFile {
    pub filename: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

pub struct BuiltMessage {
    /// Message tel qu'il sera stocké (Envoyés, Brouillons) : jamais d'en-tête Bcc.
    pub raw_for_storage: Vec<u8>,
    /// Adresses de l'enveloppe SMTP (À + Cc + Cci).
    pub envelope_to: Vec<String>,
    pub message_id: String,
}

pub fn total_attachment_size(files: &[OutgoingFile]) -> u64 {
    files.iter().map(|f| f.data.len() as u64).sum()
}

#[derive(Clone)]
struct DraftMarker(String);

impl lettre::message::header::Header for DraftMarker {
    fn name() -> HeaderName {
        HeaderName::new_from_ascii_str("X-Illy-Draft")
    }
    fn parse(s: &str) -> std::result::Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(DraftMarker(s.to_string()))
    }
    fn display(&self) -> HeaderValue {
        HeaderValue::new(Self::name(), self.0.clone())
    }
}

fn mailbox(a: &Address) -> Result<Mailbox> {
    let email = a.email.trim();
    let addr: lettre::Address = email.parse().map_err(|_| CoreError::InvalidInput(format!("Adresse invalide : {email}")))?;
    Ok(Mailbox::new(a.name.clone().filter(|n| !n.trim().is_empty()), addr))
}

fn content_type(mime: &str) -> ContentType {
    ContentType::parse(mime).unwrap_or(ContentType::parse("application/octet-stream").expect("type MIME valide"))
}

/// Remplace les images `data:` du HTML par des références `cid:` et renvoie les parties correspondantes.
fn extract_inline_images(html: &str) -> (String, Vec<(String, String, Vec<u8>)>) {
    static DATA: OnceLock<regex::Regex> = OnceLock::new();
    let re = DATA.get_or_init(|| regex::Regex::new(r#"data:(image/[A-Za-z0-9.+-]+);base64,([A-Za-z0-9+/=\s]+)"#).unwrap());
    let mut parts = Vec::new();
    let out = re.replace_all(html, |c: &regex::Captures| {
        let data: String = c[2].chars().filter(|ch| !ch.is_whitespace()).collect();
        match base64::engine::general_purpose::STANDARD.decode(data) {
            Ok(bytes) => {
                let cid = format!("illy-img-{}@illy", parts.len() + 1);
                parts.push((cid.clone(), c[1].to_string(), bytes));
                format!("cid:{cid}")
            }
            Err(_) => String::new(),
        }
    });
    (out.into_owned(), parts)
}

/// Identifiant de message au format des en-têtes : `<id>` (les identifiants stockés n'ont pas de chevrons).
fn msg_id(raw: &str) -> Option<String> {
    let id = raw.trim().trim_start_matches('<').trim_end_matches('>').trim();
    (!id.is_empty() && !id.contains(char::is_whitespace)).then(|| format!("<{id}>"))
}

pub fn build_message(
    from: &Address,
    draft: &Draft,
    reply: Option<&ReplySource>,
    files: &[OutgoingFile],
    draft_marker: Option<DraftId>,
) -> Result<BuiltMessage> {
    let from_mb = mailbox(from)?;
    let domain = from.email.rsplit('@').next().unwrap_or("illy.local").trim().to_string();
    let message_id = format!("<{}@{}>", uuid::Uuid::new_v4(), domain);

    let mut b = Message::builder().from(from_mb).subject(draft.subject.trim()).date_now().message_id(Some(message_id.clone()));
    let mut envelope_to = Vec::new();
    for a in &draft.to {
        let mb = mailbox(a)?;
        envelope_to.push(mb.email.to_string());
        b = b.to(mb);
    }
    for a in &draft.cc {
        let mb = mailbox(a)?;
        envelope_to.push(mb.email.to_string());
        b = b.cc(mb);
    }
    for a in &draft.bcc {
        // Les Cci ne vont que dans l'enveloppe SMTP, jamais dans le message stocké.
        envelope_to.push(mailbox(a)?.email.to_string());
    }
    if let Some(r) = reply {
        if let Some(id) = r.message_id.as_deref().and_then(msg_id) {
            b = b.in_reply_to(id.clone());
            let mut refs: Vec<String> = r.references.iter().filter_map(|x| msg_id(x)).collect();
            refs.push(id);
            b = b.references(refs.join(" "));
        }
    }
    if let Some(id) = draft_marker {
        b = b.header(DraftMarker(id.to_string()));
    }
    // lettre exige un destinataire : pour une copie de brouillon encore sans destinataire,
    // l'enveloppe (jamais utilisée pour un envoi) pointe sur l'expéditeur.
    let sender: lettre::Address = mailbox(from)?.email;
    let env_to: Vec<lettre::Address> = if envelope_to.is_empty() {
        vec![sender.clone()]
    } else {
        envelope_to.iter().filter_map(|e| e.parse().ok()).collect()
    };
    let envelope = lettre::address::Envelope::new(Some(sender), env_to)
        .map_err(|e| CoreError::InvalidInput(format!("enveloppe invalide : {e}")))?;
    b = b.envelope(envelope);

    // Les images distantes d'un mail cité ou transféré sont bloquées à l'écran dans Illy, mais
    // le destinataire doit les recevoir comme dans l'original (billets avec QR code, etc.).
    let (html, inline) = extract_inline_images(&crate::mime::unblock_images(&draft.body_html));
    let text = html_to_text(&html);
    let html_part = SinglePart::html(html);
    let text_part = SinglePart::plain(text);
    let alternative = if inline.is_empty() {
        MultiPart::alternative().singlepart(text_part).singlepart(html_part)
    } else {
        let mut related = MultiPart::related().singlepart(html_part);
        for (cid, mime, bytes) in inline {
            related = related.singlepart(Attachment::new_inline(cid).body(bytes, content_type(&mime)));
        }
        MultiPart::alternative().singlepart(text_part).multipart(related)
    };
    let body = if files.is_empty() {
        alternative
    } else {
        let mut mixed = MultiPart::mixed().multipart(alternative);
        for f in files {
            mixed = mixed.singlepart(Attachment::new(f.filename.clone()).body(f.data.clone(), content_type(&f.mime_type)));
        }
        mixed
    };
    let message = b.multipart(body).map_err(|e| CoreError::InvalidInput(format!("message invalide : {e}")))?;
    Ok(BuiltMessage { raw_for_storage: message.formatted(), envelope_to, message_id })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use crate::store::ReplySource;
    use mail_parser::MessageParser;

    fn a(name: &str, email: &str) -> Address {
        Address { name: (!name.is_empty()).then(|| name.to_string()), email: email.into() }
    }

    fn draft(body: &str) -> Draft {
        Draft {
            id: 7,
            account_id: 1,
            kind: DraftKind::New,
            source_message_id: None,
            to: vec![a("Bob", "bob@ex.fr")],
            cc: vec![a("", "carol@ex.fr")],
            bcc: vec![a("", "secret@ex.fr")],
            subject: "Réunion à 14h".into(),
            body_html: body.into(),
            attachments: vec![],
            state: DraftState::Editing,
            last_error: None,
            send_at: None,
            remote_uid: None,
            updated_at: 0,
        }
    }

    fn me() -> Address {
        a("Élodie Martin", "elodie@exemple.fr")
    }

    #[test]
    fn alternative_text_and_html() {
        let b = build_message(&me(), &draft("<p>Bonjour <b>Bob</b></p>"), None, &[], None).unwrap();
        let m = MessageParser::default().parse(&b.raw_for_storage).unwrap();
        assert_eq!(m.html_bodies().next().unwrap(), "<p>Bonjour <b>Bob</b></p>");
        assert!(m.text_bodies().next().unwrap().contains("Bonjour Bob"));
        assert!(b.message_id.ends_with("@exemple.fr>"), "{}", b.message_id);
        assert_eq!(m.message_id(), Some(b.message_id.trim_matches(['<', '>'])));
    }

    #[test]
    fn accented_subject_and_from() {
        let b = build_message(&me(), &draft("<p>x</p>"), None, &[], None).unwrap();
        let m = MessageParser::default().parse(&b.raw_for_storage).unwrap();
        assert_eq!(m.subject(), Some("Réunion à 14h"));
        assert_eq!(m.from().unwrap().first().unwrap().name(), Some("Élodie Martin"));
    }

    #[test]
    fn inline_image_becomes_cid() {
        let b = build_message(&me(), &draft(r#"<p>x</p><img src="data:image/png;base64,AQID">"#), None, &[], None).unwrap();
        let raw = String::from_utf8_lossy(&b.raw_for_storage).to_string();
        assert!(!raw.contains("data:image/png"), "data URL must be replaced");
        assert!(raw.contains("multipart/related"));
        let m = MessageParser::default().parse(&b.raw_for_storage).unwrap();
        let html = m.html_bodies().next().unwrap().to_string();
        assert!(html.contains("cid:illy-img-1@illy"), "{html}");
        let img = m.attachments().find(|p| p.content_id() == Some("illy-img-1@illy")).expect("inline part");
        assert_eq!(img.decoded().as_ref(), &[1u8, 2, 3]);
    }

    #[test]
    fn attachment_with_accented_name() {
        let f = OutgoingFile { filename: "facture n°1 é.pdf".into(), mime_type: "application/pdf".into(), data: b"%PDF-1.4".to_vec() };
        let b = build_message(&me(), &draft("<p>x</p>"), None, &[f], None).unwrap();
        let m = MessageParser::default().parse(&b.raw_for_storage).unwrap();
        let att = m.attachments().find(|p| p.attachment_name().is_some()).unwrap();
        assert_eq!(att.attachment_name(), Some("facture n°1 é.pdf"));
        assert_eq!(att.decoded().as_ref(), b"%PDF-1.4");
        assert_eq!(total_attachment_size(&[OutgoingFile { filename: "a".into(), mime_type: "x/y".into(), data: vec![0; 10] }]), 10);
    }

    #[test]
    fn bcc_not_in_stored_message_but_in_envelope() {
        let b = build_message(&me(), &draft("<p>x</p>"), None, &[], None).unwrap();
        let raw = String::from_utf8_lossy(&b.raw_for_storage).to_lowercase();
        assert!(!raw.contains("secret@ex.fr"), "{raw}");
        assert!(!raw.contains("bcc:"));
        assert_eq!(b.envelope_to, vec!["bob@ex.fr", "carol@ex.fr", "secret@ex.fr"]);
    }

    #[test]
    fn reply_headers() {
        let src = ReplySource {
            from: a("", "bob@ex.fr"),
            to: vec![],
            cc: vec![],
            reply_to: vec![],
            subject: "x".into(),
            date: 0,
            message_id: Some("<m1@ex.fr>".into()),
            references: vec!["<r0@ex.fr>".into()],
            account_id: 1,
        };
        let b = build_message(&me(), &draft("<p>x</p>"), Some(&src), &[], None).unwrap();
        let raw = String::from_utf8_lossy(&b.raw_for_storage).to_string();
        assert!(raw.contains("In-Reply-To: <m1@ex.fr>"), "{raw}");
        assert!(raw.contains("References: <r0@ex.fr> <m1@ex.fr>"), "{raw}");
    }

    #[test]
    fn quoted_remote_images_are_restored_for_the_recipient() {
        let body = r#"<p>Voir ci-dessous</p><div class="illy-forward"><img data-blocked-src="https://ex.fr/qr.png" alt="QR"></div>"#;
        let b = build_message(&me(), &draft(body), None, &[], None).unwrap();
        let parsed = mail_parser::MessageParser::default().parse(&b.raw_for_storage).unwrap();
        let html = parsed.body_html(0).unwrap().to_string();
        assert!(!html.contains("data-blocked-src"), "{html}");
        assert!(html.contains(r#"src="https://ex.fr/qr.png""#), "{html}");
    }

    #[test]
    fn reply_headers_add_missing_angle_brackets() {
        // mail-parser rend les Message-ID sans chevrons : ils doivent revenir dans les en-têtes envoyés.
        let src = ReplySource {
            from: a("", "bob@ex.fr"),
            to: vec![],
            cc: vec![],
            reply_to: vec![],
            subject: "x".into(),
            date: 0,
            message_id: Some("q1@localhost".into()),
            references: vec!["root@localhost".into(), " ".into()],
            account_id: 1,
        };
        let b = build_message(&me(), &draft("<p>x</p>"), Some(&src), &[], None).unwrap();
        let raw = String::from_utf8_lossy(&b.raw_for_storage).to_string();
        assert!(raw.contains("In-Reply-To: <q1@localhost>\r\n"), "{raw}");
        assert!(raw.contains("References: <root@localhost> <q1@localhost>\r\n"), "{raw}");
    }

    #[test]
    fn draft_without_recipients_can_be_stored() {
        let mut d = draft("<p>idée</p>");
        d.to.clear();
        d.cc.clear();
        d.bcc.clear();
        let b = build_message(&me(), &d, None, &[], Some(7)).unwrap();
        assert!(b.envelope_to.is_empty());
        assert!(String::from_utf8_lossy(&b.raw_for_storage).contains("X-Illy-Draft: 7"));
    }

    #[test]
    fn draft_marker_header() {
        let b = build_message(&me(), &draft("<p>x</p>"), None, &[], Some(7)).unwrap();
        assert!(String::from_utf8_lossy(&b.raw_for_storage).contains("X-Illy-Draft: 7"));
    }

    #[test]
    fn invalid_address_rejected() {
        let mut d = draft("<p>x</p>");
        d.to = vec![a("", "pas une adresse")];
        let r = build_message(&me(), &d, None, &[], None);
        assert!(matches!(&r, Err(crate::CoreError::InvalidInput(m)) if m == "Adresse invalide : pas une adresse"), "{:?}", r.err());
    }
}
