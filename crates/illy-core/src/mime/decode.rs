use mail_parser::MessageParser;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decoded {
    Text(String),
    Bytes(Vec<u8>),
}

/// Décode une partie MIME isolée (en-têtes MIME de la partie + corps brut).
/// Les parties `text/*` sont décodées selon leur charset ; les autres restent binaires.
pub fn decode_part(mime_header: &[u8], body: &[u8]) -> Decoded {
    let mut raw = Vec::with_capacity(mime_header.len() + body.len() + 4);
    raw.extend_from_slice(mime_header);
    if !mime_header.ends_with(b"\r\n") && !mime_header.ends_with(b"\n") {
        raw.extend_from_slice(b"\r\n");
    }
    if !mime_header.ends_with(b"\r\n\r\n") && !mime_header.ends_with(b"\n\n") {
        raw.extend_from_slice(b"\r\n");
    }
    raw.extend_from_slice(body);
    let Some(msg) = MessageParser::default().parse(&raw) else {
        return Decoded::Bytes(body.to_vec());
    };
    let part = msg.root_part();
    let is_text = part.content_type().map(|c| c.ctype().eq_ignore_ascii_case("text")).unwrap_or(true);
    if is_text {
        if let Some(t) = part.text() {
            return Decoded::Text(t.into_owned());
        }
    }
    Decoded::Bytes(part.decoded().into_owned())
}

/// En-tête MIME minimal reconstruit depuis la BODYSTRUCTURE (évite `BODY[x.MIME]`,
/// mal géré par certains serveurs).
pub fn synth_mime_header(p: &crate::mime::PartInfo) -> Vec<u8> {
    let clean = |s: &str| s.replace(['\r', '\n', '"'], "");
    let mut h = format!("Content-Type: {}", clean(&p.mime_type));
    if let Some(cs) = &p.charset {
        h.push_str(&format!("; charset=\"{}\"", clean(cs)));
    }
    h.push_str(&format!("\r\nContent-Transfer-Encoding: {}\r\n\r\n", clean(&p.encoding)));
    h.into_bytes()
}

/// Extrait de 200 caractères maximum, espaces regroupés.
pub fn snippet_from(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(200).collect()
}

/// Texte brut à partir de HTML (pour les extraits) : sans style ni script, sans balises.
pub fn html_to_text(html: &str) -> String {
    use std::sync::OnceLock;
    static BLOCKS: OnceLock<regex::Regex> = OnceLock::new();
    static TAGS: OnceLock<regex::Regex> = OnceLock::new();
    let blocks = BLOCKS.get_or_init(|| {
        regex::Regex::new(r"(?is)<(style|script|head|title)\b[^>]*>.*?</(style|script|head|title)\s*>").unwrap()
    });
    let tags = TAGS.get_or_init(|| regex::Regex::new(r"(?s)<[^>]*>").unwrap());
    let no_blocks = blocks.replace_all(html, " ");
    let no_tags = tags.replace_all(&no_blocks, " ");
    let decoded = html_escape::decode_html_entities(&no_tags);
    decoded.replace('\u{a0}', " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_qp_windows1252() {
        let d = decode_part(
            b"Content-Type: text/plain; charset=windows-1252\r\nContent-Transfer-Encoding: quoted-printable\r\n",
            b"caf=E9",
        );
        assert_eq!(d, Decoded::Text("café".into()));
    }

    #[test]
    fn decode_base64_binary() {
        let d = decode_part(b"Content-Type: image/png\r\nContent-Transfer-Encoding: base64\r\n", b"AQID");
        assert_eq!(d, Decoded::Bytes(vec![1, 2, 3]));
    }

    #[test]
    fn decode_html_utf8() {
        let d = decode_part(b"Content-Type: text/html; charset=utf-8\r\n", "<p>été</p>".as_bytes());
        assert_eq!(d, Decoded::Text("<p>été</p>".into()));
    }

    #[test]
    fn synth_header_decodes_like_server_header() {
        let p = crate::mime::PartInfo {
            part_path: "1".into(),
            mime_type: "text/plain".into(),
            filename: None,
            size: 6,
            content_id: None,
            encoding: "quoted-printable".into(),
            charset: Some("windows-1252".into()),
            kind: crate::mime::PartKind::BodyText,
        };
        assert_eq!(decode_part(&synth_mime_header(&p), b"caf=E9"), Decoded::Text("caf\u{e9}".into()));
    }

    #[test]
    fn snippet_collapses_and_truncates() {
        let long = "ligne\n".repeat(100);
        let s = snippet_from(&long);
        assert!(s.chars().count() <= 200);
        assert!(!s.contains('\n'));
        assert!(s.starts_with("ligne ligne"));
    }

    #[test]
    fn html_to_text_strips_tags() {
        assert_eq!(html_to_text("<p>Bonjour <b>Marie</b></p><style>p{}</style>"), "Bonjour Marie");
        assert_eq!(html_to_text("a&nbsp;&amp;&eacute;<script>x()</script>"), "a &é");
    }
}
