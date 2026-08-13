use magies_domain::{CredentialRef, ProxyProtocol, TlsConfig};
use magies_profiles::{TuicCongestionControl, TuicParseError, TuicParser, TuicUdpRelayMode};
use uuid::Uuid;

const USER_ID: &str = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";
const NODE_ID: &str = "018f78b5-08ee-7caa-94f3-1d5d781aba24";

#[test]
fn recognizes_the_tuic_scheme() {
    let parser = TuicParser;

    assert!(parser.can_parse("  tuic://uuid:pass@example.com  "));
    assert!(!parser.can_parse("tuic+v4://token@example.com"));
    assert!(!parser.can_parse("TUIC://uuid@example.com"));
    assert!(!parser.can_parse("hysteria2://password@example.com"));
}

#[test]
fn parses_uuid_password_tls_and_materializes_intrinsic_quic_node() {
    let parsed = TuicParser
        .parse(&format!(
            "tuic://{USER_ID}:hunter2@edge.example.com?sni=cdn.example.com\
             &congestion_control=bbr&udp_relay_mode=native&allow_insecure=1#Tokyo"
        ))
        .unwrap();

    assert_eq!(parsed.name().as_str(), "Tokyo");
    assert_eq!(parsed.server().as_str(), "edge.example.com");
    assert_eq!(parsed.port().get(), 443);
    assert_eq!(parsed.credential().uuid().to_string(), USER_ID);
    assert_eq!(parsed.credential().password(), Some("hunter2"));
    assert_eq!(
        parsed.credential().congestion_control(),
        Some(TuicCongestionControl::Bbr)
    );
    assert_eq!(
        parsed.credential().udp_relay_mode(),
        Some(TuicUdpRelayMode::Native)
    );
    assert!(!parsed.credential().udp_over_stream());
    assert_eq!(parsed.transport(), None);
    assert_eq!(
        parsed.tls(),
        Some(&TlsConfig::Tls {
            server_name: Some("cdn.example.com".to_owned()),
            allow_insecure: true,
            alpn: Vec::new(),
            fingerprint: None,
            pinned_sha256: None,
        })
    );

    let node = parsed
        .into_proxy_node(
            Uuid::parse_str(NODE_ID).unwrap(),
            CredentialRef::new("keychain://nodes/tuic").unwrap(),
        )
        .unwrap();
    assert_eq!(node.protocol_type, ProxyProtocol::Tuic);
    assert!(node.transport.is_none());
}

#[test]
fn rejects_a_v4_token_that_is_not_a_uuid() {
    assert!(matches!(
        TuicParser.parse("tuic://token@edge.example.com:443"),
        Err(TuicParseError::InvalidUuid { .. })
    ));
}

#[test]
fn rejects_conflicting_udp_relay_settings() {
    assert!(matches!(
        TuicParser.parse(&format!(
            "tuic://{USER_ID}:pass@edge.example.com?udp_relay_mode=quic&udp_over_stream=1"
        )),
        Err(TuicParseError::ConflictingUdpRelaySettings)
    ));
}
