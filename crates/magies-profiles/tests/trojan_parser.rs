use magies_domain::{CredentialRef, GrpcMode, ProxyProtocol, TlsConfig, TransportConfig};
use magies_profiles::{TrojanParseError, TrojanParser, VlessParseError};
use uuid::Uuid;

const PASSWORD: &str = "secret-password";
const NODE_ID: &str = "018f78b5-08ee-7caa-94f3-1d5d781aba22";

#[test]
fn recognizes_only_the_exact_trojan_scheme() {
    let parser = TrojanParser;

    assert!(parser.can_parse("  trojan://password@example.com:443  "));
    assert!(!parser.can_parse("vless://password@example.com:443"));
    assert!(!parser.can_parse("TROJAN://password@example.com:443"));
}

#[test]
fn parses_default_tcp_tls_and_materializes_after_secret_storage() {
    let parsed = TrojanParser
        .parse("trojan://secret%3Apassword@edge.example.com:443#US%20West")
        .unwrap();

    assert_eq!(parsed.name().as_str(), "US West");
    assert_eq!(parsed.server().as_str(), "edge.example.com");
    assert_eq!(parsed.port().get(), 443);
    assert_eq!(parsed.transport(), &TransportConfig::Tcp);
    assert_eq!(parsed.credential().password(), "secret:password");
    assert_eq!(parsed.credential().flow(), None);
    assert_eq!(
        parsed.tls(),
        Some(&TlsConfig::Tls {
            server_name: None,
            allow_insecure: false,
            alpn: Vec::new(),
            fingerprint: None,
        })
    );

    let credential_ref = CredentialRef::new("keychain://nodes/trojan-password").unwrap();
    let node = parsed
        .into_proxy_node(Uuid::parse_str(NODE_ID).unwrap(), credential_ref)
        .unwrap();

    assert_eq!(node.protocol_type, ProxyProtocol::Trojan);
    assert_eq!(node.name.as_str(), "US West");
    assert_eq!(node.server.as_str(), "edge.example.com");
    assert_eq!(node.transport, Some(TransportConfig::Tcp));
    assert_eq!(
        node.credential_ref.as_str(),
        "keychain://nodes/trojan-password"
    );
}

#[test]
fn parses_websocket_tls_and_v2rayn_insecure_aliases() {
    let parsed = TrojanParser
        .parse(&format!(
            "trojan://{PASSWORD}@edge.example.com:443?type=ws&security=tls\
             &host=cdn.example.com&path=%2Fproxy%20path&sni=www.example.com\
             &fp=chrome&alpn=h2%2Chttp%2F1.1&allowInsecure=1&insecure=1#WS"
        ))
        .unwrap();

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
            allow_insecure: true,
            alpn: vec!["h2".to_owned(), "http/1.1".to_owned()],
            fingerprint: Some("chrome".to_owned()),
        })
    );
}

#[test]
fn parses_ipv6_grpc_reality_and_flow_without_exposing_the_password() {
    let parsed = TrojanParser
        .parse(&format!(
            "trojan://{PASSWORD}@[2001:db8::1]:8443?type=grpc\
             &serviceName=api%2Ev1&mode=multi&authority=edge.example.com\
             &security=reality&sni=www.example.com&fp=chrome\
             &pbk=example-public-key&sid=&spx=%2Fprobe\
             &flow=xtls-rprx-vision#Reality"
        ))
        .unwrap();

    assert_eq!(parsed.server().as_str(), "2001:db8::1");
    assert_eq!(parsed.credential().flow(), Some("xtls-rprx-vision"));
    assert_eq!(
        parsed.transport(),
        &TransportConfig::Grpc {
            service_name: "api.v1".to_owned(),
            mode: GrpcMode::Multi,
            authority: Some("edge.example.com".to_owned()),
        }
    );
    assert_eq!(
        parsed.tls(),
        Some(&TlsConfig::Reality {
            server_name: "www.example.com".to_owned(),
            public_key: "example-public-key".to_owned(),
            short_id: Some(String::new()),
            fingerprint: Some("chrome".to_owned()),
            alpn: Vec::new(),
            spider_x: Some("/probe".to_owned()),
        })
    );

    let debug_output = format!("{parsed:?}");
    assert!(debug_output.contains("[REDACTED]"));
    assert!(!debug_output.contains(PASSWORD));
}

#[test]
fn accepts_root_path_raw_alias_explicit_none_and_tcp_header_none() {
    let private = TrojanParser
        .parse(&format!(
            "trojan://{PASSWORD}@192.0.2.7:8443/?type=raw&security=none#"
        ))
        .unwrap();
    let tcp = TrojanParser
        .parse(&format!(
            "trojan://{PASSWORD}@edge.example.com:443?type=tcp&headerType=none"
        ))
        .unwrap();

    assert_eq!(private.name().as_str(), "192.0.2.7:8443");
    assert_eq!(private.transport(), &TransportConfig::Tcp);
    assert_eq!(private.tls(), None);
    assert_eq!(tcp.transport(), &TransportConfig::Tcp);
    assert!(matches!(tcp.tls(), Some(TlsConfig::Tls { .. })));
}

#[test]
fn rejects_invalid_identity_endpoint_and_path_fields() {
    let cases = [
        "vless://password@example.com:443".to_owned(),
        "trojan://@example.com:443".to_owned(),
        "trojan://user:password@example.com:443".to_owned(),
        format!("trojan://{PASSWORD}@example.com"),
        format!("trojan://{PASSWORD}@example.com:443/not-allowed"),
        format!("trojan://{PASSWORD}@example.com:443?type=ws&path=%ZZ"),
    ];
    let errors = cases.map(|uri| TrojanParser.parse(&uri).unwrap_err());

    assert!(matches!(errors[0], TrojanParseError::UnsupportedScheme));
    assert!(matches!(errors[1], TrojanParseError::MissingPassword));
    assert!(matches!(
        errors[2],
        TrojanParseError::UnexpectedPasswordComponent
    ));
    assert!(matches!(
        &errors[3],
        TrojanParseError::InvalidUri {
            source: VlessParseError::MissingPort
        }
    ));
    assert!(matches!(errors[4], TrojanParseError::UnexpectedPath));
    assert!(matches!(
        &errors[5],
        TrojanParseError::InvalidUri {
            source: VlessParseError::InvalidPercentEncoding
        }
    ));
}

#[test]
fn rejects_duplicate_unknown_and_unsupported_transport_fields() {
    let duplicate = TrojanParser
        .parse(&format!(
            "trojan://{PASSWORD}@example.com:443?type=ws&type=grpc"
        ))
        .unwrap_err();
    let unknown = TrojanParser
        .parse(&format!(
            "trojan://{PASSWORD}@example.com:443?unknown=value"
        ))
        .unwrap_err();
    let kcp = TrojanParser
        .parse(&format!("trojan://{PASSWORD}@example.com:443?type=kcp"))
        .unwrap_err();
    let tcp_header = TrojanParser
        .parse(&format!(
            "trojan://{PASSWORD}@example.com:443?headerType=http"
        ))
        .unwrap_err();

    assert!(matches!(
        &duplicate,
        TrojanParseError::InvalidUri {
            source: VlessParseError::DuplicateParameter { name }
        } if name == "type"
    ));
    assert!(matches!(
        &unknown,
        TrojanParseError::InvalidUri {
            source: VlessParseError::UnsupportedParameter { name }
        } if name == "unknown"
    ));
    assert!(matches!(
        &kcp,
        TrojanParseError::UnsupportedTransport { value } if value == "kcp"
    ));
    assert!(matches!(
        &tcp_header,
        TrojanParseError::UnsupportedTcpHeader { value } if value == "http"
    ));
}

#[test]
fn validates_transport_and_security_specific_values() {
    let cases = [
        format!("trojan://{PASSWORD}@example.com:443?type=ws&path="),
        format!("trojan://{PASSWORD}@example.com:443?type=grpc"),
        format!("trojan://{PASSWORD}@example.com:443?type=grpc&serviceName=api&mode=invalid"),
        format!("trojan://{PASSWORD}@example.com:443?security=reality&fp=chrome"),
        format!("trojan://{PASSWORD}@example.com:443?security=reality&pbk=value"),
        format!("trojan://{PASSWORD}@example.com:443?security=tls&alpn=h2%2C%2Chttp"),
        format!(
            "trojan://{PASSWORD}@example.com:443?type=ws&security=reality\
             &pbk=value&fp=chrome"
        ),
    ];
    let errors = cases.map(|uri| TrojanParser.parse(&uri).unwrap_err());

    assert!(matches!(
        &errors[0],
        TrojanParseError::InvalidUri {
            source: VlessParseError::EmptyParameter { name: "path" }
        }
    ));
    assert!(matches!(
        &errors[1],
        TrojanParseError::InvalidUri {
            source: VlessParseError::MissingParameter {
                name: "serviceName"
            }
        }
    ));
    assert!(matches!(
        &errors[2],
        TrojanParseError::UnsupportedGrpcMode { value } if value == "invalid"
    ));
    assert!(matches!(
        &errors[3],
        TrojanParseError::InvalidUri {
            source: VlessParseError::MissingParameter { name: "pbk" }
        }
    ));
    assert!(matches!(
        &errors[4],
        TrojanParseError::InvalidUri {
            source: VlessParseError::MissingParameter { name: "fp" }
        }
    ));
    assert!(matches!(
        &errors[5],
        TrojanParseError::InvalidUri {
            source: VlessParseError::InvalidAlpn
        }
    ));
    assert!(matches!(
        errors[6],
        TrojanParseError::UnsupportedCombination {
            transport: "websocket",
            security: "reality"
        }
    ));
}

#[test]
fn validates_insecure_alias_values_and_security_compatibility() {
    let invalid = TrojanParser
        .parse(&format!(
            "trojan://{PASSWORD}@example.com:443?allowInsecure=2"
        ))
        .unwrap_err();
    let conflict = TrojanParser
        .parse(&format!(
            "trojan://{PASSWORD}@example.com:443?allowInsecure=1&insecure=0"
        ))
        .unwrap_err();
    let reality = TrojanParser
        .parse(&format!(
            "trojan://{PASSWORD}@example.com:443?security=reality&pbk=value\
             &fp=chrome&allowInsecure=1"
        ))
        .unwrap_err();
    let none = TrojanParser
        .parse(&format!(
            "trojan://{PASSWORD}@example.com:443?security=none&insecure=1"
        ))
        .unwrap_err();

    assert!(matches!(
        &invalid,
        TrojanParseError::InvalidBoolean { name, value }
            if *name == "allowInsecure" && value == "2"
    ));
    assert!(matches!(
        conflict,
        TrojanParseError::ConflictingInsecureAliases
    ));
    assert!(matches!(
        reality,
        TrojanParseError::UnsupportedInsecureSecurity {
            security: "reality"
        }
    ));
    assert!(matches!(
        none,
        TrojanParseError::UnsupportedInsecureSecurity { security: "none" }
    ));
}
