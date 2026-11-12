//! Cœur d'Illy : stockage local, IMAP, synchro, analyse MIME.

pub mod error;
pub mod events;
pub mod imap;
pub mod mime;
pub mod model;
pub mod providers;
pub mod secrets;
pub mod store;
pub mod sync;

pub use error::{CoreError, Result};
