//! Analyse MIME : en-têtes, structure IMAP, décodage des parties, nettoyage HTML.

mod decode;
mod headers;
mod structure;

pub use decode::{decode_part, html_to_text, snippet_from, Decoded};
pub use headers::{parse_header_block, ParsedHeaders};
pub use structure::{flatten_structure, PartInfo, PartKind};
