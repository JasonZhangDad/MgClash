use base64::{Engine as _, engine::general_purpose};
use magies_domain::{CredentialRef, GrpcMode, ProxyProtocol, TlsConfig, TransportConfig};
use magies_profiles::{VmessParseError, VmessParser, VmessSecurity};
use serde_json::{Value, json};
use uuid::Uuid;

const USER_ID: &str = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";
const NODE_ID: &str = "018f78b5-08ee-7caa-94f3-1d5d781aba22";

fn legacy_uri(value: &Value) -> String {
    format!(
        "vmess://{}",
        general_purpose::STANDARD.encode(serde_json::to_vec(value).unwrap())
    )
}

fn legacy_url_safe_uri(value: &Value) -> String {
    format!(
        "vmess://{}",
        general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(value).unwrap())
    )
}

fn default_legacy_node() -> Value {
    json!({
        "v": "2",
        "ps": "Legacy WS",
        "add": "edge.example.com",
        "port": "443",
        "id": USER_ID,
        "aid": "0",
        "scy": "auto",
        "net": "ws",
        "type": "none",
        "host": "cdn.example.com",
        "path": "/proxy",
        "tls": "tls",
        "sni": "www.example.com",
        "alpn": "h2,http/1.1",
        "fp": "chrome",
        "insecure": "1",
        "vcn": "",
        "pcs": ""
    })
}

#[test]
fn recognizes_only_the_exact_vmess_scheme() {
    let parser = VmessParser;

    assert!(parser.can_parse(&format!("  vmess://{USER_ID}@example.com:443  ")));
    assert!(!parser.can_parse(&format!("vless://{USER_ID}@example.com:443")));
    assert!(!parser.can_parse(&format!("VMESS://{USER_ID}@example.com:443")));
}

#[test]
fn parses_aead_tcp_defaults_and_materializes_after_secret_storage() {
    let parsed = VmessParser
        .parse(&format!("vmess://{USER_ID}@edge.example.com:443#US%20West"))
        .unwrap();

    assert_eq!(parsed.name().as_str(), "US West");
    assert_eq!(parsed.server().as_str(), "edge.example.com");
    assert_eq!(parsed.port().get(), 443);
    assert_eq!(parsed.transport(), &TransportConfig::Tcp);
    assert_eq!(parsed.tls(), None);
    assert_eq!(
        parsed.credential().user_id(),
        Uuid::parse_str(USER_ID).unwrap()
    );
    assert_eq!(parsed.credential().security(), VmessSecurity::Auto);
    assert_eq!(parsed.credential().alter_id(), 0);

    let credential_ref = CredentialRef::new("keychain://nodes/vmess-user").unwrap();
    let node = parsed
        .into_proxy_node(Uuid::parse_str(NODE_ID).unwrap(), credential_ref)
        .unwrap();

    assert_eq!(node.protocol_type, ProxyProtocol::Vmess);
    assert_eq!(node.name.as_str(), "US West");
    assert_eq!(node.server.as_str(), "edge.example.com");
    assert_eq!(node.transport, Some(TransportConfig::Tcp));
    assert_eq!(node.credential_ref.as_str(), "keychain://nodes/vmess-user");
}

#[test]
fn parses_aead_websocket_tls_and_redacts_credentials() {
    let parsed = VmessParser
        .parse(&format!(
            "vmess://{USER_ID}@edge.example.com:443?type=ws&security=tls\
             &encryption=none&host=cdn.example.com&path=%2Fproxy%20path\
             &sni=www.example.com&fp=chrome&alpn=h2%2Chttp%2F1.1#WS"
        ))
        .unwrap();

    assert_eq!(parsed.credential().security(), VmessSecurity::None);
    assert_eq!(
        parsed.transport(),
        &TransportConfig::WebSocket {
            path: "/proxy path".to_owned(),
            host: Some("cdn.example.com".to_owned()),
        }
    );
    assert_eq!(
        parsed.tls(),
        Some(&TlsConfig::Tls {
            server_name: Some("www.example.com".to_owned()),
            allow_insecure: false,
            alpn: vec!["h2".to_owned(), "http/1.1".to_owned()],
            fingerprint: Some("chrome".to_owned()),
            pinned_sha256: None,
        })
    );

    let debug_output = format!("{parsed:?}");
    assert!(debug_output.contains("[REDACTED]"));
    assert!(!debug_output.contains(USER_ID));
}

#[test]
fn parses_aead_ipv6_grpc_reality() {
    let parsed = VmessParser
        .parse(&format!(
            "vmess://{USER_ID}@[2001:db8::1]:8443?type=grpc&serviceName=api%2Ev1\
             &mode=guna&authority=edge.example.com&security=reality\
             &encryption=chacha20-poly1305&sni=www.example.com&fp=chrome\
             &pbk=example-public-key&sid=&spx=%2Fprobe#Reality"
        ))
        .unwrap();

    assert_eq!(parsed.server().as_str(), "2001:db8::1");
    assert_eq!(
        parsed.credential().security(),
        VmessSecurity::Chacha20Poly1305
    );
    assert_eq!(
        parsed.transport(),
        &TransportConfig::Grpc {
            service_name: "api.v1".to_owned(),
            mode: GrpcMode::Guna,
            authority: Some("edge.example.com".to_owned()),
        }
    );
    assert!(matches!(
        parsed.tls(),
        Some(TlsConfig::Reality { server_name, .. }) if server_name == "www.example.com"
    ));
}

#[test]
fn parses_v2rayn_legacy_json_with_string_fields() {
    let parsed = VmessParser
        .parse(&legacy_uri(&default_legacy_node()))
        .unwrap();

    assert_eq!(parsed.name().as_str(), "Legacy WS");
    assert_eq!(parsed.server().as_str(), "edge.example.com");
    assert_eq!(parsed.port().get(), 443);
    assert_eq!(parsed.credential().security(), VmessSecurity::Auto);
    assert_eq!(parsed.credential().alter_id(), 0);
    assert_eq!(
        parsed.transport(),
        &TransportConfig::WebSocket {
            path: "/proxy".to_owned(),
            host: Some("cdn.example.com".to_owned()),
        }
    );
    assert_eq!(
        parsed.tls(),
        Some(&TlsConfig::Tls {
            server_name: Some("www.example.com".to_owned()),
            allow_insecure: true,
            alpn: vec!["h2".to_owned(), "http/1.1".to_owned()],
            fingerprint: Some("chrome".to_owned()),
            pinned_sha256: None,
        })
    );
}

#[test]
fn parses_url_safe_legacy_json_with_numeric_fields_and_grpc_defaults() {
    let value = json!({
        "v": 2,
        "ps": "",
        "add": "2001:db8::1",
        "port": 8443,
        "id": USER_ID,
        "aid": 12,
        "scy": "zero",
        "net": "grpc",
        "type": "none",
        "host": "",
        "path": "api.v1",
        "tls": "none",
        "sni": "",
        "alpn": "",
        "fp": "",
        "insecure": 0,
        "vcn": "",
        "pcs": ""
    });
    let parsed = VmessParser.parse(&legacy_url_safe_uri(&value)).unwrap();

    assert_eq!(parsed.name().as_str(), "[2001:db8::1]:8443");
    assert_eq!(parsed.credential().security(), VmessSecurity::Zero);
    assert_eq!(parsed.credential().alter_id(), 12);
    assert_eq!(parsed.tls(), None);
    assert_eq!(
        parsed.transport(),
        &TransportConfig::Grpc {
            service_name: "api.v1".to_owned(),
            mode: GrpcMode::Gun,
            authority: None,
        }
    );
}

#[test]
fn applies_legacy_defaults_when_optional_fields_are_missing() {
    let value = json!({
        "add": "edge.example.com",
        "port": 443,
        "id": USER_ID,
        "tls": "tls"
    });
    let parsed = VmessParser.parse(&legacy_uri(&value)).unwrap();

    assert_eq!(parsed.name().as_str(), "edge.example.com:443");
    assert_eq!(parsed.credential().security(), VmessSecurity::Auto);
    assert_eq!(parsed.credential().alter_id(), 0);
    assert_eq!(parsed.transport(), &TransportConfig::Tcp);
    assert_eq!(
        parsed.tls(),
        Some(&TlsConfig::Tls {
            server_name: None,
            allow_insecure: false,
            alpn: Vec::new(),
            fingerprint: None,
            pinned_sha256: None,
        })
    );
}

#[test]
fn rejects_invalid_aead_fields_without_leaking_the_user_id() {
    let invalid_encryption = VmessParser
        .parse(&format!(
            "vmess://{USER_ID}@example.com:443?encryption=invalid"
        ))
        .unwrap_err();
    let vless_only_flow = VmessParser
        .parse(&format!(
            "vmess://{USER_ID}@example.com:443?flow=xtls-rprx-vision"
        ))
        .unwrap_err();

    assert!(matches!(
        &invalid_encryption,
        VmessParseError::UnsupportedEncryption { value } if value == "invalid"
    ));
    assert!(matches!(
        &vless_only_flow,
        VmessParseError::UnsupportedParameter { name } if name == "flow"
    ));
    assert!(!format!("{invalid_encryption:?}").contains(USER_ID));
    assert!(!format!("{vless_only_flow:?}").contains(USER_ID));
}

#[test]
fn rejects_malformed_legacy_payloads_and_required_fields() {
    assert!(matches!(
        VmessParser.parse("vless://not-vmess").unwrap_err(),
        VmessParseError::UnsupportedScheme
    ));
    assert!(matches!(
        VmessParser.parse("vmess://").unwrap_err(),
        VmessParseError::EmptyPayload
    ));
    assert!(matches!(
        VmessParser.parse("vmess://not+base64!").unwrap_err(),
        VmessParseError::InvalidBase64 { .. }
    ));

    let invalid_json = format!("vmess://{}", general_purpose::STANDARD.encode(b"not json"));
    assert!(matches!(
        VmessParser.parse(&invalid_json).unwrap_err(),
        VmessParseError::InvalidJson { .. }
    ));

    let missing_server = json!({"v": "2", "port": "443", "id": USER_ID});
    assert!(matches!(
        VmessParser.parse(&legacy_uri(&missing_server)).unwrap_err(),
        VmessParseError::MissingField { name: "add" }
    ));
}

#[test]
fn rejects_legacy_values_that_cannot_be_represented_losslessly() {
    let cases = [
        (
            "unsupported version",
            json!({"v": "1", "add": "example.com", "port": 443, "id": USER_ID}),
        ),
        (
            "unsupported transport",
            json!({"v": "2", "add": "example.com", "port": 443, "id": USER_ID, "net": "kcp"}),
        ),
        (
            "unsupported tcp header",
            json!({"v": "2", "add": "example.com", "port": 443, "id": USER_ID, "net": "tcp", "type": "http"}),
        ),
        (
            "unsupported certificate pin",
            json!({"v": "2", "add": "example.com", "port": 443, "id": USER_ID, "tls": "tls", "pcs": "sha256"}),
        ),
        (
            "invalid ALPN",
            json!({"v": "2", "add": "example.com", "port": 443, "id": USER_ID, "tls": "tls", "alpn": "h2,,http/1.1"}),
        ),
    ];

    let errors = cases.map(|(_, value)| VmessParser.parse(&legacy_uri(&value)).unwrap_err());

    assert!(matches!(
        &errors[0],
        VmessParseError::UnsupportedLegacyVersion { value } if value == "1"
    ));
    assert!(matches!(
        &errors[1],
        VmessParseError::UnsupportedTransport { value } if value == "kcp"
    ));
    assert!(matches!(
        &errors[2],
        VmessParseError::UnsupportedTcpHeader { value } if value == "http"
    ));
    assert!(matches!(
        &errors[3],
        VmessParseError::InvalidCertificatePin { value } if value == "sha256"
    ));
    assert!(matches!(errors[4], VmessParseError::InvalidAlpn));
}

#[test]
fn validates_legacy_identity_port_security_and_schema() {
    let cases = [
        json!({"v": "2", "add": "example.com", "port": 443, "id": "not-a-uuid"}),
        json!({"v": "2", "add": "example.com", "port": 0, "id": USER_ID}),
        json!({"v": "2", "add": "example.com", "port": 443, "id": USER_ID, "scy": "invalid"}),
        json!({"v": "2", "add": "example.com", "port": 443, "id": USER_ID, "tls": "xtls"}),
        json!({"v": "2", "add": "example.com", "port": 443, "id": USER_ID, "tls": "tls", "insecure": 2}),
        json!({"v": "2", "add": "example.com", "port": 443, "id": USER_ID, "unknown": true}),
    ];
    let errors = cases.map(|value| VmessParser.parse(&legacy_uri(&value)).unwrap_err());

    assert!(matches!(errors[0], VmessParseError::InvalidUserId { .. }));
    assert!(matches!(
        errors[1],
        VmessParseError::InvalidPort { port: 0 }
    ));
    assert!(matches!(
        &errors[2],
        VmessParseError::UnsupportedEncryption { value } if value == "invalid"
    ));
    assert!(matches!(
        &errors[3],
        VmessParseError::UnsupportedSecurity { value } if value == "xtls"
    ));
    assert!(matches!(
        errors[4],
        VmessParseError::InvalidNumber { name: "insecure" }
    ));
    assert!(matches!(errors[5], VmessParseError::InvalidJson { .. }));
}
