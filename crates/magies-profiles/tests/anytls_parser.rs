use magies_domain::{CredentialRef, ProxyProtocol, TlsConfig};
use magies_profiles::{AnyTlsParseError, AnyTlsParser, VlessParseError};
use uuid::Uuid;

const NODE_ID: &str = "018f78b5-08ee-7caa-94f3-1d5d781aba24";

#[test]
fn recognizes_the_anytls_scheme() {
    let parser = AnyTlsParser;

    assert!(parser.can_parse("  anytls://password@example.com  "));
    assert!(!parser.can_parse("anytls2://password@example.com"));
    assert!(!parser.can_parse("ANYTLS://password@example.com"));
    assert!(!parser.can_parse("ssr://password@example.com"));
}

#[test]
fn parses_password_default_port_and_defaults_to_tls() {
    let parsed = AnyTlsParser
        .parse("anytls://hunter2@edge.example.com#Tokyo")
        .unwrap();

    assert_eq!(parsed.name().as_str(), "Tokyo");
    assert_eq!(parsed.server().as_str(), "edge.example.com");
    assert_eq!(parsed.port().get(), 443);
    assert_eq!(parsed.credential().password(), "hunter2");
    assert_eq!(parsed.transport(), None);
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

    let node = parsed
        .into_proxy_node(
            Uuid::parse_str(NODE_ID).unwrap(),
            CredentialRef::new("keychain://nodes/anytls").unwrap(),
        )
        .unwrap();
    assert_eq!(node.protocol_type, ProxyProtocol::AnyTls);
    assert!(node.transport.is_none());
    assert!(node.tls.is_some());
}

#[test]
fn parses_explicit_tls_fields_and_insecure_alias() {
    let pin = "ab".repeat(32);
    let parsed = AnyTlsParser
        .parse(&format!(
            "anytls://hunter2@edge.example.com:8443?security=tls\
             &sni=cdn.example.com&fp=chrome&alpn=h2%2Chttp%2F1.1\
             &pinSHA256={pin}&insecure=1"
        ))
        .unwrap();

    assert_eq!(parsed.port().get(), 8443);
    assert_eq!(
        parsed.tls(),
        Some(&TlsConfig::Tls {
            server_name: Some("cdn.example.com".to_owned()),
            allow_insecure: true,
            alpn: vec!["h2".to_owned(), "http/1.1".to_owned()],
            fingerprint: Some("chrome".to_owned()),
            pinned_sha256: Some(pin.try_into().unwrap()),
        })
    );
}

#[test]
fn parses_reality_with_public_key_short_id_and_spider_x() {
    let parsed = AnyTlsParser
        .parse(
            "anytls://hunter2@edge.example.com?security=reality\
             &sni=www.example.com&fp=chrome&pbk=example-public-key\
             &sid=ab12&spx=%2Fprobe",
        )
        .unwrap();

    assert!(matches!(
        parsed.tls(),
        Some(TlsConfig::Reality {
            server_name,
            public_key,
            short_id: Some(short_id),
            fingerprint: Some(fingerprint),
            spider_x: Some(spider_x),
            ..
        }) if server_name == "www.example.com"
            && public_key == "example-public-key"
            && short_id == "ab12"
            && fingerprint == "chrome"
            && spider_x == "/probe"
    ));
}

#[test]
fn rejects_an_empty_password() {
    assert!(matches!(
        AnyTlsParser.parse("anytls://@edge.example.com:443"),
        Err(AnyTlsParseError::MissingPassword)
    ));
}

#[test]
fn rejects_a_separate_password_component() {
    assert!(matches!(
        AnyTlsParser.parse("anytls://user:pass@edge.example.com:443"),
        Err(AnyTlsParseError::UnexpectedPasswordComponent)
    ));
}

#[test]
fn rejects_an_unexpected_path() {
    assert!(matches!(
        AnyTlsParser.parse("anytls://hunter2@edge.example.com:443/extra"),
        Err(AnyTlsParseError::UnexpectedPath)
    ));
}

#[test]
fn rejects_plaintext_security_because_anytls_requires_tls() {
    assert!(matches!(
        AnyTlsParser.parse("anytls://hunter2@edge.example.com:443?security=none"),
        Err(AnyTlsParseError::MissingTls)
    ));
}

#[test]
fn rejects_conflicting_insecure_aliases() {
    assert!(matches!(
        AnyTlsParser.parse("anytls://hunter2@edge.example.com:443?insecure=1&allowInsecure=0"),
        Err(AnyTlsParseError::ConflictingInsecureAliases)
    ));
}

#[test]
fn rejects_the_padding_scheme_query_parameter() {
    assert!(matches!(
        AnyTlsParser.parse("anytls://hunter2@edge.example.com:443?padding-scheme=stop%3D0"),
        Err(AnyTlsParseError::InvalidUri {
            source: VlessParseError::UnsupportedParameter { name }
        }) if name == "padding-scheme"
    ));
}

#[test]
fn rejects_the_log_level_query_parameter() {
    assert!(matches!(
        AnyTlsParser.parse("anytls://hunter2@edge.example.com:443?log-level=debug"),
        Err(AnyTlsParseError::InvalidUri {
            source: VlessParseError::UnsupportedParameter { name }
        }) if name == "log-level"
    ));
}

#[test]
fn rejects_a_reality_link_missing_its_public_key() {
    assert!(matches!(
        AnyTlsParser.parse("anytls://hunter2@edge.example.com:443?security=reality&fp=chrome"),
        Err(AnyTlsParseError::InvalidUri {
            source: VlessParseError::MissingParameter { name: "pbk" }
        })
    ));
}

#[test]
fn falls_back_to_the_host_and_port_when_the_fragment_is_absent() {
    let parsed = AnyTlsParser
        .parse("anytls://hunter2@edge.example.com:8443")
        .unwrap();

    assert_eq!(parsed.name().as_str(), "edge.example.com:8443");
}

#[test]
fn debug_redacts_the_password() {
    let parsed = AnyTlsParser
        .parse("anytls://hunter2@edge.example.com:443")
        .unwrap();

    assert_eq!(
        format!("{:?}", parsed.credential()),
        "AnyTlsCredential([REDACTED])"
    );
}
