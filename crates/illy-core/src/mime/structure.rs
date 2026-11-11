use super::headers::decode_encoded_words;
use imap_proto::{BodyContentCommon, BodyContentSinglePart, BodyStructure, ContentEncoding};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartKind {
    BodyText,
    BodyHtml,
    Inline,
    Attachment,
}

impl PartKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PartKind::BodyText => "body_text",
            PartKind::BodyHtml => "body_html",
            PartKind::Inline => "inline",
            PartKind::Attachment => "attachment",
        }
    }
    pub fn parse(s: &str) -> PartKind {
        match s {
            "body_text" => PartKind::BodyText,
            "body_html" => PartKind::BodyHtml,
            "inline" => PartKind::Inline,
            _ => PartKind::Attachment,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartInfo {
    pub part_path: String,
    pub mime_type: String,
    pub filename: Option<String>,
    pub size: u64,
    pub content_id: Option<String>,
    pub encoding: String,
    pub charset: Option<String>,
    pub kind: PartKind,
}

/// Aplatit une BODYSTRUCTURE IMAP en liste de parties feuilles numérotées.
pub fn flatten_structure(bs: &BodyStructure<'_>) -> Vec<PartInfo> {
    let mut out = Vec::new();
    match bs {
        BodyStructure::Multipart { bodies, .. } => walk_children(bodies, "", &mut out),
        leaf => leaf_info(leaf, "1".to_string(), &mut out),
    }
    out
}

fn walk_children(bodies: &[BodyStructure<'_>], prefix: &str, out: &mut Vec<PartInfo>) {
    for (i, b) in bodies.iter().enumerate() {
        let path = if prefix.is_empty() { format!("{}", i + 1) } else { format!("{prefix}.{}", i + 1) };
        match b {
            BodyStructure::Multipart { bodies, .. } => walk_children(bodies, &path, out),
            leaf => leaf_info(leaf, path, out),
        }
    }
}

fn param<'a>(params: &'a imap_proto::BodyParams<'_>, key: &str) -> Option<String> {
    params.as_ref()?.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.to_string())
}

/// Valeur RFC 2231 `charset'lang'pourcent%20encodé`.
fn decode_rfc2231(v: &str) -> String {
    let mut it = v.splitn(3, '\'');
    let (charset, _lang, data) = match (it.next(), it.next(), it.next()) {
        (Some(c), Some(l), Some(d)) => (c, l, d),
        _ => return v.to_string(),
    };
    let mut bytes = Vec::with_capacity(data.len());
    let b = data.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(x) = u8::from_str_radix(&data[i + 1..i + 3], 16) {
                bytes.push(x);
                i += 3;
                continue;
            }
        }
        bytes.push(b[i]);
        i += 1;
    }
    if charset.eq_ignore_ascii_case("utf-8") || charset.is_empty() {
        String::from_utf8_lossy(&bytes).into_owned()
    } else {
        bytes.iter().map(|&c| c as char).collect()
    }
}

fn filename_of(common: &BodyContentCommon<'_>) -> Option<String> {
    let disp_params = common.disposition.as_ref().map(|d| &d.params);
    let from_disp = disp_params.and_then(|p| {
        param(p, "filename*").map(|v| decode_rfc2231(&v)).or_else(|| param(p, "filename"))
    });
    let from_type = || {
        param(&common.ty.params, "name*").map(|v| decode_rfc2231(&v)).or_else(|| param(&common.ty.params, "name"))
    };
    from_disp.or_else(from_type).map(|n| decode_encoded_words(&n)).filter(|n| !n.trim().is_empty())
}

fn encoding_str(e: &ContentEncoding<'_>) -> String {
    match e {
        ContentEncoding::SevenBit => "7bit".into(),
        ContentEncoding::EightBit => "8bit".into(),
        ContentEncoding::Binary => "binary".into(),
        ContentEncoding::Base64 => "base64".into(),
        ContentEncoding::QuotedPrintable => "quoted-printable".into(),
        ContentEncoding::Other(s) => s.to_ascii_lowercase(),
    }
}

fn leaf_info(b: &BodyStructure<'_>, path: String, out: &mut Vec<PartInfo>) {
    let (common, other): (&BodyContentCommon<'_>, &BodyContentSinglePart<'_>) = match b {
        BodyStructure::Basic { common, other, .. }
        | BodyStructure::Text { common, other, .. }
        | BodyStructure::Message { common, other, .. } => (common, other),
        BodyStructure::Multipart { .. } => return,
    };
    let ty = common.ty.ty.to_ascii_lowercase();
    let sub = common.ty.subtype.to_ascii_lowercase();
    let mime_type = format!("{ty}/{sub}");
    let disposition = common.disposition.as_ref().map(|d| d.ty.to_ascii_lowercase());
    let content_id = other
        .id
        .as_ref()
        .map(|c| c.trim().trim_start_matches('<').trim_end_matches('>').to_string())
        .filter(|c| !c.is_empty());
    let mut filename = filename_of(common);
    let is_message = matches!(b, BodyStructure::Message { .. }) || mime_type == "message/rfc822";
    let is_attachment = disposition.as_deref() == Some("attachment")
        || (filename.is_some() && content_id.is_none())
        || is_message;
    let kind = if is_attachment {
        PartKind::Attachment
    } else if ty == "image" && content_id.is_some() {
        PartKind::Inline
    } else if mime_type == "text/html" {
        PartKind::BodyHtml
    } else if mime_type == "text/plain" {
        PartKind::BodyText
    } else {
        PartKind::Attachment
    };
    if kind == PartKind::Attachment && filename.is_none() {
        filename = Some(if is_message { "message.eml".into() } else { format!("piece-jointe-{path}") });
    }
    out.push(PartInfo {
        part_path: path,
        mime_type,
        filename,
        size: other.octets as u64,
        content_id,
        encoding: encoding_str(&other.transfer_encoding),
        charset: param(&common.ty.params, "charset").map(|c| c.to_ascii_lowercase()),
        kind,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bs(raw: &str) -> Vec<PartInfo> {
        let line = format!("* 1 FETCH (BODYSTRUCTURE {raw})\r\n");
        let (_, resp) = imap_proto::Response::parse(line.as_bytes()).expect("parse");
        match resp {
            imap_proto::Response::Fetch(_, attrs) => {
                for a in attrs {
                    if let imap_proto::AttributeValue::BodyStructure(b) = a {
                        return flatten_structure(&b);
                    }
                }
                panic!("no bodystructure")
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    fn summary(p: &[PartInfo]) -> Vec<(&str, PartKind)> {
        p.iter().map(|x| (x.part_path.as_str(), x.kind)).collect()
    }

    #[test]
    fn structure_single_part() {
        let p = bs(r#"("TEXT" "PLAIN" ("CHARSET" "UTF-8") NIL NIL "7BIT" 12 1 NIL NIL NIL)"#);
        assert_eq!(summary(&p), vec![("1", PartKind::BodyText)]);
        assert_eq!(p[0].charset.as_deref(), Some("utf-8"));
    }

    #[test]
    fn structure_alternative() {
        let p = bs(r#"(("TEXT" "PLAIN" ("CHARSET" "UTF-8") NIL NIL "7BIT" 10 1 NIL NIL NIL)("TEXT" "HTML" ("CHARSET" "UTF-8") NIL NIL "QUOTED-PRINTABLE" 20 1 NIL NIL NIL) "ALTERNATIVE" ("BOUNDARY" "b1") NIL NIL)"#);
        assert_eq!(summary(&p), vec![("1", PartKind::BodyText), ("2", PartKind::BodyHtml)]);
        assert_eq!(p[1].encoding, "quoted-printable");
    }

    #[test]
    fn structure_mixed_inline_pdf() {
        let p = bs(r#"((("TEXT" "HTML" ("CHARSET" "UTF-8") NIL NIL "7BIT" 30 1 NIL NIL NIL)("IMAGE" "PNG" ("NAME" "logo.png") "<logo@x>" NIL "BASE64" 100 NIL ("INLINE" ("FILENAME" "logo.png")) NIL) "RELATED" ("BOUNDARY" "r1") NIL NIL)("APPLICATION" "PDF" ("NAME" "devis.pdf") NIL NIL "BASE64" 2000 NIL ("ATTACHMENT" ("FILENAME" "devis.pdf")) NIL) "MIXED" ("BOUNDARY" "m1") NIL NIL)"#);
        assert_eq!(
            summary(&p),
            vec![("1.1", PartKind::BodyHtml), ("1.2", PartKind::Inline), ("2", PartKind::Attachment)]
        );
        assert_eq!(p[1].content_id.as_deref(), Some("logo@x"));
        assert_eq!(p[1].mime_type, "image/png");
        assert_eq!(p[2].filename.as_deref(), Some("devis.pdf"));
        assert_eq!(p[2].size, 2000);
    }

    #[test]
    fn structure_text_attachment_and_encoded_name() {
        let p = bs(r#"(("TEXT" "PLAIN" ("CHARSET" "UTF-8") NIL NIL "7BIT" 10 1 NIL NIL NIL)("TEXT" "PLAIN" ("NAME" "=?UTF-8?Q?r=C3=A9sum=C3=A9.txt?=") NIL NIL "BASE64" 40 1 NIL ("ATTACHMENT" NIL) NIL) "MIXED" ("BOUNDARY" "m1") NIL NIL)"#);
        assert_eq!(summary(&p), vec![("1", PartKind::BodyText), ("2", PartKind::Attachment)]);
        assert_eq!(p[1].filename.as_deref(), Some("résumé.txt"));
    }

    #[test]
    fn structure_rfc2231_filename() {
        let p = bs(r#"(("TEXT" "PLAIN" NIL NIL NIL "7BIT" 10 1 NIL NIL NIL)("APPLICATION" "PDF" NIL NIL NIL "BASE64" 40 NIL ("ATTACHMENT" ("FILENAME*" "UTF-8''facture%20n%C2%B01.pdf")) NIL) "MIXED" ("BOUNDARY" "m1") NIL NIL)"#);
        assert_eq!(p[1].filename.as_deref(), Some("facture n°1.pdf"));
    }
}
