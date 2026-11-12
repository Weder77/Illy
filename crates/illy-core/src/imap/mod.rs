//! Client IMAP : connexion, dossiers, récupération des en-têtes et des parties.

mod conn;
mod fetch;
mod folders;

pub use conn::{connect, imap_err, Caps, ImapSession};
pub use fetch::{add_flag, append, delete_uid, fetch_flags, fetch_full, fetch_headers, fetch_part, search_header, search_all_uids, select, store_seen, uid_set, Selected};
pub use folders::{folder_info, list_folders};
