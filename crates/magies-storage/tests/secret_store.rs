use std::time::{SystemTime, UNIX_EPOCH};

use magies_domain::CredentialRef;
use magies_storage::{
    MemorySecretStore, PlatformSecretStore, SecretStore, SecretStoreError, SecretValue,
};

fn reference(suffix: &str) -> CredentialRef {
    CredentialRef::new(format!("secret://tests/{suffix}")).unwrap()
}

fn assert_store_contract(store: &impl SecretStore, credential_ref: &CredentialRef) {
    let first = SecretValue::new(b"first-secret".to_vec()).unwrap();
    store.put(credential_ref, &first).unwrap();
    assert_eq!(
        store.get(credential_ref).unwrap().expose_secret(),
        b"first-secret"
    );

    let replacement = SecretValue::new(b"replacement-secret".to_vec()).unwrap();
    store.put(credential_ref, &replacement).unwrap();
    assert_eq!(
        store.get(credential_ref).unwrap().expose_secret(),
        b"replacement-secret"
    );

    store.delete(credential_ref).unwrap();
    assert!(matches!(
        store.get(credential_ref),
        Err(SecretStoreError::NotFound)
    ));
    assert!(matches!(
        store.delete(credential_ref),
        Err(SecretStoreError::NotFound)
    ));
}

#[test]
fn secret_value_rejects_empty_data_and_redacts_debug_output() {
    assert!(matches!(
        SecretValue::new(Vec::new()),
        Err(SecretStoreError::EmptySecret)
    ));

    let secret = SecretValue::new(b"never-print-this".to_vec()).unwrap();
    let debug = format!("{secret:?}");
    assert_eq!(debug, "SecretValue([REDACTED])");
    assert!(!debug.contains("never-print-this"));
}

#[test]
fn memory_store_obeys_secret_store_contract() {
    assert_store_contract(&MemorySecretStore::default(), &reference("memory"));
}

#[test]
fn memory_store_errors_do_not_expose_reference_or_secret() {
    let store = MemorySecretStore::default();
    let credential_ref = reference("sensitive-reference");
    let secret = SecretValue::new(b"sensitive-secret".to_vec()).unwrap();
    store.put(&credential_ref, &secret).unwrap();
    store.delete(&credential_ref).unwrap();

    let error = store.get(&credential_ref).unwrap_err();
    let diagnostics = format!("{error:?} {error}");
    assert!(!diagnostics.contains("sensitive-reference"));
    assert!(!diagnostics.contains("sensitive-secret"));
}

#[test]
#[ignore = "uses the signed-in operating-system credential store"]
fn platform_store_obeys_secret_store_contract() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let credential_ref = reference(&format!("platform-{}-{unique}", std::process::id()));
    let store = PlatformSecretStore;
    let _ = store.delete(&credential_ref);
    assert_store_contract(&store, &credential_ref);
}
