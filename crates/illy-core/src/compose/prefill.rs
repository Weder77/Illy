//! Préremplissage des réponses et transferts.

use crate::model::{Address, DraftKind};
use crate::store::ReplySource;
use chrono::{Datelike, Local, TimeZone, Timelike};
use std::collections::HashSet;

const DAYS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];
const MONTHS: [&str; 12] = [
    "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre",
];

/// « jeudi 12 novembre 2026 à 14:32 » (heure locale).
pub fn french_datetime(ts: i64) -> String {
    let Some(d) = Local.timestamp_opt(ts, 0).single() else { return String::new() };
    format!(
        "{} {} {} {} à {:02}:{:02}",
        DAYS[d.weekday().num_days_from_monday() as usize],
        d.day(),
        MONTHS[d.month0() as usize],
        d.year(),
        d.hour(),
        d.minute()
    )
}

pub fn subject_with_prefix(prefix: &str, subject: &str) -> String {
    let s = subject.trim();
    let lower = s.to_lowercase();
    if ["re:", "fwd:", "fw:", "tr:"].iter().any(|p| lower.starts_with(p)) {
        s.to_string()
    } else {
        format!("{prefix}{s}")
    }
}

fn key(a: &Address) -> String {
    a.email.trim().to_lowercase()
}

/// Destinataires d'une réponse : (À, Cc).
pub fn reply_recipients(src: &ReplySource, all: bool, own: &HashSet<String>) -> (Vec<Address>, Vec<Address>) {
    let to: Vec<Address> = if src.reply_to.is_empty() { vec![src.from.clone()] } else { src.reply_to.clone() };
    if !all {
        return (to, vec![]);
    }
    let mut seen: HashSet<String> = to.iter().map(key).collect();
    seen.extend(own.iter().cloned());
    let mut cc = Vec::new();
    for a in src.to.iter().chain(src.cc.iter()) {
        let k = key(a);
        if k.is_empty() || !seen.insert(k) {
            continue;
        }
        cc.push(a.clone());
    }
    (to, cc)
}

fn esc(s: &str) -> String {
    html_escape::encode_text(s).into_owned()
}

fn display(a: &Address) -> String {
    match a.name.as_deref().filter(|n| !n.trim().is_empty()) {
        Some(n) => format!("{n} <{}>", a.email),
        None => a.email.clone(),
    }
}

/// Bloc de signature remplaçable quand on change de compte « De ».
pub fn signature_block(signature_html: &str) -> String {
    let has_content = !crate::mime::html_to_text(signature_html).trim().is_empty() || signature_html.contains("<img");
    if has_content {
        format!("<div data-illy-signature>{signature_html}</div>")
    } else {
        String::new()
    }
}

pub fn forward_header(src: &ReplySource) -> String {
    let to = src.to.iter().map(display).collect::<Vec<_>>().join(", ");
    format!(
        "<p>---------- Message transféré ----------<br>De : {}<br>Date : {}<br>Objet : {}<br>À : {}</p>",
        esc(&display(&src.from)),
        esc(&french_datetime(src.date)),
        esc(&src.subject),
        esc(&to)
    )
}

/// Corps initial : ligne vide, signature, puis citation (réponse) ou message transféré.
pub fn quoted_body(kind: DraftKind, src: &ReplySource, original_html: &str, signature_html: &str) -> String {
    let head = format!("<p></p>{}", signature_block(signature_html));
    match kind {
        DraftKind::New => head,
        DraftKind::Reply | DraftKind::ReplyAll => {
            let who = src.from.name.as_deref().filter(|n| !n.trim().is_empty()).unwrap_or(&src.from.email);
            format!(
                "{head}<div class=\"illy-quote\"><p>Le {}, {} a écrit :</p><blockquote>{original_html}</blockquote></div>",
                esc(&french_datetime(src.date)),
                esc(who)
            )
        }
        DraftKind::Forward => format!("{head}<div class=\"illy-forward\">{}{original_html}</div>", forward_header(src)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Address, DraftKind};
    use crate::store::ReplySource;
    use chrono::TimeZone;
    use std::collections::HashSet;

    fn a(name: &str, email: &str) -> Address {
        Address { name: (!name.is_empty()).then(|| name.to_string()), email: email.into() }
    }

    fn src() -> ReplySource {
        ReplySource {
            from: a("Marie Dupont", "marie@ex.fr"),
            to: vec![a("Moi", "MOI@gmail.com"), a("Paul", "paul@ex.fr")],
            cc: vec![a("", "paul@ex.fr"), a("Zoé", "zoe@ex.fr")],
            reply_to: vec![],
            subject: "Devis cuisine".into(),
            date: chrono::Local.with_ymd_and_hms(2026, 11, 12, 14, 32, 0).unwrap().timestamp(),
            message_id: Some("<m1@ex.fr>".into()),
            references: vec![],
            account_id: 1,
        }
    }

    #[test]
    fn prefix_rules() {
        assert_eq!(subject_with_prefix("Re: ", "Devis"), "Re: Devis");
        assert_eq!(subject_with_prefix("Re: ", "RE: Devis"), "RE: Devis");
        assert_eq!(subject_with_prefix("Fwd: ", "tr: x"), "tr: x");
        assert_eq!(subject_with_prefix("Fwd: ", "Fw: x"), "Fw: x");
        assert_eq!(subject_with_prefix("Re: ", "  "), "Re: ");
    }

    #[test]
    fn reply_uses_reply_to() {
        let own = HashSet::new();
        let mut s = src();
        assert_eq!(reply_recipients(&s, false, &own).0, vec![a("Marie Dupont", "marie@ex.fr")]);
        s.reply_to = vec![a("Liste", "liste@ex.fr")];
        let (to, cc) = reply_recipients(&s, false, &own);
        assert_eq!(to, vec![a("Liste", "liste@ex.fr")]);
        assert!(cc.is_empty());
    }

    #[test]
    fn reply_all_excludes_self_and_duplicates() {
        let own: HashSet<String> = ["moi@gmail.com".to_string()].into();
        let (to, cc) = reply_recipients(&src(), true, &own);
        assert_eq!(to, vec![a("Marie Dupont", "marie@ex.fr")]);
        let emails: Vec<&str> = cc.iter().map(|x| x.email.as_str()).collect();
        assert_eq!(emails, vec!["paul@ex.fr", "zoe@ex.fr"]);
    }

    #[test]
    fn quote_contains_header_and_blockquote() {
        let body = quoted_body(DraftKind::Reply, &src(), "<p>Bonjour</p>", "<p>Moi</p>");
        assert!(body.contains("Marie Dupont a écrit :"), "{body}");
        assert!(body.contains("<blockquote"), "{body}");
        assert!(body.contains("<p>Bonjour</p>"));
        let sig = body.find("data-illy-signature").unwrap();
        assert!(sig < body.find("<blockquote").unwrap(), "signature above the quote");
        let no_sig = quoted_body(DraftKind::Reply, &src(), "<p>Bonjour</p>", "  <p> </p> ");
        assert!(!no_sig.contains("data-illy-signature"));
        let new = quoted_body(DraftKind::New, &src(), "<p>x</p>", "");
        assert!(!new.contains("<blockquote") && !new.contains("<p>x</p>"));
    }

    #[test]
    fn quote_escapes_sender_name() {
        let mut s = src();
        s.from = a("<script>x</script>", "x@ex.fr");
        assert!(!quoted_body(DraftKind::Reply, &s, "", "").contains("<script>"));
    }

    #[test]
    fn forward_header_lists_fields() {
        let h = forward_header(&src());
        for k in ["Message transféré", "De :", "Date :", "Objet :", "À :", "Devis cuisine", "marie@ex.fr"] {
            assert!(h.contains(k), "{k} in {h}");
        }
    }

    #[test]
    fn french_date() {
        let ts = chrono::Local.with_ymd_and_hms(2026, 11, 12, 14, 32, 0).unwrap().timestamp();
        assert_eq!(french_datetime(ts), "jeudi 12 novembre 2026 à 14:32");
    }
}
