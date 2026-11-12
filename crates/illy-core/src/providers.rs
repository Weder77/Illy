//! Profils de fournisseurs (Gmail, Yahoo, OVH) et petites règles associées.

use crate::model::{Provider, ProviderGuess, ServerConfig, TlsMode};

const COLORS: [&str; 6] = ["#8B5CF6", "#14B8A6", "#EC4899", "#F59E0B", "#22C55E", "#38BDF8"];
/// Extensions qui exécutent du code sous Windows (ou un script dans le navigateur).
const DANGEROUS_EXT: &[&str] = &[
    "exe", "com", "pif", "scr", "bat", "cmd", "msi", "msp", "msc", "cpl", "dll", "sys", "inf", "reg", "lnk",
    "url", "scf", "hta", "chm", "jar", "js", "jse", "vbs", "vbe", "wsf", "wsh", "ws", "ps1", "psm1", "psd1",
    "appref-ms", "application", "gadget", "settingcontent-ms", "iso", "img", "vhd", "vhdx", "htm", "html",
    "xhtml", "mht", "mhtml", "svg", "xml",
];

/// Caractères invisibles de mise en forme (dont les inversions bidirectionnelles qui maquillent une extension).
pub fn is_format_char(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2069}' | '\u{FEFF}' | '\u{061C}')
}

/// Nom de fichier tel qu'il sera écrit sur le disque : sans chemin, sans caractère interdit
/// par Windows (dont `:`, flux NTFS), sans caractère invisible, sans point ni espace final.
pub fn safe_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !is_format_char(*c))
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim_end_matches(['.', ' ']);
    let mut out: String = trimmed.chars().take(120).collect();
    out = out.trim_end_matches(['.', ' ']).to_string();
    if out.is_empty() {
        out = "piece-jointe".into();
    }
    out
}

fn tls(host: &str, port: u16) -> ServerConfig {
    ServerConfig { host: host.to_string(), port, tls: TlsMode::Implicit }
}

pub fn detect_provider(email: &str) -> ProviderGuess {
    let domain = email.rsplit('@').next().unwrap_or("").trim().to_ascii_lowercase();
    let (provider, imap, smtp) = match domain.as_str() {
        "gmail.com" | "googlemail.com" => {
            (Provider::Gmail, tls("imap.gmail.com", 993), tls("smtp.gmail.com", 465))
        }
        "ymail.com" => (Provider::Yahoo, tls("imap.mail.yahoo.com", 993), tls("smtp.mail.yahoo.com", 465)),
        d if d.starts_with("yahoo.") => {
            (Provider::Yahoo, tls("imap.mail.yahoo.com", 993), tls("smtp.mail.yahoo.com", 465))
        }
        _ => return ProviderGuess { provider: None, imap: None, smtp: None },
    };
    ProviderGuess { provider: Some(provider), imap: Some(imap), smtp: Some(smtp) }
}

pub fn ovh_preset() -> (ServerConfig, ServerConfig) {
    (tls("ssl0.ovh.net", 993), tls("ssl0.ovh.net", 465))
}

pub fn color_for(sort_order: i64) -> &'static str {
    COLORS[sort_order.rem_euclid(COLORS.len() as i64) as usize]
}

pub fn is_dangerous_filename(name: &str) -> bool {
    match safe_filename(name).rsplit_once('.') {
        Some((_, ext)) => DANGEROUS_EXT.contains(&ext.trim().to_ascii_lowercase().as_str()),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Provider, ServerConfig, TlsMode};

    fn sc(host: &str, port: u16) -> ServerConfig {
        ServerConfig { host: host.into(), port, tls: TlsMode::Implicit }
    }

    #[test]
    fn detect_gmail() {
        for e in ["Moi@GMail.com", "x@googlemail.com"] {
            let g = detect_provider(e);
            assert_eq!(g.provider, Some(Provider::Gmail));
            assert_eq!(g.imap, Some(sc("imap.gmail.com", 993)));
            assert_eq!(g.smtp, Some(sc("smtp.gmail.com", 465)));
        }
    }

    #[test]
    fn detect_yahoo() {
        for e in ["a@yahoo.fr", "a@yahoo.com", "a@ymail.com"] {
            let g = detect_provider(e);
            assert_eq!(g.provider, Some(Provider::Yahoo));
            assert_eq!(g.imap, Some(sc("imap.mail.yahoo.com", 993)));
            assert_eq!(g.smtp, Some(sc("smtp.mail.yahoo.com", 465)));
        }
    }

    #[test]
    fn detect_unknown() {
        let g = detect_provider("contact@mondomaine.fr");
        assert_eq!(g, crate::model::ProviderGuess { provider: None, imap: None, smtp: None });
    }

    #[test]
    fn ovh_preset_values() {
        let (i, s) = ovh_preset();
        assert_eq!(i, sc("ssl0.ovh.net", 993));
        assert_eq!(s, sc("ssl0.ovh.net", 465));
    }

    #[test]
    fn colors_cycle() {
        assert_eq!(color_for(0), "#8B5CF6");
        assert_eq!(color_for(6), "#8B5CF6");
        assert_eq!(color_for(1), "#14B8A6");
        assert!((0..6).all(|i| color_for(i) != "#2F7BFF"), "no account dot may use the unread blue");
    }

    #[test]
    fn dangerous_names() {
        for n in ["a.EXE", "x.bat", "y.cmd", "z.ps1", "w.vbs", "v.js", "u.msi", "t.scr"] {
            assert!(is_dangerous_filename(n), "{n}");
        }
        for n in ["devis.pdf", "photo.jpg", "noext", "rapport.docx", "tableau.xlsx", "archive.zip"] {
            assert!(!is_dangerous_filename(n), "{n}");
        }
    }

    #[test]
    fn dangerous_names_cannot_hide_their_extension() {
        for n in [
            "facture.exe.",
            "facture.exe ",
            "facture.exe. . ",
            "photo\u{202E}gpj.exe",
            "x.hta",
            "y.lnk",
            "z.com",
            "w.pif",
            "page.html",
            "image.svg",
            "cle.reg",
            "app.jar",
            "lien.url",
            "panneau.cpl",
            "script.wsf",
        ] {
            assert!(is_dangerous_filename(n), "{n:?}");
        }
    }
}
