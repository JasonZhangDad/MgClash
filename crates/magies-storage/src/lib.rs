//! Secure credential storage for `MgClash`.

use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Mutex;

use magies_domain::CredentialRef;
use thiserror::Error;
use zeroize::Zeroizing;

const SERVICE_NAME: &str = "com.magies.mgclash";

/// Sensitive binary data that is zeroed when its allocation is dropped.
pub struct SecretValue(Zeroizing<Vec<u8>>);

impl SecretValue {
    /// Creates a non-empty secret value.
    ///
    /// # Errors
    ///
    /// Returns [`SecretStoreError::EmptySecret`] for empty data.
    pub fn new(value: Vec<u8>) -> Result<Self, SecretStoreError> {
        if value.is_empty() {
            return Err(SecretStoreError::EmptySecret);
        }
        Ok(Self(Zeroizing::new(value)))
    }

    #[must_use]
    pub fn expose_secret(&self) -> &[u8] {
        &self.0
    }
}

impl Clone for SecretValue {
    fn clone(&self) -> Self {
        Self(Zeroizing::new(self.0.to_vec()))
    }
}

impl Debug for SecretValue {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SecretValue([REDACTED])")
    }
}

/// A synchronous credential-store contract shared by native and test backends.
pub trait SecretStore: Send + Sync {
    /// Creates or replaces a credential.
    ///
    /// # Errors
    ///
    /// Returns a typed storage error when the backend rejects the operation.
    fn put(
        &self,
        credential_ref: &CredentialRef,
        secret: &SecretValue,
    ) -> Result<(), SecretStoreError>;

    /// Reads a credential.
    ///
    /// # Errors
    ///
    /// Returns [`SecretStoreError::NotFound`] when no credential exists.
    fn get(&self, credential_ref: &CredentialRef) -> Result<SecretValue, SecretStoreError>;

    /// Deletes a credential.
    ///
    /// # Errors
    ///
    /// Returns [`SecretStoreError::NotFound`] when no credential exists.
    fn delete(&self, credential_ref: &CredentialRef) -> Result<(), SecretStoreError>;
}

/// In-memory implementation used by deterministic unit and integration tests.
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    entries: Mutex<HashMap<String, SecretValue>>,
}

impl SecretStore for MemorySecretStore {
    fn put(
        &self,
        credential_ref: &CredentialRef,
        secret: &SecretValue,
    ) -> Result<(), SecretStoreError> {
        self.entries
            .lock()
            .expect("memory secret store mutex cannot be poisoned without a panic")
            .insert(credential_ref.as_str().to_owned(), secret.clone());
        Ok(())
    }

    fn get(&self, credential_ref: &CredentialRef) -> Result<SecretValue, SecretStoreError> {
        self.entries
            .lock()
            .expect("memory secret store mutex cannot be poisoned without a panic")
            .get(credential_ref.as_str())
            .cloned()
            .ok_or(SecretStoreError::NotFound)
    }

    fn delete(&self, credential_ref: &CredentialRef) -> Result<(), SecretStoreError> {
        self.entries
            .lock()
            .expect("memory secret store mutex cannot be poisoned without a panic")
            .remove(credential_ref.as_str())
            .map_or(Err(SecretStoreError::NotFound), |_| Ok(()))
    }
}

/// Native credential storage selected for the current desktop platform.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlatformSecretStore;

impl PlatformSecretStore {
    fn entry(
        credential_ref: &CredentialRef,
        operation: SecretStoreOperation,
    ) -> Result<keyring::Entry, SecretStoreError> {
        keyring::Entry::new(SERVICE_NAME, credential_ref.as_str())
            .map_err(|source| SecretStoreError::Backend { operation, source })
    }
}

impl SecretStore for PlatformSecretStore {
    fn put(
        &self,
        credential_ref: &CredentialRef,
        secret: &SecretValue,
    ) -> Result<(), SecretStoreError> {
        let operation = SecretStoreOperation::Put;
        Self::entry(credential_ref, operation)?
            .set_secret(secret.expose_secret())
            .map_err(|source| SecretStoreError::Backend { operation, source })
    }

    fn get(&self, credential_ref: &CredentialRef) -> Result<SecretValue, SecretStoreError> {
        let operation = SecretStoreOperation::Get;
        let value = Self::entry(credential_ref, operation)?
            .get_secret()
            .map_err(|source| map_read_error(operation, source))?;
        SecretValue::new(value)
    }

    fn delete(&self, credential_ref: &CredentialRef) -> Result<(), SecretStoreError> {
        let operation = SecretStoreOperation::Delete;
        Self::entry(credential_ref, operation)?
            .delete_credential()
            .map_err(|source| map_read_error(operation, source))
    }
}

fn map_read_error(operation: SecretStoreOperation, source: keyring::Error) -> SecretStoreError {
    if matches!(source, keyring::Error::NoEntry) {
        SecretStoreError::NotFound
    } else {
        SecretStoreError::Backend { operation, source }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretStoreOperation {
    Put,
    Get,
    Delete,
}

impl Display for SecretStoreOperation {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Put => formatter.write_str("put"),
            Self::Get => formatter.write_str("get"),
            Self::Delete => formatter.write_str("delete"),
        }
    }
}

#[derive(Debug, Error)]
pub enum SecretStoreError {
    #[error("secret value must not be empty")]
    EmptySecret,
    #[error("credential was not found")]
    NotFound,
    #[error("operating-system credential store failed during {operation}")]
    Backend {
        operation: SecretStoreOperation,
        #[source]
        source: keyring::Error,
    },
}
