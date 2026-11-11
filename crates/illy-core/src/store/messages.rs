use super::Store;
use crate::error::{CoreError, Result};
use crate::mime::{text_to_html, unblock_images, ParsedHeaders, PartInfo, PartKind, SanitizedHtml};
use crate::model::*;
use crate::providers::is_dangerous_filename;
use rusqlite::{params, params_from_iter, OptionalExtension, Row};

/// En-tête d'un mail récupéré par FETCH, prêt à être stocké.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewHeader {
    pub uid: u32,
    pub headers: ParsedHeaders,
    pub internal_date: i64,
    pub size: u64,
    pub seen: bool,
    pub answered: bool,
    pub parts: Vec<PartInfo>,
}

const SUMMARY_COLS: &str =
    "m.id, m.account_id, m.folder_id, m.from_name, m.from_addr, m.subject, m.snippet, m.date, m.seen, m.has_attachments";

fn row_to_summary(r: &Row) -> rusqlite::Result<MessageSummary> {
    Ok(MessageSummary {
        id: r.get(0)?,
        account_id: r.get(1)?,
        folder_id: r.get(2)?,
        from: Address { name: r.get(3)?, email: r.get(4)? },
        subject: r.get(5)?,
        snippet: r.get(6)?,
        date: r.get(7)?,
        seen: r.get(8)?,
        has_attachments: r.get(9)?,
    })
}

fn scope_filter(scope: ListScope) -> (String, Option<FolderId>) {
    match scope {
        ListScope::Folder(id) => ("m.folder_id = ?1".into(), Some(id)),
        ListScope::UnifiedInbox => (
            "m.folder_id IN (SELECT id FROM folders WHERE role = 'inbox' AND hidden = 0)".into(),
            None,
        ),
    }
}

fn row_to_part(r: &Row) -> rusqlite::Result<PartInfo> {
    Ok(PartInfo {
        part_path: r.get(0)?,
        mime_type: r.get(1)?,
        filename: r.get(2)?,
        size: r.get::<_, i64>(3)? as u64,
        content_id: r.get(4)?,
        encoding: r.get(5)?,
        charset: r.get(6)?,
        kind: PartKind::parse(&r.get::<_, String>(7)?),
    })
}

const PART_COLS: &str = "part_path, mime_type, filename, size, content_id, encoding, charset, kind";

impl Store {
    /// Insère les nouveaux en-têtes (et leurs parties) ; met à jour les drapeaux des existants.
    /// Retourne le nombre de mails ajoutés.
    pub fn upsert_headers(&self, account: AccountId, folder: FolderId, items: &[NewHeader]) -> Result<usize> {
        self.with(|c| {
            let tx = c.transaction()?;
            let mut added = 0;
            {
                let mut ins = tx.prepare(
                    "INSERT OR IGNORE INTO messages (account_id, folder_id, uid, message_id, in_reply_to, refs, \
                     subject, from_name, from_addr, to_json, cc_json, date, seen, answered, has_attachments, size) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                )?;
                let mut upd =
                    tx.prepare("UPDATE messages SET seen = ?1, answered = ?2 WHERE folder_id = ?3 AND uid = ?4")?;
                let mut part = tx.prepare(&format!(
                    "INSERT OR IGNORE INTO parts (message_id, {PART_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
                ))?;
                for h in items {
                    let p = &h.headers;
                    let date = p.date.filter(|d| *d > 0).unwrap_or(h.internal_date);
                    let has_att = h.parts.iter().any(|x| x.kind == PartKind::Attachment);
                    let n = ins.execute(params![
                        account,
                        folder,
                        h.uid,
                        p.message_id,
                        p.in_reply_to,
                        p.references.join(" "),
                        p.subject,
                        p.from.name,
                        p.from.email,
                        serde_json::to_string(&p.to)?,
                        serde_json::to_string(&p.cc)?,
                        date,
                        h.seen,
                        h.answered,
                        has_att,
                        h.size as i64
                    ])?;
                    if n == 0 {
                        upd.execute(params![h.seen, h.answered, folder, h.uid])?;
                        continue;
                    }
                    added += 1;
                    let id = tx.last_insert_rowid();
                    for x in &h.parts {
                        part.execute(params![
                            id,
                            x.part_path,
                            x.mime_type,
                            x.filename,
                            x.size as i64,
                            x.content_id,
                            x.encoding,
                            x.charset,
                            x.kind.as_str()
                        ])?;
                    }
                }
            }
            tx.commit()?;
            Ok(added)
        })
    }

    pub fn known_uids(&self, folder: FolderId) -> Result<Vec<u32>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT uid FROM messages WHERE folder_id = ?1 ORDER BY uid")?;
            let v = st.query_map([folder], |r| r.get(0))?.collect::<rusqlite::Result<Vec<u32>>>()?;
            Ok(v)
        })
    }

    pub fn delete_uids(&self, folder: FolderId, uids: &[u32]) -> Result<usize> {
        self.with(|c| {
            let tx = c.transaction()?;
            let mut n = 0;
            {
                let mut del = tx.prepare("DELETE FROM messages WHERE folder_id = ?1 AND uid = ?2")?;
                for u in uids {
                    n += del.execute(params![folder, u])?;
                }
            }
            tx.commit()?;
            Ok(n)
        })
    }

    pub fn clear_folder(&self, folder: FolderId) -> Result<()> {
        self.with(|c| {
            c.execute("DELETE FROM messages WHERE folder_id = ?1", [folder])?;
            Ok(())
        })
    }

    /// Applique les drapeaux du serveur ; retourne les ids des mails modifiés.
    pub fn apply_flags(&self, folder: FolderId, flags: &[(u32, bool, bool)]) -> Result<Vec<MessageId>> {
        self.with(|c| {
            let tx = c.transaction()?;
            let mut changed = Vec::new();
            {
                let mut q = tx.prepare(
                    "UPDATE messages SET seen = ?1, answered = ?2 \
                     WHERE folder_id = ?3 AND uid = ?4 AND (seen != ?1 OR answered != ?2) RETURNING id",
                )?;
                for (uid, seen, answered) in flags {
                    if let Some(id) = q.query_row(params![seen, answered, folder, uid], |r| r.get(0)).optional()? {
                        changed.push(id);
                    }
                }
            }
            tx.commit()?;
            Ok(changed)
        })
    }

    pub fn list_messages(&self, scope: ListScope, offset: u32, limit: u32) -> Result<Vec<MessageSummary>> {
        let (filter, folder) = scope_filter(scope);
        self.with(|c| {
            let sql = format!(
                "SELECT {SUMMARY_COLS} FROM messages m WHERE {filter} ORDER BY m.date DESC, m.id DESC \
                 LIMIT {limit} OFFSET {offset}"
            );
            let mut st = c.prepare(&sql)?;
            let rows = st.query_map(params_from_iter(folder), row_to_summary)?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    pub fn count_messages(&self, scope: ListScope) -> Result<u32> {
        let (filter, folder) = scope_filter(scope);
        self.with(|c| {
            Ok(c.query_row(
                &format!("SELECT COUNT(*) FROM messages m WHERE {filter}"),
                params_from_iter(folder),
                |r| r.get(0),
            )?)
        })
    }

    pub fn message_location(&self, id: MessageId) -> Result<(AccountId, FolderId, u32)> {
        self.with(|c| {
            c.query_row("SELECT account_id, folder_id, uid FROM messages WHERE id = ?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()?
            .ok_or(CoreError::NotFound)
        })
    }

    pub fn body_parts_to_fetch(&self, id: MessageId) -> Result<Vec<PartInfo>> {
        self.with(|c| {
            let mut st = c.prepare(&format!(
                "SELECT {PART_COLS} FROM parts WHERE message_id = ?1 AND kind != 'attachment' ORDER BY id"
            ))?;
            let v = st.query_map([id], row_to_part)?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(v)
        })
    }

    pub fn store_body(
        &self,
        id: MessageId,
        text: Option<&str>,
        html: Option<&SanitizedHtml>,
        snippet: &str,
        inline_data: &[(String, Vec<u8>)],
        degraded: bool,
    ) -> Result<()> {
        self.with(|c| {
            let tx = c.transaction()?;
            tx.execute(
                "UPDATE messages SET body_text = ?1, body_html = ?2, has_remote_images = ?3, snippet = ?4, \
                 degraded = ?5, body_state = 'ready' WHERE id = ?6",
                params![
                    text,
                    html.map(|h| h.html.as_str()),
                    html.map(|h| h.has_remote_images).unwrap_or(false),
                    snippet,
                    degraded,
                    id
                ],
            )?;
            for (path, data) in inline_data {
                tx.execute("UPDATE parts SET data = ?1 WHERE message_id = ?2 AND part_path = ?3", params![data, id, path])?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    pub fn store_part_data(&self, id: MessageId, part_path: &str, data: &[u8]) -> Result<()> {
        self.with(|c| {
            c.execute("UPDATE parts SET data = ?1 WHERE message_id = ?2 AND part_path = ?3", params![data, id, part_path])?;
            Ok(())
        })
    }

    pub fn part(&self, id: MessageId, part_path: &str) -> Result<(PartInfo, Option<Vec<u8>>)> {
        self.with(|c| {
            c.query_row(
                &format!("SELECT {PART_COLS}, data FROM parts WHERE message_id = ?1 AND part_path = ?2"),
                params![id, part_path],
                |r| Ok((row_to_part(r)?, r.get(8)?)),
            )
            .optional()?
            .ok_or(CoreError::NotFound)
        })
    }

    pub fn message_detail(&self, id: MessageId, show_images: bool) -> Result<Option<MessageDetail>> {
        let row = self.with(|c| {
            Ok(c.query_row(
                &format!(
                    "SELECT {SUMMARY_COLS}, m.to_json, m.cc_json, m.body_state, m.body_text, m.body_html, \
                     m.has_remote_images, m.degraded FROM messages m WHERE m.id = ?1"
                ),
                [id],
                |r| {
                    Ok((
                        row_to_summary(r)?,
                        r.get::<_, String>(10)?,
                        r.get::<_, String>(11)?,
                        r.get::<_, String>(12)?,
                        r.get::<_, Option<String>>(13)?,
                        r.get::<_, Option<String>>(14)?,
                        r.get::<_, bool>(15)?,
                        r.get::<_, bool>(16)?,
                    ))
                },
            )
            .optional()?)
        })?;
        let Some((summary, to_json, cc_json, state, text, html, remote, degraded)) = row else {
            return Err(CoreError::NotFound);
        };
        if state != "ready" {
            return Ok(None);
        }
        let allowed = remote && !show_images && self.images_allowed_for(&summary.from.email)?;
        let images_blocked = remote && !show_images && !allowed;
        let body = match html {
            Some(h) if images_blocked => MessageBody::Html(h),
            Some(h) => MessageBody::Html(unblock_images(&h)),
            None => MessageBody::Text(text_to_html(text.as_deref().unwrap_or(""))),
        };
        let attachments = self.with(|c| {
            let mut st = c.prepare(
                "SELECT part_path, filename, mime_type, size FROM parts \
                 WHERE message_id = ?1 AND kind = 'attachment' ORDER BY id",
            )?;
            let v = st
                .query_map([id], |r| {
                    let filename: String = r.get::<_, Option<String>>(1)?.unwrap_or_else(|| "piece-jointe".into());
                    Ok(Attachment {
                        part_path: r.get(0)?,
                        can_open: !is_dangerous_filename(&filename),
                        filename,
                        mime_type: r.get(2)?,
                        size: r.get::<_, i64>(3)? as u64,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(v)
        })?;
        Ok(Some(MessageDetail {
            summary,
            to: serde_json::from_str(&to_json).unwrap_or_default(),
            cc: serde_json::from_str(&cc_json).unwrap_or_default(),
            body,
            attachments,
            images_blocked,
            degraded,
        }))
    }

    /// Change le drapeau « lu » localement ; `true` si la valeur a changé.
    pub fn set_seen_local(&self, id: MessageId, seen: bool) -> Result<bool> {
        self.with(|c| Ok(c.execute("UPDATE messages SET seen = ?1 WHERE id = ?2 AND seen != ?1", params![seen, id])? > 0))
    }

    pub fn recent_without_body(&self, account: AccountId, since: i64, limit: u32) -> Result<Vec<MessageId>> {
        self.ids_without_body(account, since, limit)
    }

    pub fn any_without_body(&self, account: AccountId, limit: u32) -> Result<Vec<MessageId>> {
        self.ids_without_body(account, i64::MIN, limit)
    }

    fn ids_without_body(&self, account: AccountId, since: i64, limit: u32) -> Result<Vec<MessageId>> {
        self.with(|c| {
            let mut st = c.prepare(
                "SELECT id FROM messages WHERE account_id = ?1 AND body_state = 'none' AND date >= ?2 \
                 ORDER BY date DESC, id DESC LIMIT ?3",
            )?;
            let v = st.query_map(params![account, since, limit], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(v)
        })
    }

    /// Supprime un mail localement (disparu du serveur).
    pub fn delete_message(&self, id: MessageId) -> Result<()> {
        self.with(|c| {
            c.execute("DELETE FROM messages WHERE id = ?1", [id])?;
            Ok(())
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::mime::{ParsedHeaders, PartInfo, PartKind, SanitizedHtml};
    use crate::store::accounts::tests::new_account;
    use crate::store::{FolderInfo, Store};

    pub(crate) fn setup(s: &Store, email: &str, folders: &[(&str, FolderRole)]) -> (AccountId, Vec<FolderId>) {
        let a = s.insert_account(&new_account(email)).unwrap();
        let infos: Vec<FolderInfo> = folders
            .iter()
            .map(|(p, r)| FolderInfo { imap_path: p.to_string(), display_name: p.to_string(), role: *r, hidden: false })
            .collect();
        s.upsert_folders(a.id, &infos).unwrap();
        let all = s.syncable_folders(a.id).unwrap();
        let ids = folders.iter().map(|(p, _)| all.iter().find(|f| f.1 == *p).unwrap().0).collect();
        (a.id, ids)
    }

    pub(crate) fn header(uid: u32, date: i64, from: &str) -> NewHeader {
        NewHeader {
            uid,
            headers: ParsedHeaders {
                subject: format!("Sujet {uid}"),
                from: Address { name: Some("Marie".into()), email: from.into() },
                date: Some(date),
                message_id: Some(format!("m{uid}@x")),
                ..Default::default()
            },
            internal_date: date,
            size: 100,
            seen: false,
            answered: false,
            parts: vec![],
        }
    }

    fn part(path: &str, kind: PartKind, filename: Option<&str>, cid: Option<&str>) -> PartInfo {
        PartInfo {
            part_path: path.into(),
            mime_type: match kind {
                PartKind::BodyHtml => "text/html".into(),
                PartKind::BodyText => "text/plain".into(),
                PartKind::Inline => "image/png".into(),
                PartKind::Attachment => "application/pdf".into(),
            },
            filename: filename.map(str::to_string),
            size: 10,
            content_id: cid.map(str::to_string),
            encoding: "base64".into(),
            charset: None,
            kind,
        }
    }

    #[test]
    fn upsert_and_list_by_folder() {
        let s = Store::open_in_memory().unwrap();
        let (a, f) = setup(&s, "a@x.fr", &[("INBOX", FolderRole::Inbox)]);
        let n = s.upsert_headers(a, f[0], &[header(1, 1, "m@x"), header(2, 3, "m@x"), header(3, 2, "m@x")]).unwrap();
        assert_eq!(n, 3);
        let l = s.list_messages(ListScope::Folder(f[0]), 0, 10).unwrap();
        assert_eq!(l.iter().map(|m| m.date).collect::<Vec<_>>(), [3, 2, 1]);
        assert_eq!(l[0].subject, "Sujet 2");
        assert_eq!(l[0].from.email, "m@x");
        assert_eq!(s.count_messages(ListScope::Folder(f[0])).unwrap(), 3);
        assert_eq!(s.upsert_headers(a, f[0], &[header(1, 1, "m@x")]).unwrap(), 0);
        assert_eq!(s.count_messages(ListScope::Folder(f[0])).unwrap(), 3);
        assert_eq!(s.known_uids(f[0]).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn date_falls_back_to_internal_date() {
        let s = Store::open_in_memory().unwrap();
        let (a, f) = setup(&s, "a@x.fr", &[("INBOX", FolderRole::Inbox)]);
        let mut h = header(1, 0, "m@x");
        h.headers.date = None;
        h.internal_date = 42;
        s.upsert_headers(a, f[0], &[h]).unwrap();
        assert_eq!(s.list_messages(ListScope::Folder(f[0]), 0, 1).unwrap()[0].date, 42);
    }

    #[test]
    fn unified_inbox_spans_accounts() {
        let s = Store::open_in_memory().unwrap();
        let (a1, f1) = setup(&s, "a@x.fr", &[("INBOX", FolderRole::Inbox), ("Factures", FolderRole::Custom)]);
        let (a2, f2) = setup(&s, "b@x.fr", &[("INBOX", FolderRole::Inbox)]);
        s.upsert_headers(a1, f1[0], &[header(1, 10, "x@x")]).unwrap();
        s.upsert_headers(a1, f1[1], &[header(1, 30, "x@x")]).unwrap();
        s.upsert_headers(a2, f2[0], &[header(1, 20, "y@x")]).unwrap();
        let l = s.list_messages(ListScope::UnifiedInbox, 0, 10).unwrap();
        assert_eq!(l.iter().map(|m| (m.account_id, m.date)).collect::<Vec<_>>(), [(a2, 20), (a1, 10)]);
        assert_eq!(s.count_messages(ListScope::UnifiedInbox).unwrap(), 2);
    }

    #[test]
    fn paging() {
        let s = Store::open_in_memory().unwrap();
        let (a, f) = setup(&s, "a@x.fr", &[("INBOX", FolderRole::Inbox)]);
        let hs: Vec<_> = (1..=250).map(|i| header(i, i as i64, "m@x")).collect();
        s.upsert_headers(a, f[0], &hs).unwrap();
        assert_eq!(s.list_messages(ListScope::Folder(f[0]), 200, 100).unwrap().len(), 50);
    }

    #[test]
    fn apply_flags_reports_changes() {
        let s = Store::open_in_memory().unwrap();
        let (a, f) = setup(&s, "a@x.fr", &[("INBOX", FolderRole::Inbox)]);
        s.upsert_headers(a, f[0], &[header(1, 1, "m@x"), header(2, 2, "m@x")]).unwrap();
        assert_eq!(s.list_folders(a).unwrap()[0].unread_count, 2);
        let changed = s.apply_flags(f[0], &[(1, true, false), (2, false, false)]).unwrap();
        let id1 = s.list_messages(ListScope::Folder(f[0]), 0, 10).unwrap().into_iter().find(|m| m.date == 1).unwrap().id;
        assert_eq!(changed, vec![id1]);
        assert_eq!(s.list_folders(a).unwrap()[0].unread_count, 1);
    }

    #[test]
    fn delete_and_clear() {
        let s = Store::open_in_memory().unwrap();
        let (a, f) = setup(&s, "a@x.fr", &[("INBOX", FolderRole::Inbox)]);
        s.upsert_headers(a, f[0], &[header(1, 1, "m@x"), header(2, 2, "m@x"), header(3, 3, "m@x")]).unwrap();
        assert_eq!(s.delete_uids(f[0], &[2]).unwrap(), 1);
        assert_eq!(s.known_uids(f[0]).unwrap(), vec![1, 3]);
        s.clear_folder(f[0]).unwrap();
        assert_eq!(s.count_messages(ListScope::Folder(f[0])).unwrap(), 0);
    }

    fn with_body(s: &Store, html: &str, remote: bool) -> (MessageId, AccountId) {
        let (a, f) = setup(s, "a@x.fr", &[("INBOX", FolderRole::Inbox)]);
        let mut h = header(1, 1, "marie@ex.fr");
        h.parts = vec![
            part("1", PartKind::BodyHtml, None, None),
            part("2", PartKind::Inline, None, Some("logo@x")),
            part("3", PartKind::Attachment, Some("devis.pdf"), None),
            part("4", PartKind::Attachment, Some("setup.exe"), None),
        ];
        s.upsert_headers(a, f[0], &[h]).unwrap();
        let id = s.list_messages(ListScope::Folder(f[0]), 0, 1).unwrap()[0].id;
        let san = SanitizedHtml { html: html.into(), has_remote_images: remote };
        s.store_body(id, Some("texte"), Some(&san), "extrait", &[("2".into(), vec![1, 2, 3])], false).unwrap();
        (id, a)
    }

    #[test]
    fn detail_none_until_body() {
        let s = Store::open_in_memory().unwrap();
        let (a, f) = setup(&s, "a@x.fr", &[("INBOX", FolderRole::Inbox)]);
        s.upsert_headers(a, f[0], &[header(1, 1, "m@x")]).unwrap();
        let id = s.list_messages(ListScope::Folder(f[0]), 0, 1).unwrap()[0].id;
        assert!(s.message_detail(id, false).unwrap().is_none());
        let san = SanitizedHtml { html: "<p>x</p>".into(), has_remote_images: false };
        s.store_body(id, None, Some(&san), "x", &[], false).unwrap();
        let d = s.message_detail(id, false).unwrap().unwrap();
        assert_eq!(d.body, MessageBody::Html("<p>x</p>".into()));
        assert_eq!(s.list_messages(ListScope::Folder(f[0]), 0, 1).unwrap()[0].snippet, "x");
    }

    #[test]
    fn text_only_body_becomes_safe_html() {
        let s = Store::open_in_memory().unwrap();
        let (a, f) = setup(&s, "a@x.fr", &[("INBOX", FolderRole::Inbox)]);
        s.upsert_headers(a, f[0], &[header(1, 1, "m@x")]).unwrap();
        let id = s.list_messages(ListScope::Folder(f[0]), 0, 1).unwrap()[0].id;
        s.store_body(id, Some("a<b>"), None, "a", &[], false).unwrap();
        match s.message_detail(id, false).unwrap().unwrap().body {
            MessageBody::Text(t) => assert!(t.contains("a&lt;b&gt;"), "{t}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn detail_blocks_then_shows_images() {
        let s = Store::open_in_memory().unwrap();
        let (id, _) = with_body(&s, r#"<img data-blocked-src="https://t.example/p.gif">"#, true);
        let d = s.message_detail(id, false).unwrap().unwrap();
        assert!(d.images_blocked);
        assert!(matches!(&d.body, MessageBody::Html(h) if h.contains("data-blocked-src")));
        let d = s.message_detail(id, true).unwrap().unwrap();
        assert!(!d.images_blocked);
        assert!(matches!(&d.body, MessageBody::Html(h) if h.contains(r#"src="https://"#)));
        s.allow_images_from("marie@ex.fr").unwrap();
        assert!(!s.message_detail(id, false).unwrap().unwrap().images_blocked);
    }

    #[test]
    fn attachments_can_open() {
        let s = Store::open_in_memory().unwrap();
        let (id, _) = with_body(&s, "<p>x</p>", false);
        let d = s.message_detail(id, false).unwrap().unwrap();
        let names: Vec<_> = d.attachments.iter().map(|a| (a.filename.as_str(), a.can_open)).collect();
        assert_eq!(names, [("devis.pdf", true), ("setup.exe", false)]);
        assert!(d.summary.has_attachments);
    }

    #[test]
    fn parts_and_data() {
        let s = Store::open_in_memory().unwrap();
        let (id, _) = with_body(&s, "<p>x</p>", false);
        let to_fetch: Vec<_> = s.body_parts_to_fetch(id).unwrap().into_iter().map(|p| p.part_path).collect();
        assert_eq!(to_fetch, ["1", "2"]);
        let (p, data) = s.part(id, "2").unwrap();
        assert_eq!(p.content_id.as_deref(), Some("logo@x"));
        assert_eq!(data, Some(vec![1, 2, 3]));
        assert_eq!(s.part(id, "3").unwrap().1, None);
        s.store_part_data(id, "3", b"%PDF").unwrap();
        assert_eq!(s.part(id, "3").unwrap().1, Some(b"%PDF".to_vec()));
        assert!(matches!(s.part(id, "9"), Err(crate::CoreError::NotFound)));
        let (_, _, uid) = s.message_location(id).unwrap();
        assert_eq!(uid, 1);
    }

    #[test]
    fn set_seen_local_idempotent() {
        let s = Store::open_in_memory().unwrap();
        let (a, f) = setup(&s, "a@x.fr", &[("INBOX", FolderRole::Inbox)]);
        s.upsert_headers(a, f[0], &[header(1, 1, "m@x")]).unwrap();
        let id = s.list_messages(ListScope::Folder(f[0]), 0, 1).unwrap()[0].id;
        assert!(s.set_seen_local(id, true).unwrap());
        assert!(!s.set_seen_local(id, true).unwrap());
        assert!(s.list_messages(ListScope::Folder(f[0]), 0, 1).unwrap()[0].seen);
    }

    #[test]
    fn bodies_to_backfill_newest_first() {
        let s = Store::open_in_memory().unwrap();
        let (a, f) = setup(&s, "a@x.fr", &[("INBOX", FolderRole::Inbox)]);
        s.upsert_headers(a, f[0], &[header(1, 100, "m@x"), header(2, 300, "m@x"), header(3, 200, "m@x")]).unwrap();
        let ids = s.recent_without_body(a, 150, 10).unwrap();
        let l = s.list_messages(ListScope::Folder(f[0]), 0, 10).unwrap();
        assert_eq!(ids, vec![l[0].id, l[1].id]);
        assert_eq!(s.any_without_body(a, 10).unwrap().len(), 3);
        s.store_body(l[0].id, Some("t"), None, "t", &[], false).unwrap();
        assert_eq!(s.any_without_body(a, 10).unwrap().len(), 2);
    }
}
