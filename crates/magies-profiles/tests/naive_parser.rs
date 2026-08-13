use magies_domain::{CredentialRef, ProxyProtocol, TlsConfig};
use magies_profiles::{NaiveCongestionControl, NaiveParseError, NaiveParser, VlessParseError};
use uuid::Uuid;

const NODE_ID: &str = "018f78b5-08ee-7caa-94f3-1d5d781aba24";

#[test]
fn recognizes_naive_schemes() {
    let parser = NaiveParser;

    assert!(parser.can_parse("  naive://user:pass@example.com  "));
    assert!(parser.can_parse("naive+https://user@example.com"));
    assert!(parser.can_parse("naive+quic://example.com"));
    assert!(!parser.can_parse("naive2://example.com"));
    assert!(!parser.can_parse("NAIVE://example.com"));
    assert!(!parser.can_parse("ssr://example.com"));
}

#[test]
fn parses_password_default_port_and_tls() {
    let parsed = NaiveParser
        .parse("naive://alice:hunter2@edge.example.com#Tokyo")
        .unwrap();

    assert_eq!(parsed.name().as_str(), "Tokyo");
    assert_eq!(parsed.server().as_str(), "edge.example.com");
    assert_eq!(parsed.port().get(), 443);
    assert_eq!(parsed.credential().username(), Some("alice"));
    assert_eq!(parsed.credential().password(), Some("hunter2"));
    assert!(!parsed.credential().quic());
    assert_eq!(parsed.credential().quic_congestion_control(), None);
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
            CredentialRef::new("keychain://nodes/naive").unwrap(),
        )
        .unwrap();
    assert_eq!(node.protocol_type, ProxyProtocol::Naive);
    assert!(node.transport.is_none());
    assert!(node.tls.is_some());
}

#[test]
fn naive_https_is_http2_and_naive_quic_sets_quic() {
    let https = NaiveParser
        .parse("naive+https://edge.example.com:8443?sni=cdn.example.com")
        .unwrap();
    assert!(!https.credential().quic());
    assert_eq!(https.port().get(), 8443);
    assert_eq!(
        https.tls(),
        Some(&TlsConfig::Tls {
            server_name: Some("cdn.example.com".to_owned()),
            allow_insecure: false,
            alpn: Vec::new(),
            fingerprint: None,
            pinned_sha256: None,
        })
    );

    let quic = NaiveParser
        .parse(
            "naive+quic://alice:secret@edge.example.com?\
             congestion_control=bbr2#QUIC",
        )
        .unwrap();
    assert!(quic.credential().quic());
    assert_eq!(
        quic.credential().quic_congestion_control(),
        Some(NaiveCongestionControl::Bbr2)
    );
    assert_eq!(quic.name().as_str(), "QUIC");
}

#[test]
fn accepts_congestion_control_aliases() {
    let parsed = NaiveParser
        .parse("naive://edge.example.com?cc=cubic")
        .unwrap();
    assert_eq!(
        parsed.credential().quic_congestion_control(),
        Some(NaiveCongestionControl::Cubic)
    );
}

#[test]
fn password_without_username_is_rejected() {
    let error = NaiveParser
        .parse("naive://:hunter2@example.com")
        .unwrap_err();
    assert!(matches!(error, NaiveParseError::PasswordRequiresUsername));
}

#[test]
fn rejects_fingerprint_alpn_insecure_and_reality() {
    for link in [
        "naive://example.com?fp=chrome",
        "naive://example.com?alpn=h2",
        "naive://example.com?insecure=1",
        "naive://example.com?security=reality&pbk=key",
        "naive://example.com?insecure_concurrency=2",
    ] {
        let error = NaiveParser.parse(link).unwrap_err();
        assert!(
            matches!(error, NaiveParseError::UnsupportedParameter { .. }),
            "{link} -> {error:?}"
        );
    }
}

#[test]
fn rejects_unknown_query_parameters() {
    let error = NaiveParser
        .parse("naive://example.com?extra=1")
        .unwrap_err();
    assert!(matches!(
        error,
        NaiveParseError::InvalidUri {
            source: VlessParseError::UnsupportedParameter { name }
        } if name == "extra"
    ));
}

#[test]
fn rejects_unknown_congestion_control() {
    let error = NaiveParser
        .parse("naive://example.com?congestion_control=vegas")
        .unwrap_err();
    assert!(matches!(
        error,
        NaiveParseError::UnsupportedCongestionControl { value } if value == "vegas"
    ));
}
