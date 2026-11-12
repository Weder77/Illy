use super::Store;
use crate::error::Result;

impl Store {
    pub fn allow_images_from(&self, sender: &str) -> Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT OR IGNORE INTO image_allowlist (sender_email) VALUES (?1)",
                [sender.trim().to_lowercase()],
            )?;
            Ok(())
        })
    }

    /// Expéditeurs dont les images s'affichent toujours, triés.
    pub fn list_image_allowlist(&self) -> Result<Vec<String>> {
        self.with_read(|c| {
            let mut st = c.prepare("SELECT sender_email FROM image_allowlist ORDER BY sender_email")?;
            let rows = st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
            Ok(rows)
        })
    }

    pub fn remove_from_image_allowlist(&self, sender: &str) -> Result<()> {
        self.with(|c| {
            c.execute("DELETE FROM image_allowlist WHERE sender_email = ?1", [sender.trim().to_lowercase()])?;
            Ok(())
        })
    }

    pub fn images_allowed_for(&self, sender: &str) -> Result<bool> {
        self.with_read(|c| {
            let n: i64 = c.query_row(
                "SELECT COUNT(*) FROM image_allowlist WHERE sender_email = ?1",
                [sender.trim().to_lowercase()],
                |r| r.get(0),
            )?;
            Ok(n > 0)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::store::Store;

    #[test]
    fn allowlist_case_insensitive() {
        let s = Store::open_in_memory().unwrap();
        assert!(!s.images_allowed_for("marie@ex.fr").unwrap());
        s.allow_images_from("Marie@Ex.fr").unwrap();
        assert!(s.images_allowed_for("marie@ex.fr").unwrap());
        s.allow_images_from("marie@ex.fr").unwrap();
    }

    #[test]
    fn allowlist_list_and_remove() {
        let s = Store::open_in_memory().unwrap();
        s.allow_images_from("Zoe@Ex.fr").unwrap();
        s.allow_images_from("anna@ex.fr").unwrap();
        assert_eq!(s.list_image_allowlist().unwrap(), ["anna@ex.fr", "zoe@ex.fr"]);
        s.remove_from_image_allowlist("ZOE@ex.fr ").unwrap();
        assert_eq!(s.list_image_allowlist().unwrap(), ["anna@ex.fr"]);
        assert!(!s.images_allowed_for("zoe@ex.fr").unwrap());
    }
}
