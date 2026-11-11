//! Cœur d'Illy : stockage local, IMAP, synchro, analyse MIME.

pub mod error;
pub mod events;
pub mod model;
pub mod providers;
pub mod secrets;

pub use error::{CoreError, Result};
