use super::conn::{imap_err, ImapSession};
use crate::error::{CoreError, Result};
use crate::mime::{flatten_structure, parse_header_block};
use crate::store::NewHeader;
use async_imap::types::Flag;
use futures::TryStreamExt;
use imap_proto::SectionPath;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selected {
    pub uidvalidity: u32,
    pub uidnext: Option<u32>,
    pub highest_modseq: Option<u64>,
    pub exists: u32,
}

/// Compacte une liste d'UID en ensemble IMAP (`1:3,7,9:10`).
pub fn uid_set(uids: &[u32]) -> String {
    let mut v = uids.to_vec();
    v.sort_unstable();
    v.dedup();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let start = v[i];
        let mut end = start;
        while i + 1 < v.len() && v[i + 1] == end + 1 {
            i += 1;
            end = v[i];
        }
        out.push(if start == end { start.to_string() } else { format!("{start}:{end}") });
        i += 1;
    }
    out.join(",")
}

pub async fn select(s: &mut ImapSession, path: &str, condstore: bool) -> Result<Selected> {
    let mb = if condstore { s.select_condstore(path).await } else { s.select(path).await }.map_err(imap_err)?;
    Ok(Selected {
        uidvalidity: mb.uid_validity.unwrap_or(0),
        uidnext: mb.uid_next,
        highest_modseq: mb.highest_modseq,
        exists: mb.exists,
    })
}

pub async fn search_all_uids(s: &mut ImapSession) -> Result<Vec<u32>> {
    let mut v: Vec<u32> = s.uid_search("ALL").await.map_err(imap_err)?.into_iter().collect();
    v.sort_unstable();
    Ok(v)
}

const HEADER_FIELDS: &str = "BODY.PEEK[HEADER.FIELDS (FROM TO CC REPLY-TO SUBJECT DATE MESSAGE-ID IN-REPLY-TO REFERENCES)]";

/// En-têtes d'un lot de mails. `with_structure = false` évite BODYSTRUCTURE (mode sûr après
/// une réponse illisible) : les parties restent vides et le corps sera lu en entier.
pub async fn fetch_headers(s: &mut ImapSession, uids: &[u32], with_structure: bool) -> Result<Vec<NewHeader>> {
    if uids.is_empty() {
        return Ok(vec![]);
    }
    let query = format!(
        "(UID FLAGS INTERNALDATE RFC822.SIZE{} {HEADER_FIELDS})",
        if with_structure { " BODYSTRUCTURE" } else { "" }
    );
    let fetches: Vec<_> =
        s.uid_fetch(uid_set(uids), query).await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    let mut out = Vec::with_capacity(fetches.len());
    for f in &fetches {
        let Some(uid) = f.uid else { continue };
        let flags: Vec<Flag<'_>> = f.flags().collect();
        out.push(NewHeader {
            uid,
            headers: parse_header_block(f.header().unwrap_or_default()),
            internal_date: f.internal_date().map(|d| d.timestamp()).unwrap_or(0),
            size: f.size.unwrap_or(0) as u64,
            seen: flags.contains(&Flag::Seen),
            answered: flags.contains(&Flag::Answered),
            parts: f.bodystructure().map(flatten_structure).unwrap_or_default(),
        });
    }
    Ok(out)
}

pub async fn fetch_flags(s: &mut ImapSession, uid_set: &str, changed_since: Option<u64>) -> Result<Vec<(u32, bool, bool)>> {
    if uid_set.is_empty() {
        return Ok(vec![]);
    }
    let query = match changed_since {
        Some(m) => format!("(UID FLAGS) (CHANGEDSINCE {m})"),
        None => "(UID FLAGS)".to_string(),
    };
    let fetches: Vec<_> = s.uid_fetch(uid_set, query).await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    let mut out: Vec<(u32, bool, bool)> = fetches
        .iter()
        .filter_map(|f| {
            let flags: Vec<Flag<'_>> = f.flags().collect();
            f.uid.map(|u| (u, flags.contains(&Flag::Seen), flags.contains(&Flag::Answered)))
        })
        .collect();
    out.sort_unstable();
    Ok(out)
}

/// Récupère le corps brut d'une partie sans marquer le mail comme lu.
/// Le décodage se fait avec `mime::synth_mime_header` et les infos de BODYSTRUCTURE.
pub async fn fetch_part(s: &mut ImapSession, uid: u32, part_path: &str) -> Result<Vec<u8>> {
    let nums: Vec<u32> = part_path
        .split('.')
        .map(|n| n.parse().map_err(|_| CoreError::InvalidInput(format!("partie invalide : {part_path}"))))
        .collect::<Result<_>>()?;
    let query = format!("(UID BODY.PEEK[{part_path}])");
    let fetches: Vec<_> =
        s.uid_fetch(uid.to_string(), query).await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    let f = fetches.iter().find(|f| f.uid == Some(uid)).ok_or(CoreError::NotFound)?;
    Ok(f.section(&SectionPath::Part(nums, None)).unwrap_or_default().to_vec())
}

/// Mail complet (secours) ; ne le marque pas comme lu.
pub async fn fetch_full(s: &mut ImapSession, uid: u32) -> Result<Vec<u8>> {
    let fetches: Vec<_> =
        s.uid_fetch(uid.to_string(), "(UID BODY.PEEK[])").await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    let f = fetches.iter().find(|f| f.uid == Some(uid)).ok_or(CoreError::NotFound)?;
    Ok(f.body().unwrap_or_default().to_vec())
}

/// Dépose un message dans un dossier (copie dans Envoyés, brouillon serveur).
pub async fn append(s: &mut ImapSession, folder: &str, flags: &str, raw: &[u8]) -> Result<()> {
    s.append(folder, Some(flags), None, raw).await.map_err(imap_err)
}

/// UID des mails du dossier sélectionné dont l'en-tête `field` contient `value`.
pub async fn search_header(s: &mut ImapSession, field: &str, value: &str) -> Result<Vec<u32>> {
    let field: String = field.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    let value = value.replace('\\', "\\\\").replace('"', "\\\"");
    let mut v: Vec<u32> = s.uid_search(format!("HEADER {field} \"{value}\"")).await.map_err(imap_err)?.into_iter().collect();
    v.sort_unstable();
    Ok(v)
}

pub async fn add_flag(s: &mut ImapSession, uid: u32, flag: &str) -> Result<()> {
    let _: Vec<_> = s
        .uid_store(uid.to_string(), format!("+FLAGS.SILENT ({flag})"))
        .await
        .map_err(imap_err)?
        .try_collect()
        .await
        .map_err(imap_err)?;
    Ok(())
}

/// Supprime définitivement un mail du dossier sélectionné.
pub async fn delete_uid(s: &mut ImapSession, uid: u32, caps: &super::Caps) -> Result<()> {
    add_flag(s, uid, "\\Deleted").await?;
    if caps.uidplus {
        let _: Vec<_> = s.uid_expunge(uid.to_string()).await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    } else {
        let _: Vec<_> = s.expunge().await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    }
    Ok(())
}

pub async fn store_seen(s: &mut ImapSession, uid: u32, seen: bool) -> Result<()> {
    let q = if seen { "+FLAGS.SILENT (\\Seen)" } else { "-FLAGS.SILENT (\\Seen)" };
    let _: Vec<_> = s.uid_store(uid.to_string(), q).await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uid_set_compresses() {
        assert_eq!(uid_set(&[1, 2, 3, 7, 9, 10]), "1:3,7,9:10");
        assert_eq!(uid_set(&[10, 9, 1]), "1,9:10");
        assert_eq!(uid_set(&[]), "");
    }
}
