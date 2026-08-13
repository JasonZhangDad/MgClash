use magies_domain::{
    CertificatePin, CredentialRef, GrpcMode, NodeModelError, ProxyNode, ProxyProtocol,
    TimestampMillis, TlsConfig, TransportConfig, XhttpMode,
};
use serde_json::{Value, json};
use uuid::Uuid;

const NODE_ID: &str = "018f78b5-08ee-7caa-94f3-1d5d781aba22";

fn node_id() -> Uuid {
    Uuid::parse_str(NODE_ID).unwrap()
}

fn credential_ref() -> CredentialRef {
    CredentialRef::new("keychain://nodes/018f78b5").unwrap()
}

#[test]
fn constructs_a_valid_node_with_prd_defaults() {
    let node = ProxyNode::new(
        node_id(),
        "  US West  ",
        ProxyProtocol::Vless,
        "  edge.example.com  ",
        443,
        Some(credential_ref()),
    )
    .unwrap();

    assert_eq!(node.id, node_id());
    assert_eq!(node.name.as_str(), "US West");
    assert_eq!(node.protocol_type, ProxyProtocol::Vless);
    assert_eq!(node.server.as_str(), "edge.example.com");
    assert_eq!(node.port.get(), 443);
    assert_eq!(node.credential_ref.as_str(), "keychain://nodes/018f78b5");
    assert_eq!(node.transport, None);
    assert_eq!(node.tls, None);
    assert!(node.udp_enabled);
    assert_eq!(node.subscription_id, None);
    assert_eq!(node.group_id, None);
    assert_eq!(node.latency_ms, None);
    assert_eq!(node.last_tested_at, None);
    assert!(node.enabled);
}

#[test]
fn serializes_all_p0_protocol_names_stably() {
    let cases = [
        (ProxyProtocol::Vless, "vless"),
        (ProxyProtocol::Vmess, "vmess"),
        (ProxyProtocol::Trojan, "trojan"),
        (ProxyProtocol::Shadowsocks, "shadowsocks"),
        (ProxyProtocol::Hysteria2, "hysteria2"),
        (ProxyProtocol::Tuic, "tuic"),
        (ProxyProtocol::Socks, "socks"),
        (ProxyProtocol::Http, "http"),
        (ProxyProtocol::WireGuard, "wireguard"),
        (ProxyProtocol::AnyTls, "anytls"),
        (ProxyProtocol::Naive, "naive"),
    ];

    for (protocol, expected) in cases {
        assert_eq!(serde_json::to_value(protocol).unwrap(), json!(expected));
        assert_eq!(
            serde_json::from_value::<ProxyProtocol>(json!(expected)).unwrap(),
            protocol
        );
    }
}

#[test]
fn round_trips_transport_tls_and_runtime_metadata() {
    let mut node = ProxyNode::new(
        node_id(),
        "Reality WS",
        ProxyProtocol::Vless,
        "203.0.113.8",
        8443,
        Some(credential_ref()),
    )
    .unwrap();
    node.transport = Some(TransportConfig::WebSocket {
        path: "/proxy".to_owned(),
        host: Some("cdn.example.com".to_owned()),
    });
    node.tls = Some(TlsConfig::Reality {
        server_name: "www.example.com".to_owned(),
        public_key: "example-public-key".to_owned(),
        short_id: Some("6ba85179e30d4fc2".to_owned()),
        fingerprint: Some("chrome".to_owned()),
        alpn: Vec::new(),
        spider_x: None,
    });
    node.udp_enabled = false;
    node.subscription_id = Some(Uuid::parse_str("018f78b5-2cd0-7000-a9a6-3bccf60951e8").unwrap());
    node.group_id = Some(Uuid::parse_str("018f78b5-39dd-7000-8545-94ef3316ce55").unwrap());
    node.latency_ms = Some(87);
    node.last_tested_at = Some(TimestampMillis::new(1_723_148_123_456));
    node.enabled = false;

    let encoded = serde_json::to_value(&node).unwrap();

    assert_eq!(node.last_tested_at.unwrap().get(), 1_723_148_123_456);
    assert_eq!(encoded["id"], json!(NODE_ID));
    assert_eq!(encoded["protocolType"], json!("vless"));
    assert_eq!(encoded["credentialRef"], json!("keychain://nodes/018f78b5"));
    assert_eq!(encoded["transport"]["type"], json!("websocket"));
    assert_eq!(encoded["tls"]["type"], json!("reality"));
    assert_eq!(encoded["lastTestedAt"], json!(1_723_148_123_456_i64));
    assert_eq!(serde_json::from_value::<ProxyNode>(encoded).unwrap(), node);
}

#[test]
fn supports_tcp_grpc_and_standard_tls_without_stringly_typed_kinds() {
    assert_eq!(
        serde_json::to_value(TransportConfig::Tcp).unwrap(),
        json!({ "type": "tcp" })
    );
    assert_eq!(
        serde_json::to_value(TransportConfig::Grpc {
            service_name: "api.v1".to_owned(),
            mode: GrpcMode::Gun,
            authority: None,
        })
        .unwrap(),
        json!({
            "type": "grpc",
            "serviceName": "api.v1",
            "mode": "gun",
            "authority": null
        })
    );
    assert_eq!(
        serde_json::to_value(TlsConfig::Tls {
            server_name: Some("example.com".to_owned()),
            allow_insecure: false,
            alpn: vec!["h2".to_owned(), "http/1.1".to_owned()],
            fingerprint: Some("chrome".to_owned()),
            pinned_sha256: None,
        })
        .unwrap(),
        json!({
            "type": "tls",
            "serverName": "example.com",
            "allowInsecure": false,
            "alpn": ["h2", "http/1.1"],
            "fingerprint": "chrome",
            "pinnedSha256": null
        })
    );
}

#[test]
fn round_trips_xhttp_transport_with_its_mode() {
    let mut node = ProxyNode::new(
        node_id(),
        "XHTTP Edge",
        ProxyProtocol::Vless,
        "203.0.113.9",
        443,
        Some(credential_ref()),
    )
    .unwrap();
    node.transport = Some(TransportConfig::XHttp {
        path: "/api".to_owned(),
        host: Some("cdn.example.com".to_owned()),
        mode: XhttpMode::PacketUp,
    });

    let encoded = serde_json::to_value(&node).unwrap();

    assert_eq!(encoded["transport"]["type"], json!("xhttp"));
    assert_eq!(encoded["transport"]["mode"], json!("packet-up"));
    assert_eq!(serde_json::from_value::<ProxyNode>(encoded).unwrap(), node);
}

#[test]
fn xhttp_mode_defaults_to_auto_when_absent() {
    let stored = json!({
        "type": "xhttp",
        "path": "/api",
        "host": null,
    });

    let transport: TransportConfig = serde_json::from_value(stored).unwrap();

    assert_eq!(
        transport,
        TransportConfig::XHttp {
            path: "/api".to_owned(),
            host: None,
            mode: XhttpMode::Auto,
        }
    );
}

#[test]
fn rejects_invalid_required_fields() {
    for name in ["", "  \n"] {
        assert_eq!(
            ProxyNode::new(
                node_id(),
                name,
                ProxyProtocol::Vless,
                "example.com",
                443,
                Some(credential_ref()),
            )
            .unwrap_err(),
            NodeModelError::EmptyName
        );
    }

    for server in ["", " \t "] {
        assert_eq!(
            ProxyNode::new(
                node_id(),
                "Node",
                ProxyProtocol::Vless,
                server,
                443,
                Some(credential_ref()),
            )
            .unwrap_err(),
            NodeModelError::EmptyServer
        );
    }

    for port in [0, 65_536] {
        assert_eq!(
            ProxyNode::new(
                node_id(),
                "Node",
                ProxyProtocol::Vless,
                "example.com",
                port,
                Some(credential_ref()),
            )
            .unwrap_err(),
            NodeModelError::InvalidPort { port }
        );
    }

    assert_eq!(
        ProxyNode::new(
            node_id(),
            "Node",
            ProxyProtocol::Vless,
            "example.com",
            443,
            None,
        )
        .unwrap_err(),
        NodeModelError::MissingCredentialReference
    );
    assert_eq!(
        CredentialRef::new("  ").unwrap_err(),
        NodeModelError::EmptyCredentialReference
    );
    assert_eq!(
        CredentialRef::new("  keychain://nodes/trimmed  ")
            .unwrap()
            .as_str(),
        "keychain://nodes/trimmed"
    );
}

#[test]
fn deserialization_cannot_bypass_required_field_validation() {
    let valid = serde_json::to_value(
        ProxyNode::new(
            node_id(),
            "Node",
            ProxyProtocol::Vless,
            "example.com",
            443,
            Some(credential_ref()),
        )
        .unwrap(),
    )
    .unwrap();

    for (field, invalid_value) in [
        ("name", Value::String("  ".to_owned())),
        ("server", Value::String(String::new())),
        ("credentialRef", Value::String("\n".to_owned())),
        ("port", json!(0)),
    ] {
        let mut invalid = valid.clone();
        invalid[field] = invalid_value;
        assert!(serde_json::from_value::<ProxyNode>(invalid).is_err());
    }
}

#[test]
fn debug_output_redacts_the_credential_reference() {
    let reference = credential_ref();
    let node = ProxyNode::new(
        node_id(),
        "Node",
        ProxyProtocol::Vless,
        "example.com",
        443,
        Some(reference.clone()),
    )
    .unwrap();

    for debug_output in [format!("{reference:?}"), format!("{node:?}")] {
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains(reference.as_str()));
    }
}

#[test]
fn a_certificate_pin_normalizes_the_share_link_spelling() {
    // Share links write the digest the way OpenSSL prints it.
    let pin = CertificatePin::new(
        "6F:F2:12:BB:AB:49:0B:68:6B:06:20:9C:60:74:86:5F:93:40:F4:C0:F9:C4:AA:7D:34:D5:68:C2:A2:CE:BE:73",
    )
    .unwrap();

    assert_eq!(
        pin.as_str(),
        "6ff212bbab490b686b06209c6074865f9340f4c0f9c4aa7d34d568c2a2cebe73"
    );
}

#[test]
fn a_certificate_pin_accepts_bare_hex() {
    let pin =
        CertificatePin::new("6ff212bbab490b686b06209c6074865f9340f4c0f9c4aa7d34d568c2a2cebe73")
            .unwrap();

    assert_eq!(pin.as_str().len(), 64);
}

#[test]
fn a_certificate_pin_rejects_anything_that_is_not_32_hex_bytes() {
    for value in [
        "",
        "abc123",
        // Base64 of the same digest: a real encoding, but not this one.
        "b/ISu6tJC2hrBiCcYHSGX5NA9MD5xKp9NNVowqLOvnM=",
        // 31 bytes.
        "6ff212bbab490b686b06209c6074865f9340f4c0f9c4aa7d34d568c2a2cebe",
        // Right length, one non-hex digit.
        "6ff212bbab490b686b06209c6074865f9340f4c0f9c4aa7d34d568c2a2cebeZZ",
    ] {
        assert!(
            matches!(
                CertificatePin::new(value),
                Err(NodeModelError::InvalidCertificatePin)
            ),
            "{value:?} must not parse as a certificate pin"
        );
    }
}

#[test]
fn a_node_stored_before_pinning_existed_still_deserializes() {
    let stored = serde_json::json!({
        "type": "tls",
        "serverName": "example.com",
        "allowInsecure": false,
        "alpn": [],
        "fingerprint": null,
    });

    let tls: TlsConfig = serde_json::from_value(stored).unwrap();

    assert!(matches!(
        tls,
        TlsConfig::Tls {
            pinned_sha256: None,
            ..
        }
    ));
}
