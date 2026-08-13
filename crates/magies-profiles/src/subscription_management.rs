use std::collections::HashSet;
use std::fmt::{Display, Formatter};

use magies_domain::{CredentialRef, NodeModelError, Subscription, SubscriptionModelError};
use magies_storage::{SecretStore, SecretStoreError, SecretValue};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

use crate::{DeletedSubscription, SqliteSubscriptionStore, SubscriptionTransactionError};

pub struct SubscriptionManagementService<'a, S: SecretStore + ?Sized> {
    store: &'a mut SqliteSubscriptionStore,
    secret_store: &'a S,
}

impl<'a, S: SecretStore + ?Sized> SubscriptionManagementService<'a, S> {
    #[must_use]
    pub const fn new(store: &'a mut SqliteSubscriptionStore, secret_store: &'a S) -> Self {
        Self {
            store,
            secret_store,
        }
    }

    /// Creates a subscription while keeping its URL outside `SQLite`.
    ///
    /// # Errors
    ///
    /// Returns a typed validation, credential-store, or database error. A
    /// database failure rolls back the newly written URL secret.
    #[expect(
        clippy::too_many_arguments,
        reason = "create mirrors the editable subscription fields one-for-one"
    )]
    pub fn create(
        &mut self,
        name: &str,
        url: &str,
        update_interval_minutes: u32,
        auto_update: bool,
        user_agent: Option<&str>,
        include_keywords: &str,
        exclude_keywords: &str,
    ) -> Result<Subscription, SubscriptionManagementError> {
        let url_secret = validated_url_secret(url)?;
        let id = Uuid::new_v4();
        let url_secret_ref = CredentialRef::new(format!("subscription/{id}/url"))
            .map_err(SubscriptionManagementError::CredentialRef)?;
        let mut subscription =
            Subscription::new(id, name, url_secret_ref.clone(), update_interval_minutes)?;
        subscription.auto_update = auto_update;
        subscription.user_agent = normalize_optional_text(user_agent);
        include_keywords
            .trim()
            .clone_into(&mut subscription.include_keywords);
        exclude_keywords
            .trim()
            .clone_into(&mut subscription.exclude_keywords);

        self.secret_store
            .put(&url_secret_ref, &url_secret)
            .map_err(|source| SubscriptionManagementError::SecretStore {
                operation: SubscriptionManagementSecretOperation::SaveUrl,
                source,
            })?;
        if let Err(transaction) = self.store.insert_subscription(&subscription) {
            return match self.secret_store.delete(&url_secret_ref) {
                Ok(()) => Err(transaction.into()),
                Err(secret) => Err(SubscriptionManagementError::TransactionAndSecretRollback {
                    transaction,
                    secret,
                }),
            };
        }
        Ok(subscription)
    }

    /// Lists subscriptions without reading or exposing their URL secrets.
    ///
    /// # Errors
    ///
    /// Returns a typed database or stored-model error.
    pub fn subscriptions(&self) -> Result<Vec<Subscription>, SubscriptionManagementError> {
        self.store.subscriptions().map_err(Into::into)
    }

    /// Updates subscription settings and optionally replaces its URL secret.
    ///
    /// # Errors
    ///
    /// Returns a typed validation, credential-store, or database error. When a
    /// settings write fails, the previous URL secret is restored.
    #[expect(
        clippy::too_many_arguments,
        reason = "update mirrors the editable subscription fields one-for-one"
    )]
    pub fn update(
        &mut self,
        id: Uuid,
        name: &str,
        update_interval_minutes: u32,
        auto_update: bool,
        enabled: bool,
        new_url: Option<&str>,
        user_agent: Option<&str>,
        include_keywords: &str,
        exclude_keywords: &str,
    ) -> Result<Subscription, SubscriptionManagementError> {
        let existing = self
            .store
            .subscription(id)?
            .ok_or(SubscriptionTransactionError::SubscriptionNotFound { id })?;
        let mut edited = Subscription::new(
            id,
            name,
            existing.url_secret_ref.clone(),
            update_interval_minutes,
        )?;
        edited.auto_update = auto_update;
        edited.enabled = enabled;
        edited.etag = existing.etag;
        edited.last_modified = existing.last_modified;
        edited.last_updated_at = existing.last_updated_at;
        edited.user_agent = normalize_optional_text(user_agent);
        include_keywords
            .trim()
            .clone_into(&mut edited.include_keywords);
        exclude_keywords
            .trim()
            .clone_into(&mut edited.exclude_keywords);

        let previous_url = if let Some(new_url) = new_url {
            let new_url = validated_url_secret(new_url)?;
            let previous_url = self
                .secret_store
                .get(&edited.url_secret_ref)
                .map_err(|source| SubscriptionManagementError::SecretStore {
                    operation: SubscriptionManagementSecretOperation::ReadUrl,
                    source,
                })?;
            self.secret_store
                .put(&edited.url_secret_ref, &new_url)
                .map_err(|source| SubscriptionManagementError::SecretStore {
                    operation: SubscriptionManagementSecretOperation::SaveUrl,
                    source,
                })?;
            Some(previous_url)
        } else {
            None
        };

        if let Err(transaction) = self.store.update_subscription_settings(&edited) {
            if let Some(previous_url) = previous_url {
                return match self.secret_store.put(&edited.url_secret_ref, &previous_url) {
                    Ok(()) => Err(transaction.into()),
                    Err(secret) => Err(SubscriptionManagementError::TransactionAndSecretRestore {
                        transaction,
                        secret,
                    }),
                };
            }
            return Err(transaction.into());
        }
        Ok(edited)
    }

    /// Deletes a subscription and all of its owned URL and node credentials.
    ///
    /// # Errors
    ///
    /// Returns a typed transaction or credential-cleanup error. Credential
    /// cleanup runs only after the database deletion commits.
    pub fn delete(&mut self, id: Uuid) -> Result<DeletedSubscription, SubscriptionManagementError> {
        let deleted = self.store.delete_subscription(id)?;
        let mut credential_refs = Vec::with_capacity(deleted.nodes.len() + 1);
        credential_refs.push((
            deleted.subscription.url_secret_ref.clone(),
            SubscriptionManagementSecretOperation::DeleteUrl,
        ));
        credential_refs.extend(deleted.nodes.iter().map(|node| {
            (
                node.credential_ref.clone(),
                SubscriptionManagementSecretOperation::DeleteNode,
            )
        }));

        let mut removed = HashSet::new();
        for (credential_ref, operation) in credential_refs {
            if !removed.insert(credential_ref.as_str().to_owned()) {
                continue;
            }
            self.secret_store
                .delete(&credential_ref)
                .map_err(|source| SubscriptionManagementError::SecretStore { operation, source })?;
        }
        Ok(deleted)
    }
}

fn validated_url_secret(url: &str) -> Result<SecretValue, SubscriptionManagementError> {
    let parsed =
        Url::parse(url).map_err(|source| SubscriptionManagementError::InvalidUrl { source })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(SubscriptionManagementError::UnsupportedScheme {
            scheme: parsed.scheme().to_owned(),
        });
    }
    SecretValue::new(url.as_bytes().to_vec()).map_err(|source| {
        SubscriptionManagementError::SecretStore {
            operation: SubscriptionManagementSecretOperation::SaveUrl,
            source,
        }
    })
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionManagementSecretOperation {
    ReadUrl,
    SaveUrl,
    DeleteUrl,
    DeleteNode,
}

impl Display for SubscriptionManagementSecretOperation {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadUrl => formatter.write_str("read subscription URL"),
            Self::SaveUrl => formatter.write_str("save subscription URL"),
            Self::DeleteUrl => formatter.write_str("delete subscription URL"),
            Self::DeleteNode => formatter.write_str("delete subscription node"),
        }
    }
}

#[derive(Debug, Error)]
pub enum SubscriptionManagementError {
    #[error("invalid subscription URL")]
    InvalidUrl { source: url::ParseError },
    #[error("unsupported subscription URL scheme: {scheme}")]
    UnsupportedScheme { scheme: String },
    #[error("failed to build the subscription URL credential reference")]
    CredentialRef(#[source] NodeModelError),
    #[error(transparent)]
    Model(#[from] SubscriptionModelError),
    #[error(transparent)]
    Transaction(#[from] SubscriptionTransactionError),
    #[error("subscription credential store failed while attempting to {operation}")]
    SecretStore {
        operation: SubscriptionManagementSecretOperation,
        source: SecretStoreError,
    },
    #[error("subscription database write failed and URL-secret rollback also failed")]
    TransactionAndSecretRollback {
        transaction: SubscriptionTransactionError,
        secret: SecretStoreError,
    },
    #[error("subscription database write failed and the previous URL secret could not be restored")]
    TransactionAndSecretRestore {
        transaction: SubscriptionTransactionError,
        secret: SecretStoreError,
    },
}
