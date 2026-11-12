use super::conn::{imap_err, ImapSession};
use crate::error::Result;
use crate::model::FolderRole;
use crate::store::FolderInfo;
use futures::TryStreamExt;
use imap_proto::NameAttribute;

const SENT: &[&str] = &["sent", "sent items", "sent messages", "envoyés", "éléments envoyés", "messages envoyés"];
const DRAFTS: &[&str] = &["drafts", "brouillons"];
const TRASH: &[&str] = &["trash", "deleted items", "deleted messages", "corbeille", "éléments supprimés"];
const SPAM: &[&str] = &["spam", "junk", "bulk", "bulk mail", "courrier indésirable", "pourriels"];

fn role_name(role: FolderRole) -> &'static str {
    match role {
        FolderRole::Inbox => "Boîte de réception",
        FolderRole::Drafts => "Brouillons",
        FolderRole::Sent => "Envoyés",
        FolderRole::Spam => "Spam",
        FolderRole::Trash => "Corbeille",
        FolderRole::Custom => "",
    }
}

/// Détermine le rôle, le nom affiché et la visibilité d'un dossier (fonction pure).
pub fn folder_info(path: &str, delimiter: Option<&str>, attributes: &[String]) -> FolderInfo {
    let attrs: Vec<String> = attributes.iter().map(|a| a.to_ascii_lowercase()).collect();
    let has = |a: &str| attrs.iter().any(|x| x == a);
    let segments: Vec<String> = match delimiter.filter(|d| !d.is_empty()) {
        Some(d) => path.split(d).map(|s| utf7_imap::decode_utf7_imap(s.to_string())).collect(),
        None => vec![utf7_imap::decode_utf7_imap(path.to_string())],
    };
    let last = segments.last().cloned().unwrap_or_default();
    let make = |role: FolderRole, hidden: bool, name: String| FolderInfo {
        imap_path: path.to_string(),
        display_name: name,
        role,
        hidden,
    };
    if path.eq_ignore_ascii_case("INBOX") {
        return make(FolderRole::Inbox, false, role_name(FolderRole::Inbox).into());
    }
    let is_gmail_container = ["[gmail]", "[google mail]"].contains(&path.to_lowercase().as_str());
    if has("\\noselect") || has("\\nonexistent") || is_gmail_container {
        return make(FolderRole::Custom, true, last);
    }
    if has("\\all") || has("\\flagged") || has("\\important") {
        return make(FolderRole::Custom, true, last);
    }
    let by_attr = if has("\\sent") {
        Some(FolderRole::Sent)
    } else if has("\\drafts") {
        Some(FolderRole::Drafts)
    } else if has("\\trash") {
        Some(FolderRole::Trash)
    } else if has("\\junk") {
        Some(FolderRole::Spam)
    } else {
        None
    };
    let lower = last.to_lowercase();
    let by_name = if SENT.contains(&lower.as_str()) {
        Some(FolderRole::Sent)
    } else if DRAFTS.contains(&lower.as_str()) {
        Some(FolderRole::Drafts)
    } else if TRASH.contains(&lower.as_str()) {
        Some(FolderRole::Trash)
    } else if SPAM.contains(&lower.as_str()) {
        Some(FolderRole::Spam)
    } else {
        None
    };
    if let Some(role) = by_attr.or(by_name) {
        return make(role, false, role_name(role).into());
    }
    // Dossier perso : on retire le préfixe INBOX (OVH/Dovecot) ou [Gmail].
    let mut segs = segments.as_slice();
    if segs.len() > 1
        && (segs[0].eq_ignore_ascii_case("INBOX") || ["[gmail]", "[google mail]"].contains(&segs[0].to_lowercase().as_str()))
    {
        segs = &segs[1..];
    }
    make(FolderRole::Custom, false, segs.join(" / "))
}

fn attr_str(a: &NameAttribute<'_>) -> String {
    match a {
        NameAttribute::NoInferiors => "\\Noinferiors".into(),
        NameAttribute::NoSelect => "\\Noselect".into(),
        NameAttribute::Marked => "\\Marked".into(),
        NameAttribute::Unmarked => "\\Unmarked".into(),
        NameAttribute::All => "\\All".into(),
        NameAttribute::Archive => "\\Archive".into(),
        NameAttribute::Drafts => "\\Drafts".into(),
        NameAttribute::Flagged => "\\Flagged".into(),
        NameAttribute::Junk => "\\Junk".into(),
        NameAttribute::Sent => "\\Sent".into(),
        NameAttribute::Trash => "\\Trash".into(),
        NameAttribute::Extension(s) => s.to_string(),
        _ => String::new(),
    }
}

pub async fn list_folders(s: &mut ImapSession) -> Result<Vec<FolderInfo>> {
    let names: Vec<_> = s.list(Some(""), Some("*")).await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    Ok(names
        .iter()
        .map(|n| {
            let attrs: Vec<String> = n.attributes().iter().map(attr_str).collect();
            folder_info(n.name(), n.delimiter(), &attrs)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FolderRole;

    fn fi(path: &str, delim: &str, attrs: &[&str]) -> FolderInfo {
        folder_info(path, Some(delim), &attrs.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn inbox_any_case() {
        for p in ["INBOX", "Inbox", "inbox"] {
            let f = fi(p, "/", &[]);
            assert_eq!((f.role, f.display_name.as_str(), f.hidden), (FolderRole::Inbox, "Boîte de réception", false));
        }
    }

    #[test]
    fn special_use_wins() {
        let f = fi("[Gmail]/Messages envoy&AOk-s", "/", &["\\HasNoChildren", "\\Sent"]);
        assert_eq!((f.role, f.display_name.as_str()), (FolderRole::Sent, "Envoyés"));
        assert_eq!(fi("X", "/", &["\\Drafts"]).role, FolderRole::Drafts);
        assert_eq!(fi("X", "/", &["\\Trash"]).display_name, "Corbeille");
        assert_eq!(fi("X", "/", &["\\Junk"]).role, FolderRole::Spam);
    }

    #[test]
    fn gmail_all_hidden() {
        assert!(fi("[Gmail]/Tous les messages", "/", &["\\All"]).hidden);
        assert!(fi("[Gmail]/Important", "/", &["\\Important"]).hidden);
        assert!(fi("[Gmail]/Suivis", "/", &["\\Flagged"]).hidden);
    }

    #[test]
    fn gmail_container_hidden() {
        assert!(fi("[Gmail]", "/", &["\\Noselect", "\\HasChildren"]).hidden);
        assert!(fi("[Google Mail]", "/", &[]).hidden);
    }

    #[test]
    fn name_fallback_ovh() {
        let f = fi("INBOX.Sent", ".", &[]);
        assert_eq!((f.role, f.display_name.as_str()), (FolderRole::Sent, "Envoyés"));
        assert_eq!(fi("INBOX.Trash", ".", &[]).role, FolderRole::Trash);
        assert_eq!(fi("INBOX.Drafts", ".", &[]).role, FolderRole::Drafts);
    }

    #[test]
    fn yahoo_bulk_is_spam() {
        assert_eq!(fi("Bulk", "/", &[]).role, FolderRole::Spam);
        assert_eq!(fi("Bulk Mail", "/", &[]).role, FolderRole::Spam);
    }

    #[test]
    fn utf7_custom() {
        let f = fi("R&AOk-ception", "/", &[]);
        assert_eq!((f.role, f.display_name.as_str()), (FolderRole::Custom, "Réception"));
    }

    #[test]
    fn nested_custom_strips_inbox_prefix() {
        assert_eq!(fi("INBOX.Factures.2024", ".", &[]).display_name, "Factures / 2024");
        assert_eq!(fi("Travaux maison/Devis", "/", &[]).display_name, "Travaux maison / Devis");
    }
}
