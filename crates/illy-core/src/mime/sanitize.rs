//! Nettoyage du HTML des mails : aucun script, aucune requête externe tant que
//! les images ne sont pas autorisées.

use base64::Engine;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanitizedHtml {
    pub html: String,
    pub has_remote_images: bool,
}

const EXTRA_TAGS: &[&str] = &[
    "style", "center", "font", "u", "s", "strike", "caption", "col", "colgroup", "tfoot", "section",
    "header", "footer", "main", "article", "nav", "figure", "figcaption", "big", "small", "mark",
];
const GENERIC_ATTRS: &[&str] = &[
    "style", "class", "id", "align", "valign", "width", "height", "bgcolor", "color", "border",
    "cellpadding", "cellspacing", "dir", "lang", "title", "role",
];

fn re(cell: &'static OnceLock<regex::Regex>, pat: &str) -> &'static regex::Regex {
    cell.get_or_init(|| regex::Regex::new(pat).unwrap())
}

/// Remplace toute URL distante dans du CSS par `url()` et retire les `@import`.
fn neutralize_css(css: &str) -> String {
    static URL: OnceLock<regex::Regex> = OnceLock::new();
    static IMPORT: OnceLock<regex::Regex> = OnceLock::new();
    let no_import = re(&IMPORT, r#"(?i)@import\s+[^;]*;?"#).replace_all(css, "");
    re(&URL, r#"(?i)url\(\s*(['"]?)\s*(https?:)?//[^)]*\)"#).replace_all(&no_import, "url()").into_owned()
}

pub fn sanitize_html(html: &str, inline: &HashMap<String, (String, Vec<u8>)>) -> SanitizedHtml {
    let mut builder = ammonia::Builder::default();
    builder
        .rm_clean_content_tags(["style"])
        .add_clean_content_tags(["title", "head"])
        .add_tags(EXTRA_TAGS)
        .add_generic_attributes(GENERIC_ATTRS)
        .add_tag_attributes("img", ["srcset"])
        .add_tag_attributes("font", ["face", "size"])
        .add_tag_attributes("td", ["colspan", "rowspan", "nowrap"])
        .add_tag_attributes("th", ["colspan", "rowspan", "nowrap"])
        .add_tag_attributes("a", ["target"])
        .url_schemes(HashSet::from(["http", "https", "mailto", "cid", "data", "tel"]))
        .link_rel(Some("noopener noreferrer"))
        .set_tag_attribute_value("a", "target", "_blank");
    let inline_owned = inline.clone();
    builder.attribute_filter(move |element, attribute, value| -> Option<Cow<'_, str>> {
        let lower = value.trim().to_ascii_lowercase();
        match (element, attribute) {
            (_, "style") => Some(Cow::Owned(neutralize_css(value))),
            ("img", "src") if lower.starts_with("cid:") => {
                let id = value.trim()[4..].trim_start_matches('<').trim_end_matches('>');
                match inline_owned.get(id) {
                    Some((mime, data)) => Some(Cow::Owned(format!(
                        "data:{mime};base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(data)
                    ))),
                    None => None,
                }
            }
            ("img", "src") if lower.starts_with("data:") => {
                if lower.starts_with("data:image/") { Some(Cow::Borrowed(value)) } else { None }
            }
            (_, "href") if lower.starts_with("data:") || lower.starts_with("cid:") => None,
            _ => Some(Cow::Borrowed(value)),
        }
    });
    let cleaned = builder.clean(html).to_string();

    // Feuilles <style> : neutraliser les URL externes (le contenu est du texte pour ammonia).
    static STYLE_BLOCK: OnceLock<regex::Regex> = OnceLock::new();
    let styled = re(&STYLE_BLOCK, r"(?is)(<style[^>]*>)(.*?)(</style>)").replace_all(&cleaned, |c: &regex::Captures| {
        format!("{}{}{}", &c[1], neutralize_css(&c[2]), &c[3])
    });

    // Images distantes : src/srcset → data-blocked-*.
    static IMG: OnceLock<regex::Regex> = OnceLock::new();
    static REMOTE_ATTR: OnceLock<regex::Regex> = OnceLock::new();
    let remote_attr = re(&REMOTE_ATTR, r#"(?i)\s(src|srcset)="(\s*(?:https?:)?//[^"]*)""#);
    let mut has_remote = false;
    let blocked = re(&IMG, r"(?is)<img\b[^>]*>").replace_all(&styled, |c: &regex::Captures| {
        let tag = &c[0];
        if remote_attr.is_match(tag) {
            has_remote = true;
            remote_attr.replace_all(tag, r#" data-blocked-$1="$2""#).into_owned()
        } else {
            tag.to_string()
        }
    });
    SanitizedHtml { html: blocked.into_owned(), has_remote_images: has_remote }
}

/// Réactive les images distantes d'un HTML déjà nettoyé.
pub fn unblock_images(html: &str) -> String {
    html.replace(" data-blocked-srcset=\"", " srcset=\"").replace(" data-blocked-src=\"", " src=\"")
}

/// Convertit un mail texte brut en HTML sûr : échappé, retours à la ligne conservés, liens cliquables.
pub fn text_to_html(text: &str) -> String {
    static URL: OnceLock<regex::Regex> = OnceLock::new();
    let escaped = html_escape::encode_text(text);
    let linked = re(&URL, r#"https?://[^\s<>"']+"#).replace_all(&escaped, |c: &regex::Captures| {
        format!(r#"<a href="{0}" target="_blank" rel="noopener noreferrer">{0}</a>"#, &c[0])
    });
    let body = linked.replace("\r\n", "\n").replace('\n', "<br>");
    format!(r#"<div style="white-space:pre-wrap;font-family:inherit">{body}</div>"#)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn clean(html: &str) -> SanitizedHtml {
        sanitize_html(html, &HashMap::new())
    }

    #[test]
    fn removes_scripts_and_handlers() {
        let out = clean(r#"<p onclick="x()">a</p><script>alert(1)</script><img src="x" onerror="y()">"#).html;
        assert!(!out.contains("script"), "{out}");
        assert!(!out.contains("onclick"), "{out}");
        assert!(!out.contains("onerror"), "{out}");
        assert!(!out.contains("alert"), "{out}");
    }

    #[test]
    fn removes_javascript_links() {
        let out = clean(r#"<a href="javascript:alert(1)">x</a><a href="JaVaScRiPt:alert(2)">y</a>"#).html;
        assert!(!out.to_lowercase().contains("javascript:"), "{out}");
    }

    #[test]
    fn removes_meta_refresh_base_link_iframe_form() {
        let out = clean(
            r#"<meta http-equiv="refresh" content="0;url=https://evil"><base href="https://evil/"><link rel="stylesheet" href="https://t.example/x.css"><iframe src="https://evil"></iframe><form action="https://evil"><input name="p"></form><p>ok</p>"#,
        )
        .html;
        for bad in ["meta", "base", "link", "iframe", "form", "input", "evil", "x.css"] {
            assert!(!out.contains(bad), "{bad} in {out}");
        }
        assert!(out.contains("<p>ok</p>"));
    }

    #[test]
    fn blocks_remote_images() {
        let r = clean(r#"<img src="https://t.example/p.gif" srcset="https://t.example/a.png 2x">"#);
        assert!(r.html.contains(r#"data-blocked-src="https://t.example/p.gif""#), "{}", r.html);
        assert!(r.html.contains("data-blocked-srcset="), "{}", r.html);
        assert!(!r.html.contains(r#" src="https://"#), "{}", r.html);
        assert!(r.has_remote_images);
    }

    #[test]
    fn neutralizes_css_urls() {
        let out = clean(
            r#"<div style="background:url(https://t.example/b.png)">a</div><style>.a{background:url('https://t.example/c.png')} @import url(https://t.example/i.css);</style><table background="https://t.example/t.png"><tr><td>x</td></tr></table>"#,
        )
        .html;
        assert!(!out.contains("t.example"), "{out}");
    }

    #[test]
    fn keeps_newsletter_styling() {
        let out = clean(r##"<style>.btn{color:#fff}</style><table width="600" bgcolor="#eee"><tr><td style="padding:8px">x</td></tr></table>"##).html;
        assert!(out.contains(".btn{color:#fff}"), "{out}");
        assert!(out.contains(r#"style="padding:8px""#), "{out}");
        assert!(out.contains(r#"width="600""#), "{out}");
    }

    #[test]
    fn inlines_cid_images() {
        let mut inline = HashMap::new();
        inline.insert("logo@x".to_string(), ("image/png".to_string(), vec![1u8, 2, 3]));
        let r = sanitize_html(r#"<img src="cid:logo@x">"#, &inline);
        assert!(r.html.contains(r#"src="data:image/png;base64,AQID""#), "{}", r.html);
        assert!(!r.has_remote_images);
    }

    #[test]
    fn links_open_in_new_context() {
        let out = clean(r#"<a href="https://ex.fr/x">x</a>"#).html;
        assert!(out.contains(r#"href="https://ex.fr/x""#), "{out}");
        assert!(out.contains(r#"target="_blank""#), "{out}");
        assert!(out.contains("noopener"), "{out}");
    }

    #[test]
    fn unblock_restores_src() {
        let r = clean(r#"<img src="https://t.example/p.gif">"#);
        let un = unblock_images(&r.html);
        assert!(un.contains(r#"src="https://t.example/p.gif""#), "{un}");
        assert!(!un.contains("data-blocked"), "{un}");
    }

    #[test]
    fn text_to_html_links_and_escapes() {
        let out = text_to_html("a<b>\nvoir https://ex.fr/x");
        assert!(out.contains("a&lt;b&gt;<br>"), "{out}");
        assert!(out.contains(r#"<a href="https://ex.fr/x""#), "{out}");
    }
}
