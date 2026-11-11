use super::{dec, enc, Store};
use crate::error::Result;
use crate::model::{AccountId, Folder, FolderId, FolderRole};
use rusqlite::params;

/// Dossier tel que listé par le serveur (après détection du rôle).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderInfo {
    pub imap_path: String,
    pub display_name: String,
    pub role: FolderRole,
    pub hidden: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FolderSyncState {
    pub uidvalidity: Option<u32>,
    pub uidnext: Option<u32>,
    pub highest_modseq: Option<u64>,
    pub last_sync_at: Option<i64>,
}

impl Store {
    pub fn upsert_folders(&self, account: AccountId, found: &[FolderInfo]) -> Result<()> {
        self.with(|c| {
            let tx = c.transaction()?;
            {
                let mut up = tx.prepare(
                    "INSERT INTO folders (account_id, imap_path, display_name, role, hidden) \
                     VALUES (?1, ?2, ?3, ?4, ?5) \
                     ON CONFLICT(account_id, imap_path) DO UPDATE SET \
                     display_name = excluded.display_name, role = excluded.role, hidden = excluded.hidden",
                )?;
                for f in found {
                    up.execute(params![account, f.imap_path, f.display_name, enc(&f.role), f.hidden])?;
                }
                let mut existing = tx.prepare("SELECT id, imap_path FROM folders WHERE account_id = ?1")?;
                let rows: Vec<(FolderId, String)> = existing
                    .query_map([account], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .collect::<rusqlite::Result<_>>()?;
                for (id, path) in rows {
                    if !found.iter().any(|f| f.imap_path == path) {
                        tx.execute("DELETE FROM folders WHERE id = ?1", [id])?;
                    }
                }
            }
            tx.commit()?;
            Ok(())
        })
    }

    pub fn list_folders(&self, account: AccountId) -> Result<Vec<Folder>> {
        let mut out = self.with(|c| {
            let mut st = c.prepare(
                "SELECT f.id, f.account_id, f.imap_path, f.display_name, f.role, \
                 (SELECT COUNT(*) FROM messages m WHERE m.folder_id = f.id AND m.seen = 0), \
                 (SELECT COUNT(*) FROM messages m WHERE m.folder_id = f.id) \
                 FROM folders f WHERE f.account_id = ?1 AND f.hidden = 0",
            )?;
            let rows = st
                .query_map([account], |r| {
                    Ok(Folder {
                        id: r.get(0)?,
                        account_id: r.get(1)?,
                        imap_path: r.get(2)?,
                        display_name: r.get(3)?,
                        role: dec(&r.get::<_, String>(4)?)?,
                        unread_count: r.get(5)?,
                        total_count: r.get(6)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })?;
        out.sort_by(|a, b| {
            a.role.cmp(&b.role).then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
        });
        Ok(out)
    }

    pub fn folder_state(&self, id: FolderId) -> Result<FolderSyncState> {
        self.with(|c| {
            Ok(c.query_row(
                "SELECT uidvalidity, uidnext, highest_modseq, last_sync_at FROM folders WHERE id = ?1",
                [id],
                |r| {
                    Ok(FolderSyncState {
                        uidvalidity: r.get(0)?,
                        uidnext: r.get(1)?,
                        highest_modseq: r.get::<_, Option<i64>>(2)?.map(|v| v as u64),
                        last_sync_at: r.get(3)?,
                    })
                },
            )?)
        })
    }

    pub fn set_folder_state(&self, id: FolderId, st: &FolderSyncState) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE folders SET uidvalidity = ?1, uidnext = ?2, highest_modseq = ?3, last_sync_at = ?4 \
                 WHERE id = ?5",
                params![st.uidvalidity, st.uidnext, st.highest_modseq.map(|v| v as i64), st.last_sync_at, id],
            )?;
            Ok(())
        })
    }

    /// Dossiers non masqués : (id, chemin IMAP, rôle), boîte de réception en premier.
    pub fn syncable_folders(&self, account: AccountId) -> Result<Vec<(FolderId, String, FolderRole)>> {
        let mut out = self.with(|c| {
            let mut st =
                c.prepare("SELECT id, imap_path, role FROM folders WHERE account_id = ?1 AND hidden = 0")?;
            let rows = st
                .query_map([account], |r| Ok((r.get(0)?, r.get(1)?, dec(&r.get::<_, String>(2)?)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })?;
        out.sort_by_key(|(_, _, role)| *role);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FolderRole;
    use crate::store::accounts::tests::new_account;
    use crate::store::Store;

    fn fi(path: &str, name: &str, role: FolderRole, hidden: bool) -> FolderInfo {
        FolderInfo { imap_path: path.into(), display_name: name.into(), role, hidden }
    }

    #[test]
    fn upsert_folders_insert_update_delete() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert_account(&new_account("a@x.fr")).unwrap();
        s.upsert_folders(a.id, &[
            fi("Factures", "Factures", FolderRole::Custom, false),
            fi("INBOX", "Boîte de réception", FolderRole::Inbox, false),
            fi("[Gmail]/Tous les messages", "Tous les messages", FolderRole::Custom, true),
        ])
        .unwrap();
        let l = s.list_folders(a.id).unwrap();
        assert_eq!(l.iter().map(|f| f.imap_path.as_str()).collect::<Vec<_>>(), ["INBOX", "Factures"]);
        let inbox_id = l[0].id;
        s.upsert_folders(a.id, &[
            fi("INBOX", "Boîte de réception", FolderRole::Inbox, false),
            fi("Travaux", "Travaux", FolderRole::Custom, false),
        ])
        .unwrap();
        let l = s.list_folders(a.id).unwrap();
        assert_eq!(l.iter().map(|f| f.imap_path.as_str()).collect::<Vec<_>>(), ["INBOX", "Travaux"]);
        assert_eq!(l[0].id, inbox_id, "existing folder keeps its id");
        assert_eq!(l[0].unread_count, 0);
    }

    #[test]
    fn folders_sorted_by_role_then_name() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert_account(&new_account("a@x.fr")).unwrap();
        s.upsert_folders(a.id, &[
            fi("b", "b", FolderRole::Custom, false),
            fi("Trash", "Corbeille", FolderRole::Trash, false),
            fi("INBOX", "Boîte de réception", FolderRole::Inbox, false),
            fi("A", "A", FolderRole::Custom, false),
            fi("Sent", "Envoyés", FolderRole::Sent, false),
        ])
        .unwrap();
        let names: Vec<_> = s.list_folders(a.id).unwrap().into_iter().map(|f| f.display_name).collect();
        assert_eq!(names, ["Boîte de réception", "Envoyés", "Corbeille", "A", "b"]);
    }

    #[test]
    fn folder_state_roundtrip_and_syncable() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert_account(&new_account("a@x.fr")).unwrap();
        s.upsert_folders(a.id, &[
            fi("INBOX", "Boîte de réception", FolderRole::Inbox, false),
            fi("[Gmail]/Important", "Important", FolderRole::Custom, true),
        ])
        .unwrap();
        let sync = s.syncable_folders(a.id).unwrap();
        assert_eq!(sync.len(), 1);
        let st = FolderSyncState { uidvalidity: Some(7), uidnext: Some(42), highest_modseq: Some(9), last_sync_at: Some(100) };
        s.set_folder_state(sync[0].0, &st).unwrap();
        assert_eq!(s.folder_state(sync[0].0).unwrap(), st);
    }
}
