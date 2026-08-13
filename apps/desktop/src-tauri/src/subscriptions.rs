use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use std::str;

use magies_domain::{Subscription, TimestampMillis};
use magies_profiles::{
    SqliteSubscriptionStore, SubscriptionFetchError, SubscriptionFetchOptions, SubscriptionFetcher,
    SubscriptionManagementError, SubscriptionManagementService, SubscriptionRefreshError,
    SubscriptionRefreshService, SubscriptionTransactionError,
};
use magies_storage::{SecretStore, SecretStoreError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionBackupEntry {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub update_interval_minutes: u32,
    pub auto_update: bool,
    pub enabled: bool,
    pub user_agent: Option<String>,
    pub include_keywords: String,
    pub exclude_keywords: String,
    pub subconverter_url: Option<String>,
}

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
    pub last_error: Option<String>,
    pub user_agent: Option<String>,
    pub include_keywords: String,
    pub exclude_keywords: String,
    pub subconverter_url: Option<String>,
}

pub struct DesktopSubscriptionController<S: SecretStore> {
    store: Mutex<SqliteSubscriptionStore>,
    secret_store: S,
    errors: Mutex<HashMap<Uuid, String>>,
}

impl<S: SecretStore> DesktopSubscriptionController<S> {
    #[must_use]
    pub fn new(store: SqliteSubscriptionStore, secret_store: S) -> Self {
        Self {
            store: Mutex::new(store),
            secret_store,
            errors: Mutex::new(HashMap::new()),
        }
    }

    /// Lists subscriptions without exposing their URL credentials.
    ///
    /// # Errors
    ///
    /// Returns a typed store error when metadata or node counts cannot be read.
    pub fn list(&self) -> Result<Vec<DesktopSubscriptionSummary>, DesktopSubscriptionError> {
        let store = self.store();
        let errors = self.errors();
        store
            .subscriptions()?
            .iter()
            .map(|subscription| {
                summary(&store, subscription, errors.get(&subscription.id).cloned())
            })
            .collect()
    }

    /// Creates one subscription and returns its redacted desktop summary.
    ///
    /// # Errors
    ///
    /// Returns a typed validation, secret-store, or database error.
    #[expect(
        clippy::too_many_arguments,
        reason = "create mirrors the editable subscription fields one-for-one"
    )]
    pub fn create(
        &self,
        name: &str,
        url: &str,
        update_interval_minutes: u32,
        auto_update: bool,
        user_agent: Option<&str>,
        include_keywords: &str,
        exclude_keywords: &str,
        subconverter_url: Option<&str>,
    ) -> Result<DesktopSubscriptionSummary, DesktopSubscriptionError> {
        let mut store = self.store();
        let subscription = SubscriptionManagementService::new(&mut store, &self.secret_store)
            .create(
                name,
                url,
                update_interval_minutes,
                auto_update,
                user_agent,
                include_keywords,
                exclude_keywords,
                subconverter_url,
            )?;
        summary(&store, &subscription, None)
    }

    /// Updates editable settings and optionally replaces the URL credential.
    ///
    /// # Errors
    ///
    /// Returns a typed validation, secret-store, or database error.
    #[expect(
        clippy::too_many_arguments,
        reason = "update mirrors the editable subscription fields one-for-one"
    )]
    pub fn update(
        &self,
        id: Uuid,
        name: &str,
        update_interval_minutes: u32,
        auto_update: bool,
        enabled: bool,
        url: Option<&str>,
        user_agent: Option<&str>,
        include_keywords: &str,
        exclude_keywords: &str,
        subconverter_url: Option<&str>,
    ) -> Result<DesktopSubscriptionSummary, DesktopSubscriptionError> {
        let mut store = self.store();
        let subscription = SubscriptionManagementService::new(&mut store, &self.secret_store)
            .update(
                id,
                name,
                update_interval_minutes,
                auto_update,
                enabled,
                url,
                user_agent,
                include_keywords,
                exclude_keywords,
                subconverter_url,
            )?;
        summary(
            &store,
            &subscription,
            self.errors().get(&subscription.id).cloned(),
        )
    }

    /// Deletes one subscription and every owned credential.
    ///
    /// # Errors
    ///
    /// Returns a typed store or credential-cleanup error.
    pub fn delete(&self, id: Uuid) -> Result<(), DesktopSubscriptionError> {
        SubscriptionManagementService::new(&mut self.store(), &self.secret_store).delete(id)?;
        self.errors().remove(&id);
        Ok(())
    }

    /// Exports every subscription with its URL secret for a full profile backup.
    ///
    /// # Errors
    ///
    /// Returns a typed store or secret-store error.
    pub fn export_backup_entries(
        &self,
    ) -> Result<Vec<SubscriptionBackupEntry>, DesktopSubscriptionError> {
        let store = self.store();
        store
            .subscriptions()?
            .iter()
            .map(|subscription| {
                let secret = self
                    .secret_store
                    .get(&subscription.url_secret_ref)
                    .map_err(|source| DesktopSubscriptionError::SecretRead { source })?;
                let url = str::from_utf8(secret.expose_secret())
                    .map_err(|source| DesktopSubscriptionError::InvalidUrlSecret { source })?
                    .to_owned();
                Ok(SubscriptionBackupEntry {
                    id: subscription.id,
                    name: subscription.name.as_str().to_owned(),
                    url,
                    update_interval_minutes: subscription.update_interval_minutes.get(),
                    auto_update: subscription.auto_update,
                    enabled: subscription.enabled,
                    user_agent: subscription.user_agent.clone(),
                    include_keywords: subscription.include_keywords.clone(),
                    exclude_keywords: subscription.exclude_keywords.clone(),
                    subconverter_url: subscription.subconverter_url.clone(),
                })
            })
            .collect()
    }

    /// Replaces every subscription from a profile backup.
    ///
    /// Existing subscriptions and their node credentials are deleted first.
    ///
    /// # Errors
    ///
    /// Returns a typed validation, secret-store, or database error.
    pub fn replace_from_backup(
        &self,
        entries: &[SubscriptionBackupEntry],
    ) -> Result<(), DesktopSubscriptionError> {
        let existing = self
            .store()
            .subscriptions()?
            .into_iter()
            .map(|subscription| subscription.id)
            .collect::<Vec<_>>();
        for id in existing {
            self.delete(id)?;
        }
        for entry in entries {
            SubscriptionManagementService::new(&mut self.store(), &self.secret_store).restore(
                entry.id,
                &entry.name,
                &entry.url,
                entry.update_interval_minutes,
                entry.auto_update,
                entry.enabled,
                entry.user_agent.as_deref(),
                &entry.include_keywords,
                &entry.exclude_keywords,
                entry.subconverter_url.as_deref(),
            )?;
        }
        self.errors().clear();
        Ok(())
    }

    /// Returns enabled automatic subscriptions whose interval has elapsed.
    ///
    /// # Errors
    ///
    /// Returns a typed store error when subscription metadata cannot be read.
    pub fn due_auto_update_ids(
        &self,
        now: TimestampMillis,
    ) -> Result<Vec<Uuid>, DesktopSubscriptionError> {
        Ok(self
            .store()
            .subscriptions()?
            .into_iter()
            .filter(|subscription| subscription_is_due(subscription, now))
            .map(|subscription| subscription.id)
            .collect())
    }

    /// Refreshes every enabled subscription and records individual failures.
    ///
    /// # Errors
    ///
    /// Returns a typed store error when the enabled subscription list or final
    /// summaries cannot be read. Individual refresh failures remain visible in
    /// each returned summary.
    pub fn refresh_all(
        &self,
        updated_at: TimestampMillis,
    ) -> Result<Vec<DesktopSubscriptionSummary>, DesktopSubscriptionError> {
        let ids = self
            .store()
            .subscriptions()?
            .into_iter()
            .filter(|subscription| subscription.enabled)
            .map(|subscription| subscription.id)
            .collect::<Vec<_>>();
        for id in ids {
            let _ = self.refresh(id, updated_at);
        }
        self.list()
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
        let result = self.refresh_inner(id, updated_at);
        match result {
            Ok(mut summary) => {
                self.errors().remove(&id);
                summary.last_error = None;
                Ok(summary)
            }
            Err(error) => {
                self.errors().insert(id, error.to_string());
                Err(error)
            }
        }
    }

    fn refresh_inner(
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
        summary(
            &store,
            &subscription,
            self.errors().get(&subscription.id).cloned(),
        )
    }

    fn store(&self) -> MutexGuard<'_, SqliteSubscriptionStore> {
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn errors(&self) -> MutexGuard<'_, HashMap<Uuid, String>> {
        self.errors.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn subscription_is_due(subscription: &Subscription, now: TimestampMillis) -> bool {
    if !subscription.enabled || !subscription.auto_update {
        return false;
    }
    subscription.last_updated_at.is_none_or(|last_updated_at| {
        let interval_millis = i64::from(subscription.update_interval_minutes.get()) * 60_000;
        now.get().saturating_sub(last_updated_at.get()) >= interval_millis
    })
}

fn summary(
    store: &SqliteSubscriptionStore,
    subscription: &Subscription,
    last_error: Option<String>,
) -> Result<DesktopSubscriptionSummary, DesktopSubscriptionError> {
    Ok(DesktopSubscriptionSummary {
        id: subscription.id,
        name: subscription.name.as_str().to_owned(),
        update_interval_minutes: subscription.update_interval_minutes.get(),
        auto_update: subscription.auto_update,
        last_updated_at: subscription.last_updated_at.map(TimestampMillis::get),
        enabled: subscription.enabled,
        node_count: store.subscription_nodes(subscription.id)?.len(),
        last_error,
        user_agent: subscription.user_agent.clone(),
        include_keywords: subscription.include_keywords.clone(),
        exclude_keywords: subscription.exclude_keywords.clone(),
        subconverter_url: subscription.subconverter_url.clone(),
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
    #[error("failed to read a subscription URL from the credential store")]
    SecretRead { source: SecretStoreError },
    #[error("stored subscription URL is not valid UTF-8")]
    InvalidUrlSecret { source: std::str::Utf8Error },
}

impl DesktopSubscriptionError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Management(error) => management_error_code(error),
            Self::SecretRead { .. } | Self::InvalidUrlSecret { .. } => "secret_store_failed",
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
            Self::Refresh(SubscriptionRefreshError::Url(_)) => "invalid_subscription",
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
        SubscriptionManagementError::Url(_)
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
    use std::sync::Arc;

    use magies_domain::TimestampMillis;
    use magies_profiles::{SqliteSubscriptionStore, SubscriptionTransactionError};
    use magies_storage::{MemorySecretStore, SecretStore, SecretStoreError, SecretValue};
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
                None,
                "",
                "",
                None,
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
                None,
                "",
                "",
                None,
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

    #[test]
    fn finds_only_due_enabled_automatic_subscriptions() {
        let store = SqliteSubscriptionStore::open_in_memory().unwrap();
        let controller = DesktopSubscriptionController::new(store, MemorySecretStore::default());
        let due = controller
            .create("Due", "https://example.com/due", 60, true, None, "", "", None)
            .unwrap();
        let manual = controller
            .create("Manual", "https://example.com/manual", 60, false, None, "", "", None)
            .unwrap();
        let future = controller
            .create("Future", "https://example.com/future", 60, true, None, "", "", None)
            .unwrap();
        controller
            .store()
            .touch_subscription(
                future.id,
                &magies_profiles::SubscriptionValidators::default(),
                TimestampMillis::new(10_000),
            )
            .unwrap();

        assert_eq!(
            controller
                .due_auto_update_ids(TimestampMillis::new(3_609_999))
                .unwrap(),
            vec![due.id]
        );
        assert_eq!(
            controller
                .due_auto_update_ids(TimestampMillis::new(3_610_000))
                .unwrap(),
            vec![due.id, future.id]
        );
        assert!(
            !controller
                .due_auto_update_ids(TimestampMillis::new(0))
                .unwrap()
                .contains(&future.id)
        );
        assert!(
            !controller
                .due_auto_update_ids(TimestampMillis::new(i64::MAX))
                .unwrap()
                .contains(&manual.id)
        );
    }

    #[test]
    fn export_and_replace_backup_entries_preserve_subscription_ids() {
        let store = SqliteSubscriptionStore::open_in_memory().unwrap();
        let controller = DesktopSubscriptionController::new(store, MemorySecretStore::default());
        let created = controller
            .create(
                "Primary",
                "https://example.com/list?token=secret",
                60,
                true,
                Some("custom-agent"),
                "hk",
                "ads",
                Some("https://sub.example.com"),
            )
            .unwrap();

        let exported = controller.export_backup_entries().unwrap();
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].id, created.id);
        assert!(exported[0].url.contains("secret"));
        assert_eq!(exported[0].user_agent.as_deref(), Some("custom-agent"));
        assert_eq!(exported[0].include_keywords, "hk");
        assert_eq!(exported[0].subconverter_url.as_deref(), Some("https://sub.example.com"));

        controller.replace_from_backup(&exported).unwrap();
        let listed = controller.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
        assert_eq!(listed[0].name, "Primary");
    }

    #[test]
    fn records_a_redacted_refresh_failure_for_the_ui() {
        let secrets = SharedSecretStore::default();
        let controller = DesktopSubscriptionController::new(
            SqliteSubscriptionStore::open_in_memory().unwrap(),
            secrets.clone(),
        );
        let created = controller
            .create("Broken", "https://example.com/?token=url-secret", 60, true, None, "", "", None)
            .unwrap();
        let url_ref =
            magies_domain::CredentialRef::new(format!("subscription/{}/url", created.id)).unwrap();
        secrets
            .put(&url_ref, &SecretValue::new(b"not a URL".to_vec()).unwrap())
            .unwrap();

        controller
            .refresh(created.id, TimestampMillis::new(100))
            .unwrap_err();

        let summary = controller.list().unwrap().pop().unwrap();
        assert!(summary.last_error.is_some());
        assert!(!summary.last_error.unwrap().contains("url-secret"));
    }

    #[test]
    fn batch_refresh_continues_after_an_individual_failure() {
        let secrets = SharedSecretStore::default();
        let controller = DesktopSubscriptionController::new(
            SqliteSubscriptionStore::open_in_memory().unwrap(),
            secrets.clone(),
        );
        let broken = controller
            .create("Broken", "https://example.com/broken", 60, true, None, "", "", None)
            .unwrap();
        let also_broken = controller
            .create("Also broken", "https://example.com/also-broken", 60, true, None, "", "", None)
            .unwrap();
        let disabled = controller
            .create("Disabled", "https://example.com/disabled", 60, true, None, "", "", None)
            .unwrap();
        controller
            .update(disabled.id, "Disabled", 60, true, false, None, None, "", "", None)
            .unwrap();
        for id in [broken.id, also_broken.id] {
            let url_ref =
                magies_domain::CredentialRef::new(format!("subscription/{id}/url")).unwrap();
            secrets
                .put(&url_ref, &SecretValue::new(b"not a URL".to_vec()).unwrap())
                .unwrap();
        }

        let summaries = controller.refresh_all(TimestampMillis::new(100)).unwrap();

        assert_eq!(summaries.len(), 3);
        for id in [broken.id, also_broken.id] {
            assert!(
                summaries
                    .iter()
                    .find(|summary| summary.id == id)
                    .unwrap()
                    .last_error
                    .is_some()
            );
        }
        assert!(
            summaries
                .iter()
                .find(|summary| summary.id == disabled.id)
                .unwrap()
                .last_error
                .is_none()
        );
    }

    #[derive(Clone, Default)]
    struct SharedSecretStore(Arc<MemorySecretStore>);

    impl SecretStore for SharedSecretStore {
        fn put(
            &self,
            credential_ref: &magies_domain::CredentialRef,
            secret: &SecretValue,
        ) -> Result<(), SecretStoreError> {
            self.0.put(credential_ref, secret)
        }

        fn get(
            &self,
            credential_ref: &magies_domain::CredentialRef,
        ) -> Result<SecretValue, SecretStoreError> {
            self.0.get(credential_ref)
        }

        fn delete(
            &self,
            credential_ref: &magies_domain::CredentialRef,
        ) -> Result<(), SecretStoreError> {
            self.0.delete(credential_ref)
        }
    }
}
