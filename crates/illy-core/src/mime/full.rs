//! Analyse d'un mail complet (secours quand la BODYSTRUCTURE du serveur est illisible).

use mail_parser::MessageParser;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FullMessage {
    pub html: String,
    pub text: String,
    /// (content-id, type MIME, octets)
    pub inline: Vec<(String, String, Vec<u8>)>,
    /// (nom de fichier, type MIME, octets)
    pub attachments: Vec<(String, String, Vec<u8>)>,
}

pub fn extract_full(raw: &[u8]) -> FullMessage {
    let Some(msg) = MessageParser::default().parse(raw) else {
        return FullMessage::default();
    };
    let has_real_html = msg.html_body().any(|p| matches!(p.kind(), mail_parser::PartKind::Html));
    let html = if has_real_html { msg.html_bodies().collect::<Vec<_>>().join("\n") } else { String::new() };
    let text: String = msg.text_bodies().collect::<Vec<_>>().join("\n");
    let mut out = FullMessage { html, text, ..Default::default() };
    for (i, part) in msg.attachments().enumerate() {
        let mime = part
            .content_type()
            .map(|c| format!("{}/{}", c.ctype(), c.subtype().unwrap_or("octet-stream")).to_ascii_lowercase())
            .unwrap_or_else(|| "application/octet-stream".into());
        let bytes = part.decoded().into_owned();
        let cid = part.content_id().map(|c| c.trim_start_matches('<').trim_end_matches('>').to_string());
        match cid {
            Some(cid) if mime.starts_with("image/") && part.is_inline() => out.inline.push((cid, mime, bytes)),
            _ => {
                let name = part
                    .attachment_name()
                    .map(str::to_string)
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| format!("piece-jointe-{}", i + 1));
                out.attachments.push((name, mime, bytes));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn extract_full_message() {
        let m = extract_full(&fixture("inline_cid_and_pdf.eml"));
        assert!(m.html.contains("cid:logo@x"), "{}", m.html);
        assert_eq!(m.inline.len(), 1);
        assert_eq!(m.inline[0].0, "logo@x");
        assert_eq!(m.inline[0].2, vec![1, 2, 3]);
        assert_eq!(m.attachments.len(), 1);
        assert_eq!(m.attachments[0].0, "devis.pdf");
        assert_eq!(m.attachments[0].1, "application/pdf");
        assert!(m.attachments[0].2.starts_with(b"%PDF"));
    }

    #[test]
    fn extract_full_plain() {
        let m = extract_full(&fixture("plain_windows1252.eml"));
        assert!(m.html.is_empty());
        assert!(m.text.contains("café"), "{}", m.text);
    }

    #[test]
    fn extract_full_garbage_does_not_panic() {
        let m = extract_full(&fixture("broken_mime.eml"));
        let _ = (m.html, m.text);
        let m = extract_full(b"\xff\x00garbage");
        assert!(m.attachments.is_empty());
    }
}
