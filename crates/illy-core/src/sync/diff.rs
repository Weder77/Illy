//! Calculs purs de synchro.

use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UidDiff {
    /// UID présents sur le serveur et inconnus localement, du plus récent au plus ancien.
    pub new_desc: Vec<u32>,
    /// UID connus localement et disparus du serveur.
    pub vanished: Vec<u32>,
}

pub fn diff_uids(server: &[u32], known: &[u32]) -> UidDiff {
    let s: HashSet<u32> = server.iter().copied().collect();
    let k: HashSet<u32> = known.iter().copied().collect();
    let mut new_desc: Vec<u32> = s.difference(&k).copied().collect();
    new_desc.sort_unstable_by(|a, b| b.cmp(a));
    let mut vanished: Vec<u32> = k.difference(&s).copied().collect();
    vanished.sort_unstable();
    UidDiff { new_desc, vanished }
}

pub fn uidvalidity_changed(stored: Option<u32>, server: u32) -> bool {
    matches!(stored, Some(v) if v != server)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_new_and_vanished() {
        let d = diff_uids(&[1, 2, 5, 6], &[1, 2, 3]);
        assert_eq!(d.new_desc, vec![6, 5]);
        assert_eq!(d.vanished, vec![3]);
    }

    #[test]
    fn diff_empty() {
        let d = diff_uids(&[], &[]);
        assert!(d.new_desc.is_empty() && d.vanished.is_empty());
        let d = diff_uids(&[], &[4]);
        assert_eq!(d.vanished, vec![4]);
    }

    #[test]
    fn uidvalidity_none_is_not_change() {
        assert!(!uidvalidity_changed(None, 5));
        assert!(!uidvalidity_changed(Some(5), 5));
        assert!(uidvalidity_changed(Some(5), 6));
    }
}
