//! Cœur d'Illy : stockage local, IMAP, synchro, analyse MIME.

pub mod core;
pub mod error;
pub mod events;
pub mod imap;
pub mod mime;
pub mod model;
pub mod providers;
pub mod secrets;
pub mod smtp;
pub mod store;
pub mod sync;

pub use crate::core::Core;
pub use error::{CoreError, Result};
