use std::sync::atomic::{AtomicU64, Ordering};

use magies_domain::{
    CredentialRef, GrpcMode, ProxyNode, ProxyProtocol, Subscription, SubscriptionName,
    TimestampMillis, TlsConfig, TransportConfig,
};
use magies_profiles::{
    SqliteSubscriptionStore, SubscriptionTransactionError, SubscriptionUpdate,
    SubscriptionValidators,
};
use uuid::Uuid;

const LAST_MODIFIED: &str = "Sun, 09 Aug 2026 12:00:00 GMT";
static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn lists_reads_and_edits_subscription_settings_without_losing_fetch_state() {
    let store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let mut primary = subscription("018f78b5-2cd0-7000-a9a6-3bccf60951e8", "Primary");
    primary.auto_update = true;
    let backup = subscription("018f78b5-2cd0-7000-a9a6-3bccf60951ef", "Backup");
    store.insert_subscription(&primary).unwrap();
    store.insert_subscription(&backup).unwrap();
    store
        .touch_subscription(
            primary.id,
            &SubscriptionValidators::new(Some("\"revision-1\"".to_owned()), None),
            TimestampMillis::new(100),
        )
        .unwrap();

    let subscriptions = store.subscriptions().unwrap();
    assert_eq!(subscriptions.len(), 2);
    assert_eq!(subscriptions[0].id, primary.id);
    assert_eq!(subscriptions[1].id, backup.id);

    let mut edited = store.subscription(primary.id).unwrap().unwrap();
    edited.name = SubscriptionName::new("Primary edited").unwrap();
    edited.update_interval_minutes = std::num::NonZeroU32::new(120).unwrap();
    edited.auto_update = false;
    edited.enabled = false;
    store.update_subscription_settings(&edited).unwrap();

    let loaded = store.subscription(primary.id).unwrap().unwrap();
    assert_eq!(loaded.name.as_str(), "Primary edited");
    assert_eq!(loaded.update_interval_minutes.get(), 120);
    assert!(!loaded.auto_update);
    assert!(!loaded.enabled);
    assert_eq!(loaded.etag.as_deref(), Some("\"revision-1\""));
    assert_eq!(loaded.last_updated_at, Some(TimestampMillis::new(100)));
    assert!(store.subscription(Uuid::nil()).unwrap().is_none());
    assert!(matches!(
        store.touch_subscription(
            Uuid::nil(),
            &SubscriptionValidators::default(),
            TimestampMillis::new(200),
        ),
        Err(SubscriptionTransactionError::SubscriptionNotFound { .. })
    ));
    edited.id = Uuid::nil();
    assert!(matches!(
        store.update_subscription_settings(&edited),
        Err(SubscriptionTransactionError::SubscriptionNotFound { .. })
    ));
}

#[test]
fn deletes_a_subscription_and_returns_its_cascaded_nodes() {
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let subscription = subscription("018f78b5-2cd0-7000-a9a6-3bccf60951e8", "Delete me");
    store.insert_subscription(&subscription).unwrap();
    store
        .apply_update(&update(
            subscription.id,
            vec![node(
                "018f78b5-2cd0-7000-a9a6-3bccf60951e9",
                ProxyProtocol::Trojan,
                "delete.example.com",
                "keychain://node/delete-secret",
            )],
            "\"revision-1\"",
            100,
        ))
        .unwrap();

    let deleted = store.delete_subscription(subscription.id).unwrap();

    assert_eq!(deleted.subscription, subscription_with_state(subscription));
    assert_eq!(deleted.nodes.len(), 1);
    assert_eq!(deleted.nodes[0].server.as_str(), "delete.example.com");
    assert!(
        store
            .subscription(subscription_id(&deleted))
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .subscription_nodes(subscription_id(&deleted))
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        store.delete_subscription(subscription_id(&deleted)),
        Err(SubscriptionTransactionError::SubscriptionNotFound { .. })
    ));
}

fn subscription_id(deleted: &magies_profiles::DeletedSubscription) -> Uuid {
    deleted.subscription.id
}

fn subscription_with_state(mut subscription: Subscription) -> Subscription {
    subscription.etag = Some("\"revision-1\"".to_owned());
    subscription.last_modified = Some(LAST_MODIFIED.to_owned());
    subscription.last_updated_at = Some(TimestampMillis::new(100));
    subscription
}

#[test]
fn persists_a_committed_update_when_the_database_is_reopened() {
    let database_id = NEXT_DATABASE_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "mgclash-subscription-{}-{database_id}.sqlite",
        std::process::id()
    ));
    let subscription = subscription("018f78b5-2cd0-7000-a9a6-3bccf60951e8", "Persistent");
    let persisted_node = node(
        "018f78b5-2cd0-7000-a9a6-3bccf60951e9",
        ProxyProtocol::Vless,
        "persisted.example.com",
        "keychain://node/persisted-secret",
    );
    {
        let mut store = SqliteSubscriptionStore::open(&path).unwrap();
        store.insert_subscription(&subscription).unwrap();
        store
            .apply_update(&update(
                subscription.id,
                vec![persisted_node.clone()],
                "\"disk\"",
                100,
            ))
            .unwrap();
        store.select_node(persisted_node.id).unwrap();
    }

    let reopened = SqliteSubscriptionStore::open(&path).unwrap();
    assert_eq!(
        reopened.subscription_nodes(subscription.id).unwrap().len(),
        1
    );
    assert_eq!(
        reopened
            .subscription_state(subscription.id)
            .unwrap()
            .unwrap()
            .validators
            .etag(),
        Some("\"disk\"")
    );
    let mut expected_selected = persisted_node;
    expected_selected.subscription_id = Some(subscription.id);
    assert_eq!(reopened.selected_node().unwrap(), Some(expected_selected));
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn lists_and_selects_only_enabled_subscription_nodes() {
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let active = subscription("018f78b5-2cd0-7000-a9a6-3bccf60951e8", "Active");
    let mut disabled = subscription("018f78b5-2cd0-7000-a9a6-3bccf60951ef", "Disabled");
    disabled.enabled = false;
    store.insert_subscription(&active).unwrap();
    store.insert_subscription(&disabled).unwrap();

    let active_node = node(
        "018f78b5-2cd0-7000-a9a6-3bccf60951e9",
        ProxyProtocol::Vless,
        "active.example.com",
        "keychain://node/active-secret",
    );
    let mut disabled_node = node(
        "018f78b5-2cd0-7000-a9a6-3bccf60951ea",
        ProxyProtocol::Trojan,
        "disabled-node.example.com",
        "keychain://node/disabled-secret",
    );
    disabled_node.enabled = false;
    let disabled_subscription_node = node(
        "018f78b5-2cd0-7000-a9a6-3bccf60951eb",
        ProxyProtocol::Vmess,
        "disabled-subscription.example.com",
        "keychain://node/disabled-subscription-secret",
    );
    store
        .apply_update(&update(
            active.id,
            vec![active_node.clone(), disabled_node.clone()],
            "\"active\"",
            100,
        ))
        .unwrap();
    store
        .apply_update(&update(
            disabled.id,
            vec![disabled_subscription_node.clone()],
            "\"disabled\"",
            100,
        ))
        .unwrap();

    let mut expected_active = active_node.clone();
    expected_active.subscription_id = Some(active.id);
    assert_eq!(store.active_nodes().unwrap(), vec![expected_active.clone()]);
    assert_eq!(store.selected_node().unwrap(), None);

    assert_eq!(store.select_node(active_node.id).unwrap(), expected_active);
    assert_eq!(store.selected_node().unwrap(), Some(expected_active));
    assert!(matches!(
        store.select_node(disabled_node.id),
        Err(SubscriptionTransactionError::NodeNotFound { id }) if id == disabled_node.id
    ));
    assert!(matches!(
        store.select_node(disabled_subscription_node.id),
        Err(SubscriptionTransactionError::NodeNotFound { id })
            if id == disabled_subscription_node.id
    ));

    let mut disabled_after_selection = store.subscription(active.id).unwrap().unwrap();
    disabled_after_selection.enabled = false;
    store
        .update_subscription_settings(&disabled_after_selection)
        .unwrap();
    assert!(store.active_nodes().unwrap().is_empty());
    assert_eq!(store.selected_node().unwrap(), None);

    store.clear_selected_node().unwrap();
    assert_eq!(store.selected_node().unwrap(), None);
}

#[test]
fn atomically_replaces_nodes_and_updates_subscription_metadata() {
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let subscription = subscription("018f78b5-2cd0-7000-a9a6-3bccf60951e8", "Primary");
    store.insert_subscription(&subscription).unwrap();

    let old_node = node(
        "018f78b5-2cd0-7000-a9a6-3bccf60951e9",
        ProxyProtocol::Vless,
        "old.example.com",
        "keychain://node/old-secret",
    );
    let first_commit = store
        .apply_update(&update(
            subscription.id,
            vec![old_node],
            "\"revision-1\"",
            100,
        ))
        .unwrap();
    assert_eq!(first_commit.removed_nodes, 0);
    assert_eq!(first_commit.inserted_nodes, 1);

    let mut websocket_node = node(
        "018f78b5-2cd0-7000-a9a6-3bccf60951ea",
        ProxyProtocol::Vmess,
        "ws.example.com",
        "keychain://node/ws-secret",
    );
    websocket_node.transport = Some(TransportConfig::WebSocket {
        path: "/socket".to_owned(),
        host: Some("cdn.example.com".to_owned()),
    });
    websocket_node.tls = Some(TlsConfig::Reality {
        server_name: "www.example.com".to_owned(),
        public_key: "public-key".to_owned(),
        short_id: Some("0123abcd".to_owned()),
        fingerprint: Some("chrome".to_owned()),
        alpn: vec!["h2".to_owned()],
        spider_x: Some("/".to_owned()),
    });
    websocket_node.udp_enabled = false;
    websocket_node.group_id = Some(uuid("018f78b5-2cd0-7000-a9a6-3bccf60951ed"));
    websocket_node.latency_ms = Some(42);
    websocket_node.last_tested_at = Some(TimestampMillis::new(90));
    websocket_node.enabled = false;

    let mut grpc_node = node(
        "018f78b5-2cd0-7000-a9a6-3bccf60951eb",
        ProxyProtocol::Trojan,
        "grpc.example.com",
        "keychain://node/grpc-secret",
    );
    grpc_node.transport = Some(TransportConfig::Grpc {
        service_name: "tunnel".to_owned(),
        mode: GrpcMode::Multi,
        authority: Some("edge.example.com".to_owned()),
    });

    let mut expected_nodes = vec![websocket_node.clone(), grpc_node.clone()];
    for node in &mut expected_nodes {
        node.subscription_id = Some(subscription.id);
    }
    expected_nodes.sort_by_key(|node| node.id);

    let commit = store
        .apply_update(&update(
            subscription.id,
            vec![websocket_node, grpc_node],
            "\"revision-2\"",
            200,
        ))
        .unwrap();

    assert_eq!(commit.removed_nodes, 1);
    assert_eq!(commit.inserted_nodes, 2);
    assert_eq!(
        store.subscription_nodes(subscription.id).unwrap(),
        expected_nodes
    );
    let state = store.subscription_state(subscription.id).unwrap().unwrap();
    assert_eq!(state.validators.etag(), Some("\"revision-2\""));
    assert_eq!(state.validators.last_modified(), Some(LAST_MODIFIED));
    assert_eq!(state.last_updated_at, Some(TimestampMillis::new(200)));

    let empty_commit = store
        .apply_update(&update(subscription.id, Vec::new(), "\"revision-3\"", 300))
        .unwrap();
    assert_eq!(empty_commit.removed_nodes, 2);
    assert_eq!(empty_commit.inserted_nodes, 0);
    assert!(
        store
            .subscription_nodes(subscription.id)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn rolls_back_nodes_and_metadata_when_any_node_insert_fails() {
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let target = subscription("018f78b5-2cd0-7000-a9a6-3bccf60951e8", "Target");
    let other = subscription("018f78b5-2cd0-7000-a9a6-3bccf60951ef", "Other");
    store.insert_subscription(&target).unwrap();
    store.insert_subscription(&other).unwrap();

    let old_node = node(
        "018f78b5-2cd0-7000-a9a6-3bccf60951e9",
        ProxyProtocol::Trojan,
        "old.example.com",
        "keychain://node/old-secret",
    );
    store
        .apply_update(&update(
            target.id,
            vec![old_node.clone()],
            "\"stable\"",
            100,
        ))
        .unwrap();

    let conflicting_node = node(
        "018f78b5-2cd0-7000-a9a6-3bccf60951ec",
        ProxyProtocol::Shadowsocks,
        "other.example.com",
        "keychain://node/other-secret",
    );
    store
        .apply_update(&update(
            other.id,
            vec![conflicting_node.clone()],
            "\"other\"",
            200,
        ))
        .unwrap();

    let error = store
        .apply_update(&update(
            target.id,
            vec![
                node(
                    "018f78b5-2cd0-7000-a9a6-3bccf60951ee",
                    ProxyProtocol::Vless,
                    "new.example.com",
                    "keychain://node/new-secret",
                ),
                conflicting_node,
            ],
            "\"broken\"",
            300,
        ))
        .unwrap_err();

    assert!(matches!(
        error,
        SubscriptionTransactionError::Database { .. }
    ));
    let mut expected_old = old_node;
    expected_old.subscription_id = Some(target.id);
    assert_eq!(
        store.subscription_nodes(target.id).unwrap(),
        vec![expected_old]
    );
    let state = store.subscription_state(target.id).unwrap().unwrap();
    assert_eq!(state.validators.etag(), Some("\"stable\""));
    assert_eq!(state.last_updated_at, Some(TimestampMillis::new(100)));
    assert!(!format!("{error:?}").contains("new-secret"));
}

#[test]
fn validates_update_ownership_duplicates_and_subscription_existence() {
    let target_id = uuid("018f78b5-2cd0-7000-a9a6-3bccf60951e8");
    let other_id = uuid("018f78b5-2cd0-7000-a9a6-3bccf60951ef");
    let mut assigned_node = node(
        "018f78b5-2cd0-7000-a9a6-3bccf60951e9",
        ProxyProtocol::Vless,
        "assigned.example.com",
        "keychain://node/assigned-secret",
    );
    assigned_node.subscription_id = Some(other_id);
    let foreign = SubscriptionUpdate::new(
        target_id,
        vec![assigned_node],
        SubscriptionValidators::default(),
        TimestampMillis::new(100),
    )
    .unwrap_err();

    let duplicate_node = node(
        "018f78b5-2cd0-7000-a9a6-3bccf60951ea",
        ProxyProtocol::Vmess,
        "duplicate.example.com",
        "keychain://node/duplicate-secret",
    );
    let duplicate = SubscriptionUpdate::new(
        target_id,
        vec![duplicate_node.clone(), duplicate_node],
        SubscriptionValidators::default(),
        TimestampMillis::new(100),
    )
    .unwrap_err();

    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let missing = store
        .apply_update(&update(
            target_id,
            vec![node(
                "018f78b5-2cd0-7000-a9a6-3bccf60951eb",
                ProxyProtocol::Hysteria2,
                "missing.example.com",
                "keychain://node/missing-secret",
            )],
            "\"missing\"",
            100,
        ))
        .unwrap_err();

    assert!(matches!(
        foreign,
        SubscriptionTransactionError::NodeBelongsToAnotherSubscription { .. }
    ));
    assert!(matches!(
        duplicate,
        SubscriptionTransactionError::DuplicateNodeId { .. }
    ));
    assert!(matches!(
        missing,
        SubscriptionTransactionError::SubscriptionNotFound { id } if id == target_id
    ));
    assert!(!format!("{foreign:?}").contains("assigned-secret"));
}

fn subscription(id: &str, name: &str) -> Subscription {
    Subscription::new(
        uuid(id),
        name,
        CredentialRef::new(format!("keychain://subscription/{name}-url-secret")).unwrap(),
        60,
    )
    .unwrap()
}

fn update(
    subscription_id: Uuid,
    nodes: Vec<ProxyNode>,
    etag: &str,
    updated_at: i64,
) -> SubscriptionUpdate {
    SubscriptionUpdate::new(
        subscription_id,
        nodes,
        SubscriptionValidators::new(Some(etag.to_owned()), Some(LAST_MODIFIED.to_owned())),
        TimestampMillis::new(updated_at),
    )
    .unwrap()
}

fn node(id: &str, protocol: ProxyProtocol, server: &str, secret_ref: &str) -> ProxyNode {
    ProxyNode::new(
        uuid(id),
        format!("{protocol:?} node"),
        protocol,
        server,
        443,
        Some(CredentialRef::new(secret_ref).unwrap()),
    )
    .unwrap()
}

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap()
}
