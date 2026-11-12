//! Carnet d'adresses alimenté par la synchro (autocomplétion des destinataires).

use super::Store;
use crate::error::Result;
use crate::model::Address;
use rusqlite::{params, Connection};
use std::collections::HashSet;

/// Minuscules sans accents (comparaison des recherches).
pub(crate) fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => out.push('a'),
            'ç' => out.push('c'),
            'é' | 'è' | 'ê' | 'ë' => out.push('e'),
            'î' | 'ï' | 'í' | 'ì' => out.push('i'),
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' | 'ø' => out.push('o'),
            'ù' | 'û' | 'ü' | 'ú' => out.push('u'),
            'ÿ' | 'ý' => out.push('y'),
            'ñ' => out.push('n'),
            'œ' => out.push_str("oe"),
            'æ' => out.push_str("ae"),
            c => out.push(c),
        }
    }
    out
}

fn search_text(a: &Address) -> String {
    // Espace en tête : la recherche « % mot% » trouve le début de l'adresse et de chaque mot du nom.
    format!(" {} {}", fold(&a.email), fold(a.name.as_deref().unwrap_or("")))
}

pub(crate) fn record_contacts_in(c: &Connection, addrs: &[Address], date: i64, own: &HashSet<String>) -> rusqlite::Result<()> {
    let mut st = c.prepare(
        "INSERT INTO contacts (email, name, last_seen_at, count, search_text) VALUES (?1, ?2, ?3, 1, ?4) \
         ON CONFLICT(email) DO UPDATE SET count = count + 1, \
           last_seen_at = MAX(COALESCE(last_seen_at, 0), excluded.last_seen_at), \
           name = COALESCE(NULLIF(excluded.name, ''), name), \
           search_text = CASE WHEN COALESCE(excluded.name, '') = '' THEN search_text ELSE excluded.search_text END",
    )?;
    for a in addrs {
        let email = a.email.trim().to_lowercase();
        if email.is_empty() || !email.contains('@') || own.contains(&email) {
            continue;
        }
        let addr = Address { name: a.name.clone().filter(|n| !n.trim().is_empty()), email: email.clone() };
        st.execute(params![email, addr.name, date, search_text(&addr)])?;
    }
    Ok(())
}

pub(crate) fn own_addresses_in(c: &Connection) -> rusqlite::Result<HashSet<String>> {
    let mut st = c.prepare("SELECT lower(email) FROM accounts")?;
    let v = st.query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<HashSet<_>>>()?;
    Ok(v)
}

impl Store {
    pub fn record_contacts(&self, addrs: &[Address], date: i64, own: &HashSet<String>) -> Result<()> {
        self.with(|c| Ok(record_contacts_in(c, addrs, date, own)?))
    }

    pub fn own_addresses(&self) -> Result<HashSet<String>> {
        self.with_read(|c| Ok(own_addresses_in(c)?))
    }

    pub fn search_contacts(&self, query: &str, limit: u32) -> Result<Vec<Address>> {
        let q = fold(query.trim());
        if q.is_empty() {
            return Ok(vec![]);
        }
        let escaped = q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
        self.with_read(|c| {
            let mut st = c.prepare(
                "SELECT email, name FROM contacts WHERE search_text LIKE ?1 ESCAPE '\\' \
                 ORDER BY count DESC, last_seen_at DESC LIMIT ?2",
            )?;
            let v = st
                .query_map(params![format!("% {escaped}%"), limit], |r| Ok(Address { email: r.get(0)?, name: r.get(1)? }))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(v)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::model::Address;
    use crate::store::Store;
    use std::collections::HashSet;

    fn a(name: &str, email: &str) -> Address {
        Address { name: (!name.is_empty()).then(|| name.to_string()), email: email.into() }
    }

    #[test]
    fn contacts_counted_and_sorted() {
        let s = Store::open_in_memory().unwrap();
        let own = HashSet::new();
        s.record_contacts(&[a("Marie Dupont", "marie@ex.fr")], 10, &own).unwrap();
        s.record_contacts(&[a("", "marie@ex.fr"), a("Paul Martin", "paul@ex.fr")], 20, &own).unwrap();
        s.record_contacts(&[a("Max", "max@ex.fr")], 5, &own).unwrap();
        let r = s.search_contacts("m", 8).unwrap();
        assert_eq!(r[0].email, "marie@ex.fr");
        assert_eq!(r[0].name.as_deref(), Some("Marie Dupont"), "an empty name must not erase a known one");
        assert!(r.iter().any(|x| x.email == "paul@ex.fr"), "matches the word 'Martin'");
        assert_eq!(s.search_contacts("", 8).unwrap().len(), 0);
    }

    #[test]
    fn search_ignores_accents_and_case() {
        let s = Store::open_in_memory().unwrap();
        s.record_contacts(&[a("Élodie Martin", "e@x.fr")], 1, &HashSet::new()).unwrap();
        for q in ["elo", "ELO", "Élo", "mart", "e@"] {
            assert_eq!(s.search_contacts(q, 8).unwrap().len(), 1, "{q}");
        }
        assert!(s.search_contacts("odie", 8).unwrap().is_empty(), "word prefix only");
        assert!(s.search_contacts("%", 8).unwrap().is_empty(), "LIKE wildcards are escaped");
    }

    #[test]
    fn own_addresses_excluded() {
        let s = Store::open_in_memory().unwrap();
        let own: HashSet<String> = ["moi@gmail.com".to_string()].into();
        s.record_contacts(&[a("Moi", "Moi@GMAIL.com"), a("", "")], 1, &own).unwrap();
        assert!(s.search_contacts("moi", 8).unwrap().is_empty());
    }
}
