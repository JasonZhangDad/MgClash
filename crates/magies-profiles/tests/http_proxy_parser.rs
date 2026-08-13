use magies_domain::{CredentialRef, ProxyProtocol, TlsConfig, TransportConfig};
use magies_profiles::{HttpProxyParseError, HttpProxyParser, VlessParseError};
use uuid::Uuid;

const NODE_ID: &str = "018f78b5-08ee-7caa-94f3-1d5d781aba31";

fn node_id() -> Uuid {
    Uuid::parse_str(NODE_ID).unwrap()
}

#[test]
fn recognizes_http_and_https_schemes() {
    let parser = HttpProxyParser;

    assert!(parser.can_parse("  http://user:pass@example.com:8080  "));
    assert!(parser.can_parse("https://user:pass@example.com:8443"));
    assert!(!parser.can_parse("trojan://password@example.com:443"));
    assert!(!parser.can_parse("HTTP://example.com:8080"));
}

#[test]
fn does_not_claim_a_subscription_style_uri_with_a_path() {
    // A `https://host/sub` subscription URL must not be stolen from the
    // subscription fetcher by this parser.
    let parser = HttpProxyParser;

    assert!(!parser.can_parse("https://example.com/sub/abc123"));
    assert!(parser.can_parse("https://example.com/"));
    assert!(parser.can_parse("https://example.com"));
}

#[test]
fn parses_plain_http_with_no_tls_and_defaults_the_port() {
    let parsed = HttpProxyParser
        .parse("http://alice:hunter2@edge.example.com#US%20West")
        .unwrap();

    assert_eq!(parsed.name().as_str(), "US West");
    assert_eq!(parsed.server().as_str(), "edge.example.com");
    assert_eq!(parsed.port().get(), 80);
    assert_eq!(parsed.transport(), &TransportConfig::Tcp);
    assert_eq!(parsed.tls(), None);
    assert_eq!(parsed.credential().username(), Some("alice"));
    assert_eq!(parsed.credential().password(), Some("hunter2"));

    let node = parsed
        .into_proxy_node(
            node_id(),
            CredentialRef::new("keychain://nodes/http").unwrap(),
        )
        .unwrap();
    assert_eq!(node.protocol_type, ProxyProtocol::Http);
    assert_eq!(node.transport, Some(TransportConfig::Tcp));
    assert_eq!(node.tls, None);
}

#[test]
fn parses_https_with_tls_defaulting_server_name_to_host_and_the_default_port() {
    let parsed = HttpProxyParser.parse("https://proxy.example.com").unwrap();

    assert_eq!(parsed.port().get(), 443);
    assert_eq!(
        parsed.tls(),
        Some(&TlsConfig::Tls {
            server_name: Some("proxy.example.com".to_owned()),
            allow_insecure: false,
            alpn: Vec::new(),
            fingerprint: None,
            pinned_sha256: None,
        })
    );

    let node = parsed
        .into_proxy_node(
            node_id(),
            CredentialRef::new("keychain://nodes/https").unwrap(),
        )
        .unwrap();
    assert!(matches!(node.tls, Some(TlsConfig::Tls { .. })));
}

#[test]
fn parses_an_explicit_port() {
    let parsed = HttpProxyParser
        .parse("https://proxy.example.com:9443")
        .unwrap();

    assert_eq!(parsed.port().get(), 9443);
}

#[test]
fn username_without_password_is_accepted() {
    let parsed = HttpProxyParser
        .parse("http://alice@example.com:8080")
        .unwrap();

    assert_eq!(parsed.credential().username(), Some("alice"));
    assert_eq!(parsed.credential().password(), None);
}

#[test]
fn password_without_username_is_rejected() {
    let error = HttpProxyParser
        .parse("http://:hunter2@example.com:8080")
        .unwrap_err();

    assert!(matches!(
        error,
        HttpProxyParseError::PasswordRequiresUsername
    ));
}

#[test]
fn rejects_unknown_query_parameters() {
    let error = HttpProxyParser
        .parse("http://example.com:8080?foo=bar")
        .unwrap_err();

    assert!(matches!(
        &error,
        HttpProxyParseError::InvalidUri {
            source: VlessParseError::UnsupportedParameter { name }
        } if name == "foo"
    ));
}

#[test]
fn rejects_wrong_scheme_and_a_uri_with_a_path() {
    let wrong_scheme = HttpProxyParser
        .parse("socks://example.com:8080")
        .unwrap_err();
    // `can_parse` already excludes a path, so `parse` reports the same
    // rejection `ShareLinkParser` would see: no parser claims this URI.
    let bad_path = HttpProxyParser
        .parse("http://example.com:8080/not-allowed")
        .unwrap_err();

    assert!(matches!(
        wrong_scheme,
        HttpProxyParseError::UnsupportedScheme
    ));
    assert!(matches!(bad_path, HttpProxyParseError::UnsupportedScheme));
}

#[test]
fn redacts_username_and_password_from_debug_output() {
    let parsed = HttpProxyParser
        .parse("http://alice:top-secret@example.com:8080")
        .unwrap();

    let credential_debug = format!("{:?}", parsed.credential());
    let parsed_debug = format!("{parsed:?}");
    for debug_output in [credential_debug, parsed_debug] {
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("alice"));
        assert!(!debug_output.contains("top-secret"));
    }
}
