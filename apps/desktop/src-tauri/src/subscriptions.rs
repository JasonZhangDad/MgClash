use std::sync::{Mutex, MutexGuard, PoisonError};

use magies_domain::{Subscription, TimestampMillis};
use magies_profiles::{
    SqliteSubscriptionStore, SubscriptionFetchError, SubscriptionFetchOptions, SubscriptionFetcher,
    SubscriptionManagementError, SubscriptionManagementService, SubscriptionRefreshError,
    SubscriptionRefreshService, SubscriptionTransactionError,
};
use magies_storage::SecretStore;
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSubscriptionSummary {
    pub id: Uuid,
    pub name: String,
    pub update_interval_minutes: u32,
    pub auto_update: bool,
    pub last_updated_at: Option<i64>,
    pub enabled: bool,
    pub node_count: usize,
}

pub struct DesktopSubscriptionController<S: SecretStore> {
    store: Mutex<SqliteSubscriptionStore>,
    secret_store: S,
}

impl<S: SecretStore> DesktopSubscriptionController<S> {
    #[must_use]
    pub const fn new(store: SqliteSubscriptionStore, secret_store: S) -> Self {
        Self {
            store: Mutex::new(store),
            secret_store,
        }
    }

    /// Lists subscriptions without exposing their URL credentials.
    ///
    /// # Errors
    ///
    /// Returns a typed store error when metadata or node counts cannot be read.
    pub fn list(&self) -> Result<Vec<DesktopSubscriptionSummary>, DesktopSubscriptionError> {
        let store = self.store();
        store
            .subscriptions()?
            .iter()
            .map(|subscription| summary(&store, subscription))
            .collect()
    }

    /// Creates one subscription and returns its redacted desktop summary.
    ///
    /// # Errors
    ///
    /// Returns a typed validation, secret-store, or database error.
    pub fn create(
        &self,
        name: &str,
        url: &str,
        update_interval_minutes: u32,
        auto_update: bool,
    ) -> Result<DesktopSubscriptionSummary, DesktopSubscriptionError> {
        let mut store = self.store();
        let subscription = SubscriptionManagementService::new(&mut store, &self.secret_store)
            .create(name, url, update_interval_minutes, auto_update)?;
        summary(&store, &subscription)
    }

    /// Updates editable settings and optionally replaces the URL credential.
    ///
    /// # Errors
    ///
    /// Returns a typed validation, secret-store, or database error.
    pub fn update(
        &self,
        id: Uuid,
        name: &str,
        update_interval_minutes: u32,
        auto_update: bool,
        enabled: bool,
        url: Option<&str>,
    ) -> Result<DesktopSubscriptionSummary, DesktopSubscriptionError> {
        let mut store = self.store();
        let subscription = SubscriptionManagementService::new(&mut store, &self.secret_store)
            .update(id, name, update_interval_minutes, auto_update, enabled, url)?;
        summary(&store, &subscription)
    }

    /// Deletes one subscription and every owned credential.
    ///
    /// # Errors
    ///
    /// Returns a typed store or credential-cleanup error.
    pub fn delete(&self, id: Uuid) -> Result<(), DesktopSubscriptionError> {
        SubscriptionManagementService::new(&mut self.store(), &self.secret_store).delete(id)?;
        Ok(())
    }

    /// Refreshes one subscription on the Tauri async runtime.
    ///
    /// # Errors
    ///
    /// Returns a typed fetch, parsing, credential, or transaction error.
    pub fn refresh(
        &self,
        id: Uuid,
        updated_at: TimestampMillis,
    ) -> Result<DesktopSubscriptionSummary, DesktopSubscriptionError> {
        let mut store = self.store();
        let fetcher = SubscriptionFetcher::new(SubscriptionFetchOptions::default())?;
        let mut service = SubscriptionRefreshService::new(&mut store, &self.secret_store, fetcher);
        tauri::async_runtime::block_on(service.refresh(id, updated_at))?;
        let subscription = store
            .subscription(id)?
            .ok_or(SubscriptionTransactionError::SubscriptionNotFound { id })?;
        summary(&store, &subscription)
    }

    fn store(&self) -> MutexGuard<'_, SqliteSubscriptionStore> {
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn summary(
    store: &SqliteSubscriptionStore,
    subscription: &Subscription,
) -> Result<DesktopSubscriptionSummary, DesktopSubscriptionError> {
    Ok(DesktopSubscriptionSummary {
        id: subscription.id,
        name: subscription.name.as_str().to_owned(),
        update_interval_minutes: subscription.update_interval_minutes.get(),
        auto_update: subscription.auto_update,
        last_updated_at: subscription.last_updated_at.map(TimestampMillis::get),
        enabled: subscription.enabled,
        node_count: store.subscription_nodes(subscription.id)?.len(),
    })
}

#[derive(Debug, Error)]
pub enum DesktopSubscriptionError {
    #[error(transparent)]
    Management(Box<SubscriptionManagementError>),
    #[error(transparent)]
    Transaction(#[from] SubscriptionTransactionError),
    #[error(transparent)]
    Fetcher(#[from] SubscriptionFetchError),
    #[error(transparent)]
    Refresh(#[from] SubscriptionRefreshError),
}

impl DesktopSubscriptionError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Management(error) => management_error_code(error),
            Self::Transaction(SubscriptionTransactionError::SubscriptionNotFound { .. })
            | Self::Refresh(SubscriptionRefreshError::Transaction(
                SubscriptionTransactionError::SubscriptionNotFound { .. },
            )) => "subscription_not_found",
            Self::Transaction(_) | Self::Refresh(SubscriptionRefreshError::Transaction(_)) => {
                "subscription_store_failed"
            }
            Self::Fetcher(_) => "subscription_fetcher_failed",
            Self::Refresh(SubscriptionRefreshError::Fetch(_)) => "subscription_fetch_failed",
            Self::Refresh(SubscriptionRefreshError::Content { .. }) => {
                "invalid_subscription_content"
            }
            Self::Refresh(
                SubscriptionRefreshError::InvalidUrlSecret { .. }
                | SubscriptionRefreshError::SecretStore { .. }
                | SubscriptionRefreshError::Credential { .. },
            ) => "secret_store_failed",
        }
    }
}

impl From<SubscriptionManagementError> for DesktopSubscriptionError {
    fn from(source: SubscriptionManagementError) -> Self {
        Self::Management(Box::new(source))
    }
}

const fn management_error_code(error: &SubscriptionManagementError) -> &'static str {
    match error {
        SubscriptionManagementError::InvalidUrl { .. }
        | SubscriptionManagementError::UnsupportedScheme { .. }
        | SubscriptionManagementError::CredentialRef(_)
        | SubscriptionManagementError::Model(_) => "invalid_subscription",
        SubscriptionManagementError::Transaction(
            SubscriptionTransactionError::SubscriptionNotFound { .. },
        ) => "subscription_not_found",
        SubscriptionManagementError::Transaction(_) => "subscription_store_failed",
        SubscriptionManagementError::SecretStore { .. }
        | SubscriptionManagementError::TransactionAndSecretRollback { .. }
        | SubscriptionManagementError::TransactionAndSecretRestore { .. } => "secret_store_failed",
    }
}

#[cfg(test)]
mod tests {
    use magies_domain::TimestampMillis;
    use magies_profiles::{SqliteSubscriptionStore, SubscriptionTransactionError};
    use magies_storage::MemorySecretStore;
    use uuid::Uuid;

    use super::{DesktopSubscriptionController, DesktopSubscriptionError};

    #[test]
    fn manages_redacted_subscription_summaries() {
        let store = SqliteSubscriptionStore::open_in_memory().unwrap();
        let controller = DesktopSubscriptionController::new(store, MemorySecretStore::default());

        let created = controller
            .create(
                "Primary",
                "https://example.com/list?token=url-secret",
                60,
                true,
            )
            .unwrap();
        assert_eq!(created.name, "Primary");
        assert_eq!(created.node_count, 0);
        assert_eq!(created.last_updated_at, None);
        assert!(created.auto_update);
        assert!(!format!("{created:?}").contains("url-secret"));

        let edited = controller
            .update(
                created.id,
                "Edited",
                120,
                false,
                false,
                Some("https://example.com/new?token=new-url-secret"),
            )
            .unwrap();
        assert_eq!(edited.name, "Edited");
        assert_eq!(edited.update_interval_minutes, 120);
        assert!(!edited.enabled);
        assert_eq!(controller.list().unwrap(), vec![edited]);

        controller.delete(created.id).unwrap();
        assert!(controller.list().unwrap().is_empty());
    }

    #[test]
    fn refresh_reports_a_missing_subscription_without_network_access() {
        let store = SqliteSubscriptionStore::open_in_memory().unwrap();
        let controller = DesktopSubscriptionController::new(store, MemorySecretStore::default());

        let error = controller
            .refresh(Uuid::nil(), TimestampMillis::new(100))
            .unwrap_err();

        assert!(matches!(
            error,
            DesktopSubscriptionError::Refresh(
                magies_profiles::SubscriptionRefreshError::Transaction(
                    SubscriptionTransactionError::SubscriptionNotFound { .. }
                )
            )
        ));
        assert_eq!(error.code(), "subscription_not_found");
    }
}
