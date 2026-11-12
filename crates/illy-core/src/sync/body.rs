//! Téléchargement des corps (texte, HTML, images intégrées) et des pièces jointes.

use crate::error::{CoreError, Result};
use crate::events::{CoreEvent, EventSink};
use crate::imap::{fetch_full, fetch_part, select, ImapSession};
use crate::mime::{
    decode_part, extract_full, html_to_text, sanitize_html, snippet_from, synth_mime_header, Decoded, PartInfo,
    PartKind,
};
use crate::model::MessageId;
use crate::store::Store;
use std::collections::HashMap;

const UNAVAILABLE: &str = "(contenu indisponible)";

/// Récupère une partie ; si le mail a disparu du serveur, le retire localement.
async fn part_or_vanish(
    s: &mut ImapSession,
    store: &Store,
    events: &dyn EventSink,
    id: MessageId,
    uid: u32,
    part_path: &str,
) -> Result<Vec<u8>> {
    match fetch_part(s, uid, part_path).await {
        Err(CoreError::NotFound) => {
            let (account, folder, _) = store.message_location(id)?;
            store.delete_message(id)?;
            events.emit(CoreEvent::FolderChanged { account_id: account, folder_id: folder });
            Err(CoreError::NotFound)
        }
        other => other,
    }
}

pub async fn fetch_body(s: &mut ImapSession, store: &Store, events: &dyn EventSink, id: MessageId) -> Result<()> {
    let (_, folder, uid) = store.message_location(id)?;
    select(s, &store.folder_path(folder)?, false).await?;
    let parts = store.body_parts_to_fetch(id)?;
    let has_attachments = store.message_detail_has_attachments(id)?;
    if parts.is_empty() && !has_attachments && !store.has_any_part(id)? {
        return fetch_body_full(s, store, events, id, uid).await;
    }

    let (mut html, mut text) = (String::new(), String::new());
    let mut inline: HashMap<String, (String, Vec<u8>)> = HashMap::new();
    let mut inline_data: Vec<(String, Vec<u8>)> = Vec::new();
    for p in &parts {
        let raw = part_or_vanish(s, store, events, id, uid, &p.part_path).await?;
        match (p.kind, decode_part(&synth_mime_header(p), &raw)) {
            (PartKind::BodyHtml, Decoded::Text(t)) => html.push_str(&t),
            (PartKind::BodyText, Decoded::Text(t)) => text.push_str(&t),
            (PartKind::BodyHtml, Decoded::Bytes(b)) => html.push_str(&String::from_utf8_lossy(&b)),
            (PartKind::BodyText, Decoded::Bytes(b)) => text.push_str(&String::from_utf8_lossy(&b)),
            (PartKind::Inline, decoded) => {
                let bytes = match decoded {
                    Decoded::Bytes(b) => b,
                    Decoded::Text(t) => t.into_bytes(),
                };
                if let Some(cid) = &p.content_id {
                    inline.insert(cid.clone(), (p.mime_type.clone(), bytes.clone()));
                }
                inline_data.push((p.part_path.clone(), bytes));
            }
            (PartKind::Attachment, _) => {}
        }
    }
    let empty = html.trim().is_empty() && text.trim().is_empty();
    let degraded = empty && !(parts.is_empty() && has_attachments);
    let sanitized = (!html.trim().is_empty()).then(|| sanitize_html(&html, &inline));
    let snippet = if !text.trim().is_empty() { snippet_from(&text) } else { snippet_from(&html_to_text(&html)) };
    let text_opt = if degraded { Some(UNAVAILABLE) } else { Some(text.as_str()) };
    store.store_body(id, text_opt, sanitized.as_ref(), &snippet, &inline_data, degraded)?;
    events.emit(CoreEvent::MessageUpdated { message_id: id });
    Ok(())
}

/// Secours : structure inconnue → mail complet analysé localement.
async fn fetch_body_full(
    s: &mut ImapSession,
    store: &Store,
    events: &dyn EventSink,
    id: MessageId,
    uid: u32,
) -> Result<()> {
    let raw = match fetch_full(s, uid).await {
        Err(CoreError::NotFound) => {
            let (account, folder, _) = store.message_location(id)?;
            store.delete_message(id)?;
            events.emit(CoreEvent::FolderChanged { account_id: account, folder_id: folder });
            return Err(CoreError::NotFound);
        }
        other => other?,
    };
    let full = extract_full(&raw);
    let inline: HashMap<String, (String, Vec<u8>)> =
        full.inline.iter().map(|(cid, mime, b)| (cid.clone(), (mime.clone(), b.clone()))).collect();
    let degraded = full.html.trim().is_empty() && full.text.trim().is_empty() && full.attachments.is_empty();
    let sanitized = (!full.html.trim().is_empty()).then(|| sanitize_html(&full.html, &inline));
    let snippet =
        if !full.text.trim().is_empty() { snippet_from(&full.text) } else { snippet_from(&html_to_text(&full.html)) };
    store.add_raw_attachments(id, &full.attachments)?;
    let text = if degraded { UNAVAILABLE.to_string() } else { full.text };
    store.store_body(id, Some(&text), sanitized.as_ref(), &snippet, &[], degraded)?;
    events.emit(CoreEvent::MessageUpdated { message_id: id });
    Ok(())
}

pub async fn fetch_attachment(s: &mut ImapSession, store: &Store, id: MessageId, part_path: &str) -> Result<Vec<u8>> {
    let (info, cached) = store.part(id, part_path)?;
    if let Some(data) = cached {
        return Ok(data);
    }
    let (_, folder, uid) = store.message_location(id)?;
    select(s, &store.folder_path(folder)?, false).await?;
    let raw = fetch_part(s, uid, part_path).await?;
    // Type forcé en binaire : on veut les octets d'origine, sans conversion de charset.
    let as_binary = PartInfo { mime_type: "application/octet-stream".into(), charset: None, ..info };
    let bytes = match decode_part(&synth_mime_header(&as_binary), &raw) {
        Decoded::Bytes(b) => b,
        Decoded::Text(t) => t.into_bytes(),
    };
    store.store_part_data(id, part_path, &bytes)?;
    Ok(bytes)
}
