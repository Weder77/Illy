use super::{dec, enc, Store};
use crate::error::{CoreError, Result};
use crate::model::*;
use crate::providers::color_for;
use rusqlite::{params, OptionalExtension, Row};

const COLS: &str = "id, email, display_name, provider, imap_host, imap_port, imap_tls, \
                    smtp_host, smtp_port, smtp_tls, color, sort_order, status, last_error";

fn row_to_account(r: &Row) -> rusqlite::Result<Account> {
    Ok(Account {
        id: r.get(0)?,
        email: r.get(1)?,
        display_name: r.get(2)?,
        provider: dec(&r.get::<_, String>(3)?)?,
        imap: ServerConfig { host: r.get(4)?, port: r.get(5)?, tls: dec(&r.get::<_, String>(6)?)? },
        smtp: ServerConfig { host: r.get(7)?, port: r.get(8)?, tls: dec(&r.get::<_, String>(9)?)? },
        color: r.get(10)?,
        sort_order: r.get(11)?,
        status: dec(&r.get::<_, String>(12)?)?,
        last_error: r.get(13)?,
    })
}

impl Store {
    pub fn insert_account(&self, a: &NewAccount) -> Result<Account> {
        let id = self.with(|c| {
            let order: i64 =
                c.query_row("SELECT COALESCE(MAX(sort_order) + 1, 0) FROM accounts", [], |r| r.get(0))?;
            c.execute(
                "INSERT INTO accounts (email, display_name, provider, imap_host, imap_port, imap_tls, \
                 smtp_host, smtp_port, smtp_tls, color, sort_order, status) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'ok')",
                params![
                    a.email,
                    a.display_name,
                    enc(&a.provider),
                    a.imap.host,
                    a.imap.port,
                    enc(&a.imap.tls),
                    a.smtp.host,
                    a.smtp.port,
                    enc(&a.smtp.tls),
                    color_for(order),
                    order
                ],
            )?;
            Ok(c.last_insert_rowid())
        })?;
        self.get_account(id)
    }

    pub fn list_accounts(&self) -> Result<Vec<Account>> {
        self.with_read(|c| {
            let mut st = c.prepare(&format!("SELECT {COLS} FROM accounts ORDER BY sort_order"))?;
            let rows = st.query_map([], row_to_account)?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    pub fn get_account(&self, id: AccountId) -> Result<Account> {
        self.with_read(|c| {
            c.query_row(&format!("SELECT {COLS} FROM accounts WHERE id = ?1"), [id], row_to_account)
                .optional()?
                .ok_or(CoreError::NotFound)
        })
    }

    pub fn set_account_status(&self, id: AccountId, s: AccountStatus, err: Option<&str>) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE accounts SET status = ?1, last_error = ?2 WHERE id = ?3",
                params![enc(&s), err, id],
            )?;
            Ok(())
        })
    }

    pub fn account_signature(&self, id: AccountId) -> Result<String> {
        self.with_read(|c| {
            c.query_row("SELECT signature_html FROM accounts WHERE id = ?1", [id], |r| r.get(0))
                .optional()?
                .ok_or(CoreError::NotFound)
        })
    }

    pub fn delete_account(&self, id: AccountId) -> Result<()> {
        self.with(|c| {
            c.execute("DELETE FROM accounts WHERE id = ?1", [id])?;
            Ok(())
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::model::*;
    use crate::store::Store;

    pub(crate) fn new_account(email: &str) -> NewAccount {
        let sc = ServerConfig { host: "imap.example.org".into(), port: 993, tls: TlsMode::Implicit };
        NewAccount {
            email: email.into(),
            display_name: "Moi".into(),
            provider: Provider::Custom,
            imap: sc.clone(),
            smtp: ServerConfig { port: 465, ..sc },
        }
    }

    #[test]
    fn insert_and_list_accounts() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert_account(&new_account("a@x.fr")).unwrap();
        let b = s.insert_account(&new_account("b@x.fr")).unwrap();
        assert_eq!((a.sort_order, b.sort_order), (0, 1));
        assert_eq!((a.color.as_str(), b.color.as_str()), ("#8B5CF6", "#14B8A6"));
        assert_eq!(a.status, AccountStatus::Ok);
        let all = s.list_accounts().unwrap();
        assert_eq!(all.iter().map(|x| x.email.as_str()).collect::<Vec<_>>(), ["a@x.fr", "b@x.fr"]);
        assert_eq!(all[0].imap.port, 993);
        assert_eq!(all[0].smtp.port, 465);
    }

    #[test]
    fn status_roundtrip() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert_account(&new_account("a@x.fr")).unwrap();
        s.set_account_status(a.id, AccountStatus::AuthError, Some("NO [AUTH]")).unwrap();
        let got = s.get_account(a.id).unwrap();
        assert_eq!(got.status, AccountStatus::AuthError);
        assert_eq!(got.last_error.as_deref(), Some("NO [AUTH]"));
        assert!(matches!(s.get_account(999), Err(crate::CoreError::NotFound)));
    }

    #[test]
    fn delete_account_cascades() {
        let s = Store::open_in_memory().unwrap();
        let a = s.insert_account(&new_account("a@x.fr")).unwrap();
        s.upsert_folders(a.id, &[crate::store::FolderInfo {
            imap_path: "INBOX".into(),
            display_name: "Boîte de réception".into(),
            role: FolderRole::Inbox,
            hidden: false,
        }])
        .unwrap();
        s.delete_account(a.id).unwrap();
        assert!(s.list_folders(a.id).unwrap().is_empty());
        assert!(s.list_accounts().unwrap().is_empty());
    }
}
