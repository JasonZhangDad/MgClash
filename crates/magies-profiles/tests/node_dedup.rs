use std::collections::HashSet;

use magies_domain::{
    CredentialRef, GrpcMode, NodeName, ProxyNode, ProxyProtocol, TimestampMillis, TlsConfig,
    TransportConfig,
};
use magies_profiles::{CredentialIdentity, NodeDedupCandidate, NodeDeduplicator};
use uuid::Uuid;

#[test]
fn credential_identity_is_stable_and_redacts_credential_material() {
    let identity = CredentialIdentity::from_bytes(b"top-secret-password");

    assert_eq!(
        identity,
        CredentialIdentity::from_bytes(b"top-secret-password")
    );
    assert_ne!(identity, CredentialIdentity::from_bytes(b"other-password"));
    assert!(!format!("{identity:?}").contains("top-secret-password"));
}

#[test]
fn fingerprint_uses_only_the_prd_identity_fields() {
    let identity = CredentialIdentity::from_bytes(b"credential-a");
    let mut baseline = node(
        "018f78b5-2cd0-7000-a9a6-3bccf6095101",
        "Subscription name",
        ProxyProtocol::Vless,
        "edge.example.com",
        443,
        "keychain://new-reference",
    );
    baseline.transport = Some(TransportConfig::WebSocket {
        path: "/socket".to_owned(),
        host: Some("cdn.example.com".to_owned()),
    });
    baseline.tls = Some(TlsConfig::Tls {
        server_name: Some("tls.example.com".to_owned()),
        allow_insecure: false,
        alpn: vec!["h2".to_owned()],
        fingerprint: Some("chrome".to_owned()),
        pinned_sha256: None,
    });
    let baseline_fingerprint = candidate(baseline.clone(), identity).fingerprint();

    let mut local_fields_changed = baseline.clone();
    local_fields_changed.id = uuid("018f78b5-2cd0-7000-a9a6-3bccf6095102");
    local_fields_changed.name = NodeName::new("Local remark").unwrap();
    local_fields_changed.udp_enabled = false;
    local_fields_changed.subscription_id = Some(uuid("018f78b5-2cd0-7000-a9a6-3bccf6095103"));
    local_fields_changed.group_id = Some(uuid("018f78b5-2cd0-7000-a9a6-3bccf6095104"));
    local_fields_changed.latency_ms = Some(23);
    local_fields_changed.last_tested_at = Some(TimestampMillis::new(99));
    local_fields_changed.enabled = false;
    assert_eq!(
        candidate(local_fields_changed, identity).fingerprint(),
        baseline_fingerprint
    );

    let mut changes = Vec::new();
    let mut protocol_changed = baseline.clone();
    protocol_changed.protocol_type = ProxyProtocol::Vmess;
    changes.push(candidate(protocol_changed, identity));
    let mut server_changed = baseline.clone();
    server_changed.server = magies_domain::ServerAddress::new("other.example.com").unwrap();
    changes.push(candidate(server_changed, identity));
    let mut port_changed = baseline.clone();
    port_changed.port = std::num::NonZeroU16::new(8443).unwrap();
    changes.push(candidate(port_changed, identity));
    changes.push(candidate(
        baseline.clone(),
        CredentialIdentity::from_bytes(b"credential-b"),
    ));
    let mut transport_changed = baseline.clone();
    transport_changed.transport = Some(TransportConfig::Tcp);
    changes.push(candidate(transport_changed, identity));
    let mut tls_changed = baseline;
    tls_changed.tls = None;
    changes.push(candidate(tls_changed, identity));

    for changed in changes {
        assert_ne!(changed.fingerprint(), baseline_fingerprint);
    }
}

#[test]
fn fingerprint_encodes_every_supported_protocol_transport_and_tls_variant() {
    let protocols = [
        ProxyProtocol::Vless,
        ProxyProtocol::Vmess,
        ProxyProtocol::Trojan,
        ProxyProtocol::Shadowsocks,
        ProxyProtocol::Hysteria2,
    ];
    let transports = [
        None,
        Some(TransportConfig::Tcp),
        Some(TransportConfig::WebSocket {
            path: "/ws".to_owned(),
            host: None,
        }),
        Some(TransportConfig::Grpc {
            service_name: "gun".to_owned(),
            mode: GrpcMode::Gun,
            authority: None,
        }),
        Some(TransportConfig::Grpc {
            service_name: "multi".to_owned(),
            mode: GrpcMode::Multi,
            authority: Some("authority.example.com".to_owned()),
        }),
        Some(TransportConfig::Grpc {
            service_name: "guna".to_owned(),
            mode: GrpcMode::Guna,
            authority: None,
        }),
    ];
    let tls_options = [
        None,
        Some(TlsConfig::Tls {
            server_name: None,
            allow_insecure: true,
            alpn: Vec::new(),
            fingerprint: None,
            pinned_sha256: None,
        }),
        Some(TlsConfig::Reality {
            server_name: "reality.example.com".to_owned(),
            public_key: "public-key".to_owned(),
            short_id: Some("0123abcd".to_owned()),
            fingerprint: Some("firefox".to_owned()),
            alpn: vec!["h2".to_owned(), "http/1.1".to_owned()],
            spider_x: Some("/index".to_owned()),
        }),
    ];
    let identity = CredentialIdentity::from_bytes(b"one-account");
    let mut fingerprints = HashSet::new();

    for protocol in protocols {
        for transport in &transports {
            for tls in &tls_options {
                let mut value = node(
                    "018f78b5-2cd0-7000-a9a6-3bccf6095105",
                    "Coverage",
                    protocol,
                    "coverage.example.com",
                    443,
                    "keychain://coverage",
                );
                value.transport.clone_from(transport);
                value.tls.clone_from(tls);
                fingerprints.insert(candidate(value, identity).fingerprint());
            }
        }
    }

    assert_eq!(fingerprints.len(), 90);
}

#[test]
fn matching_node_keeps_local_state_and_accepts_new_subscription_properties() {
    let subscription_id = uuid("018f78b5-2cd0-7000-a9a6-3bccf6095110");
    let group_id = uuid("018f78b5-2cd0-7000-a9a6-3bccf6095111");
    let identity = CredentialIdentity::from_bytes(b"same-account");
    let mut existing = node(
        "018f78b5-2cd0-7000-a9a6-3bccf6095112",
        "My local remark",
        ProxyProtocol::Trojan,
        "node.example.com",
        443,
        "keychain://old-reference",
    );
    existing.subscription_id = Some(subscription_id);
    existing.group_id = Some(group_id);
    existing.latency_ms = Some(35);
    existing.last_tested_at = Some(TimestampMillis::new(100));
    existing.enabled = false;

    let mut incoming = node(
        "018f78b5-2cd0-7000-a9a6-3bccf6095113",
        "Provider name",
        ProxyProtocol::Trojan,
        "node.example.com",
        443,
        "keychain://rotated-reference",
    );
    incoming.subscription_id = Some(subscription_id);
    incoming.udp_enabled = false;

    let result = NodeDeduplicator::reconcile(
        &[candidate(existing.clone(), identity)],
        vec![candidate(incoming, identity)],
    );

    assert_eq!(result.summary.added_nodes, 0);
    assert_eq!(result.summary.updated_nodes, 1);
    assert_eq!(result.summary.removed_nodes, 0);
    assert_eq!(result.summary.deduplicated_nodes, 0);
    assert_eq!(result.nodes.len(), 1);
    let merged = &result.nodes[0];
    assert_eq!(merged.id, existing.id);
    assert_eq!(merged.name, existing.name);
    assert_eq!(merged.group_id, existing.group_id);
    assert_eq!(merged.latency_ms, existing.latency_ms);
    assert_eq!(merged.last_tested_at, existing.last_tested_at);
    assert_eq!(merged.enabled, existing.enabled);
    assert_eq!(
        merged.credential_ref.as_str(),
        "keychain://rotated-reference"
    );
    assert!(!merged.udp_enabled);
}

#[test]
fn reconcile_adds_removes_and_collapses_duplicate_nodes_deterministically() {
    let identity = CredentialIdentity::from_bytes(b"shared-account");
    let higher_id = node(
        "018f78b5-2cd0-7000-a9a6-3bccf6095122",
        "Higher ID",
        ProxyProtocol::Shadowsocks,
        "duplicate.example.com",
        8388,
        "keychain://old-a",
    );
    let lower_id = node(
        "018f78b5-2cd0-7000-a9a6-3bccf6095121",
        "Lower ID wins",
        ProxyProtocol::Shadowsocks,
        "duplicate.example.com",
        8388,
        "keychain://old-b",
    );
    let stale = node(
        "018f78b5-2cd0-7000-a9a6-3bccf6095123",
        "Stale",
        ProxyProtocol::Vless,
        "stale.example.com",
        443,
        "keychain://stale",
    );
    let replacement = node(
        "018f78b5-2cd0-7000-a9a6-3bccf6095124",
        "Provider duplicate",
        ProxyProtocol::Shadowsocks,
        "duplicate.example.com",
        8388,
        "keychain://new",
    );
    let added = node(
        "018f78b5-2cd0-7000-a9a6-3bccf6095125",
        "New first occurrence",
        ProxyProtocol::Hysteria2,
        "new.example.com",
        8443,
        "keychain://new-node-a",
    );
    let duplicate_added = node(
        "018f78b5-2cd0-7000-a9a6-3bccf6095126",
        "New repeated occurrence",
        ProxyProtocol::Hysteria2,
        "new.example.com",
        8443,
        "keychain://new-node-b",
    );

    let result = NodeDeduplicator::reconcile(
        &[
            candidate(higher_id, identity),
            candidate(lower_id.clone(), identity),
            candidate(stale, CredentialIdentity::from_bytes(b"stale-account")),
        ],
        vec![
            candidate(replacement, identity),
            candidate(
                added.clone(),
                CredentialIdentity::from_bytes(b"new-account"),
            ),
            candidate(
                duplicate_added,
                CredentialIdentity::from_bytes(b"new-account"),
            ),
        ],
    );

    assert_eq!(result.summary.added_nodes, 1);
    assert_eq!(result.summary.updated_nodes, 1);
    assert_eq!(result.summary.removed_nodes, 2);
    assert_eq!(result.summary.deduplicated_nodes, 1);
    assert_eq!(result.nodes.len(), 2);
    assert_eq!(result.nodes[0].id, lower_id.id);
    assert_eq!(result.nodes[0].name, lower_id.name);
    assert_eq!(result.nodes[1].id, added.id);
    assert_eq!(result.nodes[1].name, added.name);
}

fn candidate(node: ProxyNode, identity: CredentialIdentity) -> NodeDedupCandidate {
    NodeDedupCandidate::new(node, identity)
}

fn node(
    id: &str,
    name: &str,
    protocol: ProxyProtocol,
    server: &str,
    port: u32,
    credential_ref: &str,
) -> ProxyNode {
    ProxyNode::new(
        uuid(id),
        name,
        protocol,
        server,
        port,
        Some(CredentialRef::new(credential_ref).unwrap()),
    )
    .unwrap()
}

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap()
}
