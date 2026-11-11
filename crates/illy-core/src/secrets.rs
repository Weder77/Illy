//! Stockage des mots de passe : Gestionnaire d'identification Windows en production.

use crate::error::{CoreError, Result};
use crate::model::AccountId;
use std::collections::HashMap;
use std::sync::Mutex;

pub trait SecretStore: Send + Sync + 'static {
    fn set(&self, account: AccountId, password: &str) -> Result<()>;
    fn get(&self, account: AccountId) -> Result<Option<String>>;
    fn delete(&self, account: AccountId) -> Result<()>;
}

#[derive(Default)]
pub struct MemorySecretStore(Mutex<HashMap<AccountId, String>>);

impl SecretStore for MemorySecretStore {
    fn set(&self, account: AccountId, password: &str) -> Result<()> {
        self.0.lock().unwrap().insert(account, password.to_string());
        Ok(())
    }
    fn get(&self, account: AccountId) -> Result<Option<String>> {
        Ok(self.0.lock().unwrap().get(&account).cloned())
    }
    fn delete(&self, account: AccountId) -> Result<()> {
        self.0.lock().unwrap().remove(&account);
        Ok(())
    }
}

#[allow(dead_code)]
fn secret_err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Secret(e.to_string())
}

#[cfg(windows)]
pub use windows_store::KeyringSecretStore;

#[cfg(windows)]
mod windows_store {
    use super::*;

    const SERVICE: &str = "Illy";

    /// Mots de passe dans le Gestionnaire d'identification Windows.
    pub struct KeyringSecretStore;

    impl KeyringSecretStore {
        pub fn new() -> Result<Self> {
            let store = windows_native_keyring_store::Store::new().map_err(secret_err)?;
            keyring_core::set_default_store(store);
            Ok(KeyringSecretStore)
        }
        fn entry(account: AccountId) -> Result<keyring_core::Entry> {
            keyring_core::Entry::new(SERVICE, &format!("account:{account}")).map_err(secret_err)
        }
    }

    impl SecretStore for KeyringSecretStore {
        fn set(&self, account: AccountId, password: &str) -> Result<()> {
            Self::entry(account)?.set_password(password).map_err(secret_err)
        }
        fn get(&self, account: AccountId) -> Result<Option<String>> {
            match Self::entry(account)?.get_password() {
                Ok(p) => Ok(Some(p)),
                Err(keyring_core::Error::NoEntry) => Ok(None),
                Err(e) => Err(secret_err(e)),
            }
        }
        fn delete(&self, account: AccountId) -> Result<()> {
            match Self::entry(account)?.delete_credential() {
                Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
                Err(e) => Err(secret_err(e)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_secret_roundtrip() {
        let s = MemorySecretStore::default();
        s.set(1, "pw").unwrap();
        assert_eq!(s.get(1).unwrap(), Some("pw".to_string()));
        s.delete(1).unwrap();
        assert_eq!(s.get(1).unwrap(), None);
    }
}
