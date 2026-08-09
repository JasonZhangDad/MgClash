use std::sync::atomic::{AtomicU64, Ordering};

use magies_domain::{
    CredentialRef, GrpcMode, ProxyNode, ProxyProtocol, Subscription, TimestampMillis, TlsConfig,
    TransportConfig,
};
use magies_profiles::{
    SqliteSubscriptionStore, SubscriptionTransactionError, SubscriptionUpdate,
    SubscriptionValidators,
};
use uuid::Uuid;

const LAST_MODIFIED: &str = "Sun, 09 Aug 2026 12:00:00 GMT";
static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(0);

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
                vec![persisted_node],
                "\"disk\"",
                100,
            ))
            .unwrap();
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
    drop(reopened);
    std::fs::remove_file(path).unwrap();
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
