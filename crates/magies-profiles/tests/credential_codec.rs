use magies_domain::{CredentialRef, ProxyProtocol};
use magies_profiles::{
    CredentialCodec, CredentialCodecError, Hysteria2Parser, ShadowsocksParser,
    SingBoxOutboundConfigGenerator, StoredNodeCredential, TrojanParser, VlessParser, VmessParser,
};
use magies_storage::{MemorySecretStore, SecretStore, SecretValue};
use uuid::Uuid;

const USER_ID: &str = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";

fn assert_round_trip(original: &StoredNodeCredential, expected_protocol: ProxyProtocol) {
    assert_eq!(original.protocol(), expected_protocol);
    assert_eq!(original.as_node_credential().protocol(), expected_protocol);

    let payload = CredentialCodec::encode(original).unwrap();
    let decoded = CredentialCodec::decode(&payload).unwrap();
    assert_eq!(&decoded, original);
}

#[test]
fn round_trips_all_supported_node_credentials() {
    let parsed = VlessParser
        .parse(&format!("vless://{USER_ID}@edge.example.com:443"))
        .unwrap();
    assert_round_trip(
        &StoredNodeCredential::from(parsed.credential()),
        ProxyProtocol::Vless,
    );

    let parsed = VmessParser
        .parse(&format!(
            "vmess://{USER_ID}@edge.example.com:443?encryption=none"
        ))
        .unwrap();
    assert_round_trip(
        &StoredNodeCredential::from(parsed.credential()),
        ProxyProtocol::Vmess,
    );

    let parsed = TrojanParser
        .parse("trojan://secret%3Apassword@edge.example.com:443")
        .unwrap();
    assert_round_trip(
        &StoredNodeCredential::from(parsed.credential()),
        ProxyProtocol::Trojan,
    );

    let parsed = ShadowsocksParser
        .parse("ss://aes-128-gcm:p%40ss%3Aword@edge.example.com:443")
        .unwrap();
    assert_round_trip(
        &StoredNodeCredential::from(parsed.credential()),
        ProxyProtocol::Shadowsocks,
    );

    let parsed = Hysteria2Parser
        .parse(
            "hysteria2://secret%3Atoken@edge.example.com:443?obfs=salamander\
             &obfs-password=obfs-secret",
        )
        .unwrap();
    assert_round_trip(
        &StoredNodeCredential::from(parsed.credential()),
        ProxyProtocol::Hysteria2,
    );
}

#[test]
fn rejects_malformed_and_unsupported_payload_versions() {
    let malformed = SecretValue::new(b"not-json".to_vec()).unwrap();
    assert!(matches!(
        CredentialCodec::decode(&malformed),
        Err(CredentialCodecError::InvalidPayload { .. })
    ));

    let parsed = TrojanParser
        .parse("trojan://secret-password@edge.example.com:443")
        .unwrap();
    let encoded =
        CredentialCodec::encode(&StoredNodeCredential::from(parsed.credential())).unwrap();
    let mut envelope: serde_json::Value = serde_json::from_slice(encoded.expose_secret()).unwrap();
    envelope["version"] = serde_json::json!(2);
    let unsupported = SecretValue::new(serde_json::to_vec(&envelope).unwrap()).unwrap();

    assert!(matches!(
        CredentialCodec::decode(&unsupported),
        Err(CredentialCodecError::UnsupportedVersion { version: 2 })
    ));
}

#[test]
fn credential_diagnostics_are_redacted() {
    let parsed = TrojanParser
        .parse("trojan://never-print-this@edge.example.com:443")
        .unwrap();
    let credential = StoredNodeCredential::from(parsed.credential());
    let payload = CredentialCodec::encode(&credential).unwrap();
    let diagnostics = format!("{credential:?} {payload:?}");

    assert_eq!(
        diagnostics,
        "StoredNodeCredential([REDACTED]) SecretValue([REDACTED])"
    );
    assert!(!diagnostics.contains("never-print-this"));
}

#[test]
fn restores_a_stored_credential_for_runtime_config_generation() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-128-gcm:runtime-secret@edge.example.com:443")
        .unwrap();
    let stored = StoredNodeCredential::from(parsed.credential());
    let credential_ref = CredentialRef::new("secret://nodes/runtime-test").unwrap();
    let node = parsed
        .into_proxy_node(Uuid::nil(), credential_ref.clone())
        .unwrap();
    let store = MemorySecretStore::default();
    store
        .put(&credential_ref, &CredentialCodec::encode(&stored).unwrap())
        .unwrap();

    let restored = CredentialCodec::decode(&store.get(&credential_ref).unwrap()).unwrap();
    let outbound =
        SingBoxOutboundConfigGenerator::generate(&node, restored.as_node_credential()).unwrap();

    assert_eq!(outbound.json()["type"], "shadowsocks");
    assert_eq!(outbound.json()["password"], "runtime-secret");
}
