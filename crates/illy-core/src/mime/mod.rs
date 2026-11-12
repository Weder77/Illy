//! Analyse MIME : en-têtes, structure IMAP, décodage des parties, nettoyage HTML.

mod decode;
mod full;
mod headers;
mod sanitize;
mod structure;

pub use decode::{decode_part, html_to_text, snippet_from, synth_mime_header, Decoded};
pub use full::{extract_full, FullMessage};
pub use headers::{parse_header_block, ParsedHeaders};
pub use sanitize::{sanitize_html, text_to_html, unblock_images, SanitizedHtml};
pub use structure::{flatten_structure, PartInfo, PartKind};
