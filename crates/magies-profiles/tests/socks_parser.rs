use magies_domain::{CredentialRef, ProxyProtocol, TransportConfig};
use magies_profiles::{SocksParseError, SocksParser, VlessParseError};
use uuid::Uuid;

const NODE_ID: &str = "018f78b5-08ee-7caa-94f3-1d5d781aba30";

fn node_id() -> Uuid {
    Uuid::parse_str(NODE_ID).unwrap()
}

#[test]
fn recognizes_socks_socks5_and_socks5h_schemes() {
    let parser = SocksParser;

    assert!(parser.can_parse("  socks://user:pass@example.com:1080  "));
    assert!(parser.can_parse("socks5://user:pass@example.com:1080"));
    assert!(parser.can_parse("socks5h://user:pass@example.com:1080"));
    assert!(!parser.can_parse("trojan://password@example.com:443"));
    assert!(!parser.can_parse("SOCKS://user@example.com:1080"));
}

#[test]
fn does_not_claim_a_uri_with_a_path() {
    // A subscription-style URL must never be mistaken for a SOCKS node link.
    let parser = SocksParser;

    assert!(!parser.can_parse("socks://example.com:1080/some/path"));
}

#[test]
fn parses_username_password_and_defaults_the_port() {
    let parsed = SocksParser
        .parse("socks5://alice:hunter2@edge.example.com#US%20West")
        .unwrap();

    assert_eq!(parsed.name().as_str(), "US West");
    assert_eq!(parsed.server().as_str(), "edge.example.com");
    assert_eq!(parsed.port().get(), 1080);
    assert_eq!(parsed.transport(), &TransportConfig::Tcp);
    assert_eq!(parsed.tls(), None);
    assert_eq!(parsed.credential().username(), Some("alice"));
    assert_eq!(parsed.credential().password(), Some("hunter2"));

    let node = parsed
        .into_proxy_node(
            node_id(),
            CredentialRef::new("keychain://nodes/socks").unwrap(),
        )
        .unwrap();
    assert_eq!(node.protocol_type, ProxyProtocol::Socks);
    assert_eq!(node.server.as_str(), "edge.example.com");
    assert_eq!(node.port.get(), 1080);
    assert_eq!(node.transport, Some(TransportConfig::Tcp));
    assert_eq!(node.tls, None);
}

#[test]
fn parses_an_explicit_port_and_treats_socks5h_like_socks5() {
    let parsed = SocksParser
        .parse("socks5h://proxy.example.com:10800")
        .unwrap();

    assert_eq!(parsed.port().get(), 10800);
    assert_eq!(parsed.credential().username(), None);
    assert_eq!(parsed.credential().password(), None);
}

#[test]
fn username_without_password_is_accepted() {
    let parsed = SocksParser.parse("socks://alice@example.com:1080").unwrap();

    assert_eq!(parsed.credential().username(), Some("alice"));
    assert_eq!(parsed.credential().password(), None);
}

#[test]
fn password_without_username_is_rejected() {
    let error = SocksParser
        .parse("socks://:hunter2@example.com:1080")
        .unwrap_err();

    assert!(matches!(error, SocksParseError::PasswordRequiresUsername));
}

#[test]
fn rejects_unknown_query_parameters() {
    let error = SocksParser
        .parse("socks://example.com:1080?v=4")
        .unwrap_err();

    assert!(matches!(
        &error,
        SocksParseError::InvalidUri {
            source: VlessParseError::UnsupportedParameter { name }
        } if name == "v"
    ));
}

#[test]
fn rejects_wrong_scheme_and_a_uri_with_a_path() {
    let wrong_scheme = SocksParser.parse("http://example.com:1080").unwrap_err();
    // `can_parse` already excludes a path, so `parse` reports the same
    // rejection `ShareLinkParser` would see: no parser claims this URI.
    let bad_path = SocksParser
        .parse("socks://example.com:1080/not-allowed")
        .unwrap_err();

    assert!(matches!(wrong_scheme, SocksParseError::UnsupportedScheme));
    assert!(matches!(bad_path, SocksParseError::UnsupportedScheme));
}

#[test]
fn redacts_username_and_password_from_debug_output() {
    let parsed = SocksParser
        .parse("socks://alice:top-secret@example.com:1080")
        .unwrap();

    let credential_debug = format!("{:?}", parsed.credential());
    let parsed_debug = format!("{parsed:?}");
    for debug_output in [credential_debug, parsed_debug] {
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("alice"));
        assert!(!debug_output.contains("top-secret"));
    }
}
