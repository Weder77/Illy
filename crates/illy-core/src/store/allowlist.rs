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

    pub fn images_allowed_for(&self, sender: &str) -> Result<bool> {
        self.with(|c| {
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
}
