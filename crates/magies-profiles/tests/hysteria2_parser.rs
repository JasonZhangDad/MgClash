use magies_domain::{CertificatePin, CredentialRef, ProxyProtocol, TlsConfig};
use magies_profiles::{
    Hysteria2ObfuscationMethod, Hysteria2ParseError, Hysteria2Parser, VlessParseError,
};
use uuid::Uuid;

const AUTHENTICATION: &str = "secret:token";
const OBFS_PASSWORD: &str = "obfs-secret";
const NODE_ID: &str = "018f78b5-08ee-7caa-94f3-1d5d781aba24";

#[test]
fn recognizes_both_exact_hysteria2_schemes() {
    let parser = Hysteria2Parser;

    assert!(parser.can_parse("  hysteria2://password@example.com  "));
    assert!(parser.can_parse("hy2://password@example.com:8443"));
    assert!(!parser.can_parse("hysteria2+realm://token@example.com/realm"));
    assert!(!parser.can_parse("HYSTERIA2://password@example.com"));
    assert!(!parser.can_parse("trojan://password@example.com:443"));
}

#[test]
fn parses_default_port_tls_and_materializes_intrinsic_quic_node() {
    let parsed = Hysteria2Parser
        .parse("hysteria2://secret%3Atoken@edge.example.com#Default")
        .unwrap();

    assert_eq!(parsed.name().as_str(), "Default");
    assert_eq!(parsed.server().as_str(), "edge.example.com");
    assert_eq!(parsed.port().get(), 443);
    assert_eq!(parsed.credential().authentication(), Some(AUTHENTICATION));
    assert_eq!(parsed.credential().obfuscation(), None);
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

    let credential_ref = CredentialRef::new("keychain://nodes/hysteria2-auth").unwrap();
    let node = parsed
        .into_proxy_node(Uuid::parse_str(NODE_ID).unwrap(), credential_ref)
        .unwrap();

    assert_eq!(node.protocol_type, ProxyProtocol::Hysteria2);
    assert_eq!(node.server.as_str(), "edge.example.com");
    assert_eq!(node.transport, None);
    assert!(matches!(node.tls, Some(TlsConfig::Tls { .. })));
    assert!(node.udp_enabled);
}

#[test]
fn parses_userpass_ipv6_and_v2rayn_tls_fields() {
    let parsed = Hysteria2Parser
        .parse(
            "hy2://alice:p%40ss@[2001:db8::1]:8443/?sni=cdn.example.com\
             &insecure=1&alpn=h3%2Ch2&fp=chrome#IPv6%20QUIC",
        )
        .unwrap();

    assert_eq!(parsed.name().as_str(), "IPv6 QUIC");
    assert_eq!(parsed.server().as_str(), "2001:db8::1");
    assert_eq!(parsed.port().get(), 8443);
    assert_eq!(parsed.credential().authentication(), Some("alice:p@ss"));
    assert_eq!(
        parsed.tls(),
        Some(&TlsConfig::Tls {
            server_name: Some("cdn.example.com".to_owned()),
            allow_insecure: true,
            alpn: vec!["h3".to_owned(), "h2".to_owned()],
            fingerprint: Some("chrome".to_owned()),
            pinned_sha256: None,
        })
    );
}

#[test]
fn parses_salamander_obfuscation_without_authentication() {
    let parsed = Hysteria2Parser
        .parse(&format!(
            "hysteria2://example.com:443?obfs=salamander\
             &obfs-password={OBFS_PASSWORD}"
        ))
        .unwrap();

    assert_eq!(parsed.credential().authentication(), None);
    let obfuscation = parsed.credential().obfuscation().unwrap();
    assert_eq!(obfuscation.method(), Hysteria2ObfuscationMethod::Salamander);
    assert_eq!(obfuscation.password(), OBFS_PASSWORD);
    assert_eq!(obfuscation.min_packet_size(), None);
    assert_eq!(obfuscation.max_packet_size(), None);

    let parsed_debug = format!("{parsed:?}");
    let obfuscation_debug = format!("{obfuscation:?}");
    for output in [parsed_debug, obfuscation_debug] {
        assert!(output.contains("[REDACTED]"));
        assert!(!output.contains(OBFS_PASSWORD));
    }
}

#[test]
fn parses_gecko_obfuscation_and_v2rayn_packet_sizes() {
    let parsed = Hysteria2Parser
        .parse(
            "hysteria2://password@example.com:443?obfs=gecko\
             &obfs-password=gecko%3Asecret&minPacketSize=512&maxPacketSize=1200",
        )
        .unwrap();
    let obfuscation = parsed.credential().obfuscation().unwrap();

    assert_eq!(obfuscation.method(), Hysteria2ObfuscationMethod::Gecko);
    assert_eq!(obfuscation.password(), "gecko:secret");
    assert_eq!(obfuscation.min_packet_size(), Some(512));
    assert_eq!(obfuscation.max_packet_size(), Some(1200));
}

#[test]
fn redacts_authentication_and_uses_endpoint_as_the_default_name() {
    let parsed = Hysteria2Parser
        .parse("hysteria2://private-auth@[2001:db8::9]:9443#")
        .unwrap();

    assert_eq!(parsed.name().as_str(), "[2001:db8::9]:9443");
    let credential_debug = format!("{:?}", parsed.credential());
    let parsed_debug = format!("{parsed:?}");
    for output in [credential_debug, parsed_debug] {
        assert!(output.contains("[REDACTED]"));
        assert!(!output.contains("private-auth"));
    }
}

#[test]
fn rejects_invalid_or_incomplete_obfuscation_settings() {
    let unsupported = parse_error("?obfs=xor&obfs-password=password");
    let missing_password = parse_error("?obfs=salamander");
    let orphan_password = parse_error("?obfs-password=password");
    let salamander_size = parse_error("?obfs=salamander&obfs-password=password&minPacketSize=512");

    assert!(matches!(
        &unsupported,
        Hysteria2ParseError::UnsupportedObfuscation { value } if value == "xor"
    ));
    assert!(matches!(
        missing_password,
        Hysteria2ParseError::MissingObfuscationPassword
    ));
    assert!(matches!(
        orphan_password,
        Hysteria2ParseError::UnexpectedObfuscationPassword
    ));
    assert!(matches!(
        salamander_size,
        Hysteria2ParseError::UnexpectedGeckoPacketSize
    ));
}

#[test]
fn validates_gecko_packet_sizes() {
    let invalid = parse_error("?obfs=gecko&obfs-password=password&minPacketSize=large");
    let zero = parse_error("?obfs=gecko&obfs-password=password&maxPacketSize=0");
    let reversed =
        parse_error("?obfs=gecko&obfs-password=password&minPacketSize=1300&maxPacketSize=1200");

    assert!(matches!(
        &invalid,
        Hysteria2ParseError::InvalidPacketSize { name, value }
            if *name == "minPacketSize" && value == "large"
    ));
    assert!(matches!(
        &zero,
        Hysteria2ParseError::InvalidPacketSize { name, value }
            if *name == "maxPacketSize" && value == "0"
    ));
    assert!(matches!(
        reversed,
        Hysteria2ParseError::InvalidPacketSizeRange
    ));
}

#[test]
fn rejects_unrepresentable_port_hopping_and_ech() {
    let authority_hopping = Hysteria2Parser
        .parse("hysteria2://password@example.com:443,8443-8450")
        .unwrap_err();
    let mport = parse_error("?mport=443%2C8443-8450");
    let ech = parse_error("?ech=ZWNoLWNvbmZpZw%3D%3D");

    assert!(matches!(
        authority_hopping,
        Hysteria2ParseError::UnsupportedPortHopping
    ));
    assert!(matches!(mport, Hysteria2ParseError::UnsupportedPortHopping));
    assert!(matches!(ech, Hysteria2ParseError::UnsupportedEch));
}

#[test]
fn keeps_the_certificate_pin_from_either_spelling() {
    const DIGEST: &str = "6ff212bbab490b686b06209c6074865f9340f4c0f9c4aa7d34d568c2a2cebe73";

    for parameter in [
        // The Hysteria2 spelling, printed with colons as share links carry it.
        "?pinSHA256=6F%3AF2%3A12%3ABB%3AAB%3A49%3A0B%3A68%3A6B%3A06%3A20%3A9C%3A60%3A74%3A86%3A5F%3A93%3A40%3AF4%3AC0%3AF9%3AC4%3AAA%3A7D%3A34%3AD5%3A68%3AC2%3AA2%3ACE%3ABE%3A73",
        // The abbreviation v2rayN writes.
        &format!("?pcs={DIGEST}"),
    ] {
        let node = Hysteria2Parser
            .parse(&format!("hysteria2://password@example.com:443{parameter}"))
            .unwrap();

        let Some(TlsConfig::Tls { pinned_sha256, .. }) = node.tls() else {
            panic!("Hysteria2 always carries plain TLS");
        };
        assert_eq!(
            pinned_sha256.as_ref().map(CertificatePin::as_str),
            Some(DIGEST)
        );
    }
}

#[test]
fn rejects_a_pin_that_is_not_a_digest_and_two_that_disagree() {
    assert!(matches!(
        parse_error("?pinSHA256=AA%3ABB%3ACC"),
        Hysteria2ParseError::InvalidCertificatePin { .. }
    ));

    let disagreeing = "?pinSHA256=6ff212bbab490b686b06209c6074865f9340f4c0f9c4aa7d34d568c2a2cebe73&pcs=0000000000000000000000000000000000000000000000000000000000000000";
    assert!(matches!(
        parse_error(disagreeing),
        Hysteria2ParseError::ConflictingCertificatePins
    ));
}

#[test]
fn rejects_invalid_endpoint_path_boolean_percent_and_query_fields() {
    let cases = [
        "trojan://password@example.com:443".to_owned(),
        "hysteria2+realm://token@example.com/realm".to_owned(),
        "hysteria2://password@".to_owned(),
        "hysteria2://password@example.com:0".to_owned(),
        "hysteria2://password@example.com:443/not-allowed".to_owned(),
        "hysteria2://password@example.com:443#bad%ZZtag".to_owned(),
        "hysteria2://password@example.com:443?insecure=true".to_owned(),
        "hysteria2://password@example.com:443?sni=a&sni=b".to_owned(),
        "hysteria2://password@example.com:443?unknown=value".to_owned(),
    ];
    let errors = cases.map(|uri| Hysteria2Parser.parse(&uri).unwrap_err());

    assert!(matches!(errors[0], Hysteria2ParseError::UnsupportedScheme));
    assert!(matches!(errors[1], Hysteria2ParseError::UnsupportedScheme));
    assert!(matches!(
        errors[2],
        Hysteria2ParseError::MalformedUrl { .. }
    ));
    assert!(matches!(
        errors[3],
        Hysteria2ParseError::InvalidPort { port: 0 }
    ));
    assert!(matches!(errors[4], Hysteria2ParseError::UnexpectedPath));
    assert!(matches!(
        &errors[5],
        Hysteria2ParseError::InvalidUri {
            source: VlessParseError::InvalidPercentEncoding
        }
    ));
    assert!(matches!(
        &errors[6],
        Hysteria2ParseError::InvalidBoolean { name, value }
            if *name == "insecure" && value == "true"
    ));
    assert!(matches!(
        &errors[7],
        Hysteria2ParseError::InvalidUri {
            source: VlessParseError::DuplicateParameter { name }
        } if name == "sni"
    ));
    assert!(matches!(
        &errors[8],
        Hysteria2ParseError::InvalidUri {
            source: VlessParseError::UnsupportedParameter { name }
        } if name == "unknown"
    ));
}

fn parse_error(query: &str) -> Hysteria2ParseError {
    Hysteria2Parser
        .parse(&format!("hysteria2://password@example.com:443{query}"))
        .unwrap_err()
}
