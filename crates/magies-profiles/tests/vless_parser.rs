use magies_domain::{CredentialRef, GrpcMode, ProxyProtocol, TlsConfig, TransportConfig};
use magies_profiles::{VlessParseError, VlessParser};
use uuid::Uuid;

const USER_ID: &str = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";
const NODE_ID: &str = "018f78b5-08ee-7caa-94f3-1d5d781aba22";

#[test]
fn recognizes_only_the_exact_vless_scheme() {
    let parser = VlessParser;

    assert!(parser.can_parse(&format!("  vless://{USER_ID}@example.com:443  ")));
    assert!(!parser.can_parse(&format!("vmess://{USER_ID}@example.com:443")));
    assert!(!parser.can_parse(&format!("VLESS://{USER_ID}@example.com:443")));
}

#[test]
fn parses_a_default_tcp_node_and_materializes_it_after_secret_storage() {
    let parser = VlessParser;
    let parsed = parser
        .parse(&format!("vless://{USER_ID}@edge.example.com:443#US%20West"))
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
    assert_eq!(parsed.credential().encryption(), "none");
    assert_eq!(parsed.credential().flow(), None);

    let credential_ref = CredentialRef::new("keychain://nodes/vless-user").unwrap();
    let node = parsed
        .into_proxy_node(Uuid::parse_str(NODE_ID).unwrap(), credential_ref)
        .unwrap();

    assert_eq!(node.id, Uuid::parse_str(NODE_ID).unwrap());
    assert_eq!(node.protocol_type, ProxyProtocol::Vless);
    assert_eq!(node.name.as_str(), "US West");
    assert_eq!(node.server.as_str(), "edge.example.com");
    assert_eq!(node.transport, Some(TransportConfig::Tcp));
    assert_eq!(node.credential_ref.as_str(), "keychain://nodes/vless-user");
}

#[test]
fn parses_percent_encoded_websocket_tls_fields() {
    let parsed = VlessParser
        .parse(&format!(
            "vless://{USER_ID}@edge.example.com:443?type=ws&security=tls\
             &host=cdn.example.com&path=%2Fproxy%20path&sni=www.example.com\
             &fp=chrome&alpn=h2%2Chttp%2F1.1#WS%20%E8%8A%82%E7%82%B9"
        ))
        .unwrap();

    assert_eq!(parsed.name().as_str(), "WS 节点");
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
}

#[test]
fn parses_ipv6_grpc_reality_without_exposing_the_user_id_in_debug() {
    let parsed = VlessParser
        .parse(&format!(
            "vless://{USER_ID}@[2001:db8::1]:8443?type=grpc\
             &serviceName=api%2Ev1&mode=multi&authority=edge.example.com\
             &security=reality&sni=www.example.com&fp=chrome\
             &pbk=example-public-key&sid=&spx=%2Fprobe\
             &flow=xtls-rprx-vision#Reality"
        ))
        .unwrap();

    assert_eq!(parsed.server().as_str(), "2001:db8::1");
    assert_eq!(parsed.port().get(), 8443);
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
    assert_eq!(parsed.credential().flow(), Some("xtls-rprx-vision"));

    let debug_output = format!("{parsed:?}");
    assert!(debug_output.contains("[REDACTED]"));
    assert!(!debug_output.contains(USER_ID));
}

#[test]
fn applies_documented_defaults_for_names_transport_and_tls() {
    let tcp = VlessParser
        .parse(&format!("vless://{USER_ID}@edge.example.com:80"))
        .unwrap();
    let websocket = VlessParser
        .parse(&format!("vless://{USER_ID}@edge.example.com:80?type=ws"))
        .unwrap();
    let reality = VlessParser
        .parse(&format!(
            "vless://{USER_ID}@edge.example.com:443?security=reality\
             &fp=chrome&pbk=example-public-key"
        ))
        .unwrap();

    assert_eq!(tcp.name().as_str(), "edge.example.com:80");
    assert_eq!(tcp.transport(), &TransportConfig::Tcp);
    assert_eq!(
        websocket.transport(),
        &TransportConfig::WebSocket {
            path: "/".to_owned(),
            host: None,
        }
    );
    assert!(matches!(
        reality.tls(),
        Some(TlsConfig::Reality { server_name, .. }) if server_name == "edge.example.com"
    ));
}

#[test]
fn handles_empty_url_sections_ipv4_and_the_remaining_grpc_mode() {
    let empty_query = VlessParser
        .parse(&format!("vless://{USER_ID}@198.51.100.7:443?"))
        .unwrap();
    let empty_name = VlessParser
        .parse(&format!("vless://{USER_ID}@edge.example.com:443#"))
        .unwrap();
    let grpc = VlessParser
        .parse(&format!(
            "vless://{USER_ID}@edge.example.com:443?type=grpc\
             &serviceName=api&mode=guna&encryption=none"
        ))
        .unwrap();

    assert_eq!(empty_query.server().as_str(), "198.51.100.7");
    assert_eq!(empty_query.name().as_str(), "198.51.100.7:443");
    assert_eq!(empty_name.name().as_str(), "edge.example.com:443");
    assert!(matches!(
        grpc.transport(),
        TransportConfig::Grpc {
            mode: GrpcMode::Guna,
            ..
        }
    ));
    assert_eq!(grpc.credential().encryption(), "none");

    assert_eq!(
        VlessParser
            .parse(&format!(
                "vless://{USER_ID}@example.com:443?security=tls&alpn=h2%2C%2Chttp"
            ))
            .unwrap_err(),
        VlessParseError::InvalidAlpn
    );
}

#[test]
fn rejects_invalid_uri_identity_and_endpoint_fields() {
    let cases = [
        format!("vmess://{USER_ID}@example.com:443"),
        "vless://example.com:443".to_owned(),
        "vless://not-a-uuid@example.com:443".to_owned(),
        format!("vless://{USER_ID}:password@example.com:443"),
        format!("vless://{USER_ID}@example.com"),
        format!("vless://{USER_ID}@example.com:0"),
        format!("vless://{USER_ID}@example.com:443?type=ws&path=%ZZ"),
        format!("vless://{USER_ID}@example.com:443/not-allowed"),
    ];

    let errors = cases.map(|uri| VlessParser.parse(&uri).unwrap_err());

    assert!(matches!(errors[0], VlessParseError::UnsupportedScheme));
    assert!(matches!(errors[1], VlessParseError::MissingUserId));
    assert!(matches!(errors[2], VlessParseError::InvalidUserId { .. }));
    assert!(matches!(errors[3], VlessParseError::UnexpectedPassword));
    assert!(matches!(errors[4], VlessParseError::MissingPort));
    assert!(matches!(
        errors[5],
        VlessParseError::InvalidPort { port: 0 }
    ));
    assert!(matches!(errors[6], VlessParseError::InvalidPercentEncoding));
    assert!(matches!(errors[7], VlessParseError::UnexpectedPath));

    for error in errors {
        assert!(!format!("{error:?}").contains(USER_ID));
    }
}

#[test]
fn rejects_duplicate_unsupported_and_unmappable_parameters() {
    let cases = [
        (
            format!("vless://{USER_ID}@example.com:443?type=ws&type=grpc"),
            VlessParseError::DuplicateParameter {
                name: "type".to_owned(),
            },
        ),
        (
            format!("vless://{USER_ID}@example.com:443?type=kcp"),
            VlessParseError::UnsupportedTransport {
                value: "kcp".to_owned(),
            },
        ),
        (
            format!("vless://{USER_ID}@example.com:443?security=xtls"),
            VlessParseError::UnsupportedSecurity {
                value: "xtls".to_owned(),
            },
        ),
        (
            format!("vless://{USER_ID}@example.com:443?unknown=value"),
            VlessParseError::UnsupportedParameter {
                name: "unknown".to_owned(),
            },
        ),
        (
            format!("vless://{USER_ID}@example.com:443?security=tls&pbk=value"),
            VlessParseError::UnsupportedParameter {
                name: "pbk".to_owned(),
            },
        ),
    ];

    for (uri, expected) in cases {
        assert_eq!(VlessParser.parse(&uri).unwrap_err(), expected);
    }
}

#[test]
fn validates_transport_and_security_specific_required_values() {
    let cases = [
        (
            format!("vless://{USER_ID}@example.com:443?type=ws&path="),
            VlessParseError::EmptyParameter { name: "path" },
        ),
        (
            format!("vless://{USER_ID}@example.com:443?type=grpc"),
            VlessParseError::MissingParameter {
                name: "serviceName",
            },
        ),
        (
            format!("vless://{USER_ID}@example.com:443?type=grpc&serviceName=api&mode=invalid"),
            VlessParseError::UnsupportedGrpcMode {
                value: "invalid".to_owned(),
            },
        ),
        (
            format!("vless://{USER_ID}@example.com:443?security=reality&fp=chrome"),
            VlessParseError::MissingParameter { name: "pbk" },
        ),
        (
            format!("vless://{USER_ID}@example.com:443?security=reality&pbk=value"),
            VlessParseError::MissingParameter { name: "fp" },
        ),
        (
            format!("vless://{USER_ID}@example.com:443?security=tls&alpn="),
            VlessParseError::EmptyParameter { name: "alpn" },
        ),
    ];

    for (uri, expected) in cases {
        assert_eq!(VlessParser.parse(&uri).unwrap_err(), expected);
    }
}
