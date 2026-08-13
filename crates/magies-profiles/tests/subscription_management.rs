use magies_domain::{CredentialRef, SubscriptionName, TimestampMillis};
use magies_profiles::{
    CredentialCodec, ShareLinkParser, SqliteSubscriptionStore, SubscriptionManagementError,
    SubscriptionManagementService, SubscriptionUpdate, SubscriptionValidators,
};
use magies_storage::{MemorySecretStore, SecretStore, SecretStoreError, SecretValue};
use uuid::Uuid;

const OLD_URL: &str = "https://example.com/list?token=old-url-secret";
const NEW_URL: &str = "https://example.com/list?token=new-url-secret";

#[test]
fn creates_and_lists_a_subscription_without_persisting_its_url_in_sqlite() {
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let secrets = MemorySecretStore::default();

    let created = SubscriptionManagementService::new(&mut store, &secrets)
        .create(" Primary ", OLD_URL, 60, true, None, "", "")
        .unwrap();

    assert_eq!(created.name.as_str(), "Primary");
    assert_eq!(created.update_interval_minutes.get(), 60);
    assert!(created.auto_update);
    assert!(created.enabled);
    assert_eq!(
        secrets
            .get(&created.url_secret_ref)
            .unwrap()
            .expose_secret(),
        OLD_URL.as_bytes()
    );
    let subscriptions = SubscriptionManagementService::new(&mut store, &secrets)
        .subscriptions()
        .unwrap();
    assert_eq!(subscriptions, vec![created.clone()]);
    let debug_output = format!("{created:?}");
    assert!(!debug_output.contains("old-url-secret"));
}

#[test]
fn rejects_invalid_urls_before_changing_storage() {
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let secrets = MemorySecretStore::default();
    let mut service = SubscriptionManagementService::new(&mut store, &secrets);

    let invalid = service
        .create("Invalid", "not a URL", 60, false, None, "", "")
        .unwrap_err();
    let unsupported = service
        .create("File", "file:///tmp/subscription", 60, false, None, "", "")
        .unwrap_err();

    assert!(matches!(
        invalid,
        SubscriptionManagementError::InvalidUrl { .. }
    ));
    assert!(matches!(
        unsupported,
        SubscriptionManagementError::UnsupportedScheme { .. }
    ));
    assert!(service.subscriptions().unwrap().is_empty());
}

#[test]
fn edits_settings_and_url_without_losing_fetch_state() {
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let secrets = MemorySecretStore::default();
    let created = SubscriptionManagementService::new(&mut store, &secrets)
        .create("Primary", OLD_URL, 60, true, None, "", "")
        .unwrap();
    store
        .touch_subscription(
            created.id,
            &SubscriptionValidators::new(Some("\"revision-1\"".to_owned()), None),
            TimestampMillis::new(100),
        )
        .unwrap();

    let edited = SubscriptionManagementService::new(&mut store, &secrets)
        .update(created.id, "Edited", 120, false, false, Some(NEW_URL), None, "", "")
        .unwrap();

    assert_eq!(edited.name, SubscriptionName::new("Edited").unwrap());
    assert_eq!(edited.update_interval_minutes.get(), 120);
    assert!(!edited.auto_update);
    assert!(!edited.enabled);
    assert_eq!(edited.etag.as_deref(), Some("\"revision-1\""));
    assert_eq!(edited.last_updated_at, Some(TimestampMillis::new(100)));
    assert_eq!(
        secrets
            .get(&created.url_secret_ref)
            .unwrap()
            .expose_secret(),
        NEW_URL.as_bytes()
    );
    assert_eq!(store.subscription(created.id).unwrap(), Some(edited));
}

#[test]
fn deletes_subscription_metadata_nodes_and_every_owned_secret() {
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let secrets = MemorySecretStore::default();
    let created = SubscriptionManagementService::new(&mut store, &secrets)
        .create("Primary", OLD_URL, 60, false, None, "", "")
        .unwrap();
    let node_ref = credential_ref("subscription/primary/node/one");
    let parsed = ShareLinkParser
        .parse(
            "trojan://password@edge.example.com:443?security=tls#Edge",
            uuid("018f78b5-2cd0-7000-a9a6-3bccf60951e9"),
            node_ref.clone(),
        )
        .unwrap();
    let (mut node, credential) = parsed.into_parts();
    node.subscription_id = Some(created.id);
    secrets
        .put(&node_ref, &CredentialCodec::encode(&credential).unwrap())
        .unwrap();
    store
        .apply_update(
            &SubscriptionUpdate::new(
                created.id,
                vec![node.clone()],
                SubscriptionValidators::default(),
                TimestampMillis::new(100),
            )
            .unwrap(),
        )
        .unwrap();

    let deleted = SubscriptionManagementService::new(&mut store, &secrets)
        .delete(created.id)
        .unwrap();

    assert_eq!(deleted.subscription.id, created.id);
    assert_eq!(deleted.nodes, vec![node]);
    assert!(store.subscription(created.id).unwrap().is_none());
    assert!(matches!(
        secrets.get(&created.url_secret_ref),
        Err(SecretStoreError::NotFound)
    ));
    assert!(matches!(
        secrets.get(&node_ref),
        Err(SecretStoreError::NotFound)
    ));
}

#[test]
fn failed_url_write_keeps_the_subscription_database_empty() {
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let secrets = RejectingSecretStore;

    let error = SubscriptionManagementService::new(&mut store, &secrets)
        .create("Primary", OLD_URL, 60, false, None, "", "")
        .unwrap_err();

    assert!(matches!(
        &error,
        SubscriptionManagementError::SecretStore {
            source: SecretStoreError::EmptySecret,
            ..
        }
    ));
    assert!(error.to_string().contains("save subscription URL"));
    assert!(store.subscriptions().unwrap().is_empty());
}

#[test]
fn settings_only_update_does_not_require_the_url_secret() {
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let secrets = MemorySecretStore::default();
    let subscription = magies_domain::Subscription::new(
        uuid("018f78b5-2cd0-7000-a9a6-3bccf60951e8"),
        "Primary",
        credential_ref("subscription/missing/url"),
        60,
    )
    .unwrap();
    store.insert_subscription(&subscription).unwrap();

    let edited = SubscriptionManagementService::new(&mut store, &secrets)
        .update(subscription.id, "Renamed", 30, true, false, None, None, "", "")
        .unwrap();

    assert_eq!(edited.name.as_str(), "Renamed");
    assert_eq!(edited.update_interval_minutes.get(), 30);
    assert!(edited.auto_update);
    assert!(!edited.enabled);
}

#[test]
fn update_reports_missing_subscription_and_missing_previous_url_secret() {
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let secrets = MemorySecretStore::default();
    let missing_id = uuid("018f78b5-2cd0-7000-a9a6-3bccf60951e8");

    let missing = SubscriptionManagementService::new(&mut store, &secrets)
        .update(missing_id, "Missing", 60, false, true, None, None, "", "")
        .unwrap_err();
    assert!(matches!(
        missing,
        SubscriptionManagementError::Transaction(
            magies_profiles::SubscriptionTransactionError::SubscriptionNotFound { .. }
        )
    ));

    let subscription = magies_domain::Subscription::new(
        missing_id,
        "Primary",
        credential_ref("subscription/missing/url"),
        60,
    )
    .unwrap();
    store.insert_subscription(&subscription).unwrap();
    let missing_secret = SubscriptionManagementService::new(&mut store, &secrets)
        .update(subscription.id, "Primary", 60, false, true, Some(NEW_URL), None, "", "")
        .unwrap_err();
    assert!(matches!(
        missing_secret,
        SubscriptionManagementError::SecretStore {
            operation: magies_profiles::SubscriptionManagementSecretOperation::ReadUrl,
            source: SecretStoreError::NotFound,
        }
    ));
}

#[test]
fn delete_commits_metadata_removal_before_reporting_secret_cleanup_failure() {
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let secrets = RejectingSecretStore;
    let subscription = magies_domain::Subscription::new(
        uuid("018f78b5-2cd0-7000-a9a6-3bccf60951e8"),
        "Primary",
        credential_ref("subscription/missing/url"),
        60,
    )
    .unwrap();
    store.insert_subscription(&subscription).unwrap();

    let error = SubscriptionManagementService::new(&mut store, &secrets)
        .delete(subscription.id)
        .unwrap_err();

    assert!(matches!(
        error,
        SubscriptionManagementError::SecretStore {
            operation: magies_profiles::SubscriptionManagementSecretOperation::DeleteUrl,
            source: SecretStoreError::NotFound,
        }
    ));
    assert!(store.subscription(subscription.id).unwrap().is_none());
}

#[test]
fn secret_operations_have_stable_diagnostic_labels() {
    use magies_profiles::SubscriptionManagementSecretOperation::{DeleteNode, DeleteUrl, ReadUrl};

    assert_eq!(ReadUrl.to_string(), "read subscription URL");
    assert_eq!(DeleteUrl.to_string(), "delete subscription URL");
    assert_eq!(DeleteNode.to_string(), "delete subscription node");
}

struct RejectingSecretStore;

impl SecretStore for RejectingSecretStore {
    fn put(
        &self,
        _credential_ref: &CredentialRef,
        _secret: &SecretValue,
    ) -> Result<(), SecretStoreError> {
        Err(SecretStoreError::EmptySecret)
    }

    fn get(&self, _credential_ref: &CredentialRef) -> Result<SecretValue, SecretStoreError> {
        Err(SecretStoreError::NotFound)
    }

    fn delete(&self, _credential_ref: &CredentialRef) -> Result<(), SecretStoreError> {
        Err(SecretStoreError::NotFound)
    }
}

fn credential_ref(value: &str) -> CredentialRef {
    CredentialRef::new(value).unwrap()
}

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap()
}
