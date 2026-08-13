use magies_domain::{CredentialRef, ProxyProtocol};
use magies_profiles::{WireGuardParseError, WireGuardParser};
use uuid::Uuid;

const NODE_ID: &str = "018f78b5-08ee-7caa-94f3-1d5d781aba25";

fn node_id() -> Uuid {
    Uuid::parse_str(NODE_ID).unwrap()
}

#[test]
fn recognizes_the_wireguard_scheme() {
    let parser = WireGuardParser;

    assert!(parser.can_parse("  wireguard://key@example.com  "));
    assert!(!parser.can_parse("wg://key@example.com"));
    assert!(!parser.can_parse("WIREGUARD://key@example.com"));
    assert!(!parser.can_parse("tuic://key@example.com"));
}

#[test]
fn parses_a_full_link_with_every_optional_field() {
    let parsed = WireGuardParser
        .parse(
            "wireguard://cHJpdmF0ZS1rZXk%3D@edge.example.com:51821\
             ?publickey=cGVlci1wdWJsaWMta2V5&address=10.0.0.2/32,fd00::1/128\
             &mtu=1420&presharedkey=cHNr&reserved=1,2,3#Tokyo",
        )
        .unwrap();

    assert_eq!(parsed.name().as_str(), "Tokyo");
    assert_eq!(parsed.server().as_str(), "edge.example.com");
    assert_eq!(parsed.port().get(), 51821);
    assert_eq!(parsed.credential().private_key(), "cHJpdmF0ZS1rZXk=");
    assert_eq!(parsed.credential().peer_public_key(), "cGVlci1wdWJsaWMta2V5");
    assert_eq!(parsed.credential().pre_shared_key(), Some("cHNr"));
    assert_eq!(
        parsed.credential().local_address(),
        &["10.0.0.2/32".to_owned(), "fd00::1/128".to_owned()]
    );
    assert_eq!(parsed.credential().mtu(), Some(1420));
    assert_eq!(parsed.credential().reserved(), Some([1, 2, 3]));
    assert_eq!(parsed.transport(), None);
    assert_eq!(parsed.tls(), None);

    let node = parsed
        .into_proxy_node(node_id(), CredentialRef::new("keychain://nodes/wg").unwrap())
        .unwrap();
    assert_eq!(node.protocol_type, ProxyProtocol::WireGuard);
    assert!(node.transport.is_none());
    assert!(node.tls.is_none());
}

#[test]
fn defaults_the_port_and_name_when_omitted() {
    let parsed = WireGuardParser
        .parse("wireguard://key@edge.example.com?publickey=peer&address=10.0.0.2/32")
        .unwrap();

    assert_eq!(parsed.port().get(), 51820);
    assert_eq!(parsed.name().as_str(), "edge.example.com:51820");
}

#[test]
fn rejects_a_missing_private_key() {
    assert!(matches!(
        WireGuardParser.parse("wireguard://edge.example.com?publickey=peer&address=10.0.0.2/32"),
        Err(WireGuardParseError::MissingPrivateKey)
    ));
}

#[test]
fn rejects_a_missing_public_key() {
    assert!(WireGuardParser
        .parse("wireguard://key@edge.example.com?address=10.0.0.2/32")
        .is_err());
}

#[test]
fn rejects_a_missing_address() {
    assert!(WireGuardParser
        .parse("wireguard://key@edge.example.com?publickey=peer")
        .is_err());
}

#[test]
fn rejects_an_invalid_reserved_field() {
    assert!(matches!(
        WireGuardParser.parse(
            "wireguard://key@edge.example.com?publickey=peer&address=10.0.0.2/32&reserved=1,2"
        ),
        Err(WireGuardParseError::InvalidReserved { .. })
    ));
}

#[test]
fn rejects_conflicting_pre_shared_key_spellings() {
    assert!(matches!(
        WireGuardParser.parse(
            "wireguard://key@edge.example.com?publickey=peer&address=10.0.0.2/32\
             &presharedkey=aaa&psk=bbb"
        ),
        Err(WireGuardParseError::ConflictingPreSharedKey)
    ));
}

#[test]
fn rejects_an_unknown_query_parameter() {
    assert!(
        WireGuardParser
            .parse(
                "wireguard://key@edge.example.com?publickey=peer&address=10.0.0.2/32\
                 &unknown=1"
            )
            .is_err()
    );
}

#[test]
fn rejects_a_non_empty_path() {
    assert!(matches!(
        WireGuardParser.parse(
            "wireguard://key@edge.example.com/tunnel?publickey=peer&address=10.0.0.2/32"
        ),
        Err(WireGuardParseError::UnexpectedPath)
    ));
}

#[test]
fn rejects_a_separate_password() {
    assert!(matches!(
        WireGuardParser.parse(
            "wireguard://key:secret@edge.example.com?publickey=peer&address=10.0.0.2/32"
        ),
        Err(WireGuardParseError::UnexpectedPassword)
    ));
}
