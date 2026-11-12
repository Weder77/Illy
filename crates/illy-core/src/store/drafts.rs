use super::{dec, enc, now_ts, Store};
use crate::error::{CoreError, Result};
use crate::model::*;
use rusqlite::{params, OptionalExtension, Row};

const COLS: &str = "id, account_id, mode, source_message_id, to_json, cc_json, bcc_json, subject, body_html, \
                    attachments_json, state, last_error, send_at, remote_uid, updated_at";

fn row_to_draft(r: &Row) -> rusqlite::Result<Draft> {
    let js = |i: usize| -> rusqlite::Result<Vec<Address>> { Ok(serde_json::from_str(&r.get::<_, String>(i)?).unwrap_or_default()) };
    Ok(Draft {
        id: r.get(0)?,
        account_id: r.get(1)?,
        kind: dec(&r.get::<_, String>(2)?)?,
        source_message_id: r.get(3)?,
        to: js(4)?,
        cc: js(5)?,
        bcc: js(6)?,
        subject: r.get(7)?,
        body_html: r.get(8)?,
        attachments: serde_json::from_str(&r.get::<_, String>(9)?).unwrap_or_default(),
        state: dec(&r.get::<_, String>(10)?)?,
        last_error: r.get(11)?,
        send_at: r.get(12)?,
        remote_uid: r.get(13)?,
        updated_at: r.get(14)?,
    })
}

impl Store {
    pub fn insert_draft(&self, d: &Draft) -> Result<DraftId> {
        self.with(|c| {
            c.execute(
                "INSERT INTO drafts (account_id, mode, source_message_id, to_json, cc_json, bcc_json, subject, body_html, \
                 attachments_json, state, remote_uid, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    d.account_id,
                    enc(&d.kind),
                    d.source_message_id,
                    serde_json::to_string(&d.to)?,
                    serde_json::to_string(&d.cc)?,
                    serde_json::to_string(&d.bcc)?,
                    d.subject,
                    d.body_html,
                    serde_json::to_string(&d.attachments)?,
                    enc(&d.state),
                    d.remote_uid,
                    now_ts()
                ],
            )?;
            Ok(c.last_insert_rowid())
        })
    }

    pub fn get_draft(&self, id: DraftId) -> Result<Draft> {
        self.with_read(|c| {
            c.query_row(&format!("SELECT {COLS} FROM drafts WHERE id = ?1"), [id], row_to_draft)
                .optional()?
                .ok_or(CoreError::NotFound)
        })
    }

    pub fn update_draft(&self, d: &DraftInput) -> Result<Draft> {
        let n = self.with(|c| {
            Ok(c.execute(
                "UPDATE drafts SET account_id = ?1, to_json = ?2, cc_json = ?3, bcc_json = ?4, subject = ?5, body_html = ?6, \
                 updated_at = ?7 WHERE id = ?8 AND state IN ('editing', 'failed')",
                params![
                    d.account_id,
                    serde_json::to_string(&d.to)?,
                    serde_json::to_string(&d.cc)?,
                    serde_json::to_string(&d.bcc)?,
                    d.subject,
                    d.body_html,
                    now_ts(),
                    d.id
                ],
            )?)
        })?;
        if n == 0 {
            self.get_draft(d.id)?;
            return Err(CoreError::InvalidInput("brouillon en cours d'envoi".into()));
        }
        self.get_draft(d.id)
    }

    pub fn set_draft_state(&self, id: DraftId, state: DraftState, error: Option<&str>, send_at: Option<i64>) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE drafts SET state = ?1, last_error = ?2, send_at = ?3 WHERE id = ?4",
                params![enc(&state), error, send_at, id],
            )?;
            Ok(())
        })
    }

    pub fn set_draft_attachments(&self, id: DraftId, atts: &[DraftAttachment]) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE drafts SET attachments_json = ?1, updated_at = ?2 WHERE id = ?3",
                params![serde_json::to_string(atts)?, now_ts(), id],
            )?;
            Ok(())
        })
    }

    pub fn set_draft_remote_uid(&self, id: DraftId, uid: Option<u32>) -> Result<()> {
        self.with(|c| {
            c.execute("UPDATE drafts SET remote_uid = ?1 WHERE id = ?2", params![uid, id])?;
            Ok(())
        })
    }

    pub fn draft_for_message(&self, message_id: MessageId) -> Result<Option<Draft>> {
        self.with_read(|c| {
            Ok(c.query_row(
                &format!(
                    "SELECT {COLS} FROM drafts WHERE source_message_id = ?1 AND state IN ('editing', 'failed') \
                     ORDER BY updated_at DESC, id DESC LIMIT 1"
                ),
                [message_id],
                row_to_draft,
            )
            .optional()?)
        })
    }

    pub fn delete_draft(&self, id: DraftId) -> Result<()> {
        self.with(|c| {
            c.execute("DELETE FROM drafts WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    /// Envois en attente, en cours ou en échec.
    pub fn outbox(&self) -> Result<Vec<Draft>> {
        self.with_read(|c| {
            let mut st =
                c.prepare(&format!("SELECT {COLS} FROM drafts WHERE state IN ('queued', 'sending', 'failed') ORDER BY id"))?;
            let v = st.query_map([], row_to_draft)?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(v)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::model::*;
    use crate::store::accounts::tests::new_account;
    use crate::store::Store;
    use crate::CoreError;

    fn draft(account: AccountId, source: Option<MessageId>) -> Draft {
        Draft {
            id: 0,
            account_id: account,
            kind: if source.is_some() { DraftKind::Reply } else { DraftKind::New },
            source_message_id: source,
            to: vec![Address { name: None, email: "bob@ex.fr".into() }],
            cc: vec![],
            bcc: vec![],
            subject: "Salut".into(),
            body_html: "<p>Bonjour</p>".into(),
            attachments: vec![DraftAttachment {
                filename: "a.pdf".into(),
                mime_type: "application/pdf".into(),
                size: 3,
                source: AttachmentSource::File { path: "/tmp/a.pdf".into() },
            }],
            state: DraftState::Editing,
            last_error: None,
            send_at: None,
            remote_uid: None,
            updated_at: 0,
        }
    }

    #[test]
    fn drafts_roundtrip_and_states() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert_account(&new_account("a@x.fr")).unwrap().id;
        let id = s.insert_draft(&draft(a, None)).unwrap();
        let d = s.get_draft(id).unwrap();
        assert_eq!(d.subject, "Salut");
        assert_eq!(d.attachments.len(), 1);
        assert_eq!(d.state, DraftState::Editing);
        let input = DraftInput { id, account_id: a, to: d.to.clone(), cc: vec![], bcc: vec![], subject: "Modifié".into(), body_html: "<p>x</p>".into() };
        assert_eq!(s.update_draft(&input).unwrap().subject, "Modifié");
        s.set_draft_state(id, DraftState::Queued, None, Some(123)).unwrap();
        assert!(matches!(s.update_draft(&input), Err(CoreError::InvalidInput(_))));
        assert_eq!(s.get_draft(id).unwrap().send_at, Some(123));
        let id2 = s.insert_draft(&draft(a, None)).unwrap();
        let id3 = s.insert_draft(&draft(a, None)).unwrap();
        s.set_draft_state(id3, DraftState::Failed, Some("refusé"), None).unwrap();
        let out: Vec<DraftId> = s.outbox().unwrap().iter().map(|d| d.id).collect();
        assert_eq!(out, vec![id, id3]);
        assert!(!out.contains(&id2));
        s.set_draft_remote_uid(id2, Some(9)).unwrap();
        assert_eq!(s.get_draft(id2).unwrap().remote_uid, Some(9));
        s.delete_draft(id2).unwrap();
        assert!(matches!(s.get_draft(id2), Err(CoreError::NotFound)));
    }

    #[test]
    fn draft_for_message_finds_reply() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert_account(&new_account("a@x.fr")).unwrap().id;
        assert!(s.draft_for_message(5).unwrap().is_none());
        let id = s.insert_draft(&draft(a, Some(5))).unwrap();
        assert_eq!(s.draft_for_message(5).unwrap().unwrap().id, id);
        s.set_draft_state(id, DraftState::Queued, None, None).unwrap();
        assert!(s.draft_for_message(5).unwrap().is_none(), "a queued reply is not resumable");
    }
}
