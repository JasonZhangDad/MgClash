use std::fs;
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};
use std::process::id;
use std::sync::atomic::{AtomicUsize, Ordering};

use magies_domain::{
    CredentialRef, NodeName, ProxyNode, ProxyProtocol, ServerAddress, TimestampMillis,
};
use magies_profiles::{ManualNodeStoreError, SqliteManualNodeStore};
use uuid::Uuid;

#[test]
fn stores_multiple_manual_nodes_and_restores_the_selection() {
    let database = TestDatabase::new("restore-selection");
    let tokyo = node("Tokyo", 8_388);
    let osaka = node("Osaka", 9_000);

    {
        let mut store = SqliteManualNodeStore::open(database.path()).unwrap();
        assert!(store.nodes().unwrap().is_empty());
        assert!(store.selected_node().unwrap().is_none());

        store.save_and_select(&tokyo).unwrap();
        store.save_and_select(&osaka).unwrap();

        assert_eq!(store.nodes().unwrap(), vec![tokyo.clone(), osaka.clone()]);
        assert_eq!(store.selected_node().unwrap(), Some(osaka.clone()));
    }

    let store = SqliteManualNodeStore::open(database.path()).unwrap();
    assert_eq!(store.nodes().unwrap(), vec![tokyo, osaka.clone()]);
    assert_eq!(store.selected_node().unwrap(), Some(osaka));
}

#[test]
fn selects_and_deletes_existing_nodes_without_losing_the_previous_selection() {
    let mut store = SqliteManualNodeStore::open_in_memory().unwrap();
    let tokyo = node("Tokyo", 8_388);
    let osaka = node("Osaka", 9_000);
    store.save_and_select(&tokyo).unwrap();
    store.save_and_select(&osaka).unwrap();

    store.select(tokyo.id).unwrap();
    assert_eq!(store.selected_node().unwrap(), Some(tokyo.clone()));

    let missing = uuid();
    assert!(matches!(
        store.select(missing),
        Err(ManualNodeStoreError::NodeNotFound { id }) if id == missing
    ));
    assert_eq!(store.selected_node().unwrap(), Some(tokyo.clone()));

    assert_eq!(store.delete(tokyo.id).unwrap(), tokyo);
    assert!(store.selected_node().unwrap().is_none());
    assert_eq!(store.nodes().unwrap(), vec![osaka]);
}

#[test]
fn rejects_subscription_nodes_and_reports_corrupt_rows() {
    let database = TestDatabase::new("corrupt-row");
    let mut subscribed = node("Subscription", 443);
    subscribed.subscription_id = Some(uuid());

    let mut store = SqliteManualNodeStore::open(database.path()).unwrap();
    assert!(matches!(
        store.save_and_select(&subscribed),
        Err(ManualNodeStoreError::SubscriptionNode { id }) if id == subscribed.id
    ));

    let manual = node("Manual", 443);
    store.save_and_select(&manual).unwrap();
    drop(store);

    let connection = rusqlite::Connection::open(database.path()).unwrap();
    connection
        .execute("UPDATE manual_nodes SET node_json = '{}'", [])
        .unwrap();
    drop(connection);

    let store = SqliteManualNodeStore::open(database.path()).unwrap();
    assert!(matches!(
        store.nodes(),
        Err(ManualNodeStoreError::DeserializeNode { .. })
    ));
}

#[test]
fn records_latency_without_changing_the_selected_node() {
    let mut store = SqliteManualNodeStore::open_in_memory().unwrap();
    let tokyo = node("Tokyo", 8_388);
    let osaka = node("Osaka", 9_000);
    store.save_and_select(&tokyo).unwrap();
    store.save_and_select(&osaka).unwrap();

    let tested = store
        .update_latency(tokyo.id, Some(42), TimestampMillis::new(100))
        .unwrap();

    assert_eq!(tested.latency_ms, Some(42));
    assert_eq!(tested.last_tested_at, Some(TimestampMillis::new(100)));
    assert_eq!(store.selected_node().unwrap(), Some(osaka));
    assert!(matches!(
        store.update_latency(Uuid::nil(), None, TimestampMillis::new(200)),
        Err(ManualNodeStoreError::NodeNotFound { .. })
    ));
}

#[test]
fn updates_an_existing_manual_node_without_changing_the_selection() {
    let mut store = SqliteManualNodeStore::open_in_memory().unwrap();
    let tokyo = node("Tokyo", 8_388);
    let osaka = node("Osaka", 9_000);
    store.save_and_select(&tokyo).unwrap();
    store.save_and_select(&osaka).unwrap();

    let mut edited = tokyo.clone();
    edited.name = NodeName::new("Tokyo 2").unwrap();
    edited.server = ServerAddress::new("new.example.com").unwrap();
    edited.port = NonZeroU16::new(443).unwrap();

    assert_eq!(store.update(&edited).unwrap(), edited);
    assert_eq!(store.nodes().unwrap(), vec![edited, osaka.clone()]);
    assert_eq!(store.selected_node().unwrap(), Some(osaka));

    let missing = node("Missing", 443);
    assert!(matches!(
        store.update(&missing),
        Err(ManualNodeStoreError::NodeNotFound { id }) if id == missing.id
    ));
}

fn node(name: &str, port: u32) -> ProxyNode {
    let id = uuid();
    ProxyNode::new(
        id,
        name,
        ProxyProtocol::Shadowsocks,
        "edge.example.com",
        port,
        Some(CredentialRef::new(format!("node/{id}")).unwrap()),
    )
    .unwrap()
}

static DATABASE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);
static UUID_SEQUENCE: AtomicUsize = AtomicUsize::new(1);

fn uuid() -> Uuid {
    Uuid::from_u128(UUID_SEQUENCE.fetch_add(1, Ordering::Relaxed) as u128)
}

struct TestDatabase(PathBuf);

impl TestDatabase {
    fn new(name: &str) -> Self {
        let sequence = DATABASE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "mgclash-manual-nodes-{name}-{}-{sequence}.sqlite",
            id()
        )))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.0)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("failed to remove {}: {error}", self.0.display());
        }
    }
}
