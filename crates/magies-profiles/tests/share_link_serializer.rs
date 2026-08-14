//! Covers turning a stored node back into a sharing URI.
//!
//! The property that matters is the round trip: a link this produces has to
//! parse back into the same node and credential, because the point of exporting
//! is that another client — or this one — can read it again.

use base64::Engine as _;
use magies_domain::{CredentialRef, GrpcMode, ProxyProtocol, TlsConfig, TransportConfig};
use magies_profiles::{
    ShareLinkParser, ShareLinkSerializer, ShareLinkSerializerError, StoredNodeCredential,
};
use uuid::Uuid;

const NODE_ID: &str = "018f78b5-08ee-7caa-94f3-1d5d781aba22";
const USER_ID: &str = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";

/// Every link shape the parsers accept, so the serializer is exercised against
/// the transports, TLS layers and credential fields that actually occur.
#[expect(
    clippy::too_many_lines,
    reason = "a flat list of fixture links; splitting it would not make any single case clearer"
)]
fn links() -> Vec<(&'static str, String)> {
    vec![
        (
            "tuic",
            format!(
                "tuic://{USER_ID}:hunter2@edge.example.com:443?sni=cdn.example.com\
                 &congestion_control=bbr&udp_relay_mode=native#Tokyo"
            ),
        ),
        (
            "wireguard",
            "wireguard://cHJpdmF0ZS1rZXk%3D@edge.example.com:51821\
             ?publickey=cGVlci1wdWJsaWMta2V5&address=10.0.0.2/32,fd00::1/128\
             &mtu=1420&presharedkey=cHNr&reserved=1,2,3#Tokyo"
                .to_owned(),
        ),
        (
            "vless-httpupgrade",
            format!(
                "vless://{USER_ID}@edge.example.com:443?type=httpupgrade\
                 &path=%2Fupgrade&host=cdn.example.com#Tokyo"
            ),
        ),
        (
            "vless-xhttp",
            format!(
                "vless://{USER_ID}@edge.example.com:443?type=xhttp&path=%2Fx\
                 &host=cdn.example.com&mode=packet-up#Tokyo"
            ),
        ),
        (
            "vless-tcp",
            format!("vless://{USER_ID}@edge.example.com:443"),
        ),
        (
            "vless-ws-tls",
            format!(
                "vless://{USER_ID}@edge.example.com:443?type=ws&security=tls\
                 &host=cdn.example.com&path=%2Fproxy&sni=www.example.com&fp=chrome#Tokyo"
            ),
        ),
        (
            "vless-grpc-reality",
            format!(
                "vless://{USER_ID}@edge.example.com:443?type=grpc&serviceName=api\
                 &security=reality&sni=www.example.com&pbk=public-key&sid=ab&fp=chrome"
            ),
        ),
        (
            "vless-flow",
            format!("vless://{USER_ID}@edge.example.com:443?flow=xtls-rprx-vision&security=tls"),
        ),
        (
            "vless-kcp",
            format!(
                "vless://{USER_ID}@edge.example.com:443?type=kcp&mtu=1350&tti=50\
                 &uplinkCapacity=5&downlinkCapacity=20&congestion=1\
                 &headerType=wechat-video&seed=s3cr3t#KCP"
            ),
        ),
        (
            "vmess-tcp",
            format!("vmess://{USER_ID}@edge.example.com:443"),
        ),
        (
            "vmess-ws-tls",
            format!(
                "vmess://{USER_ID}@edge.example.com:443?type=ws&path=%2Fws\
                 &security=tls&sni=www.example.com#Osaka"
            ),
        ),
        // The Base64 document form, the only one carrying alterId.
        ("vmess-legacy", legacy_vmess()),
        (
            "trojan",
            "trojan://hunter2@edge.example.com:443?sni=www.example.com#Trojan".to_owned(),
        ),
        (
            "trojan-kcp",
            "trojan://hunter2@edge.example.com:443?type=kcp&headerType=dtls\
             &seed=s3cr3t&sni=www.example.com#TrojanKCP"
                .to_owned(),
        ),
        (
            "shadowsocks",
            "ss://aes-256-gcm:hunter2@edge.example.com:8388#SS".to_owned(),
        ),
        (
            "hysteria2",
            "hysteria2://hunter2@edge.example.com:5555?sni=www.example.com&insecure=1#HK"
                .to_owned(),
        ),
        (
            "hysteria2-pinned",
            "hysteria2://hunter2@edge.example.com:5555?sni=www.example.com\
             &pinSHA256=6ff212bbab490b686b06209c6074865f9340f4c0f9c4aa7d34d568c2a2cebe73#Pinned"
                .to_owned(),
        ),
        (
            "socks-anonymous",
            "socks://edge.example.com:1080#Anon".to_owned(),
        ),
        (
            "socks-auth",
            "socks5://alice:hunter2@edge.example.com:1080#Auth".to_owned(),
        ),
        (
            "http-plain",
            "http://edge.example.com:8080#Plain".to_owned(),
        ),
        (
            "http-username-only",
            "http://alice@edge.example.com:8080#Alice".to_owned(),
        ),
        (
            "https-tls",
            "https://alice:hunter2@edge.example.com#Secure".to_owned(),
        ),
        (
            "anytls-tls",
            "anytls://hunter2@edge.example.com:443?sni=www.example.com&insecure=1#AnyTLS"
                .to_owned(),
        ),
        (
            "anytls-reality",
            "anytls://hunter2@edge.example.com:443?security=reality\
             &sni=www.example.com&pbk=public-key&sid=ab&fp=chrome#AnyTLS-Reality"
                .to_owned(),
        ),
        (
            "naive-http2",
            "naive://alice:hunter2@edge.example.com:443?sni=www.example.com#Naive".to_owned(),
        ),
        (
            "naive-quic",
            "naive+quic://alice:hunter2@edge.example.com:443?\
             congestion_control=bbr&sni=www.example.com#Naive-QUIC"
                .to_owned(),
        ),
        (
            "naive-https-scheme",
            "naive+https://edge.example.com#Anon".to_owned(),
        ),
    ]
}

/// A `VMess` sharing document with a non-zero `alterId`, which the URI form has no
/// field for.
fn legacy_vmess() -> String {
    let document = format!(
        r#"{{"v":"2","ps":"Legacy","add":"edge.example.com","port":"443",
           "id":"{USER_ID}","aid":"4","scy":"aes-128-gcm","net":"ws",
           "path":"/ws","tls":"tls","sni":"www.example.com"}}"#
    );
    format!(
        "vmess://{}",
        base64::engine::general_purpose::STANDARD.encode(document)
    )
}

fn parse(link: &str) -> (magies_domain::ProxyNode, StoredNodeCredential) {
    ShareLinkParser
        .parse(
            link,
            Uuid::parse_str(NODE_ID).unwrap(),
            CredentialRef::new("keychain://nodes/test").unwrap(),
        )
        .unwrap()
        .into_parts()
}

#[test]
fn every_supported_link_survives_a_round_trip() {
    for (name, link) in links() {
        let (node, credential) = parse(&link);

        let exported = ShareLinkSerializer::serialize(&node, &credential)
            .unwrap_or_else(|error| panic!("{name} could not be exported: {error}"));
        let (reparsed, reparsed_credential) = parse(&exported);

        assert_eq!(reparsed.protocol_type, node.protocol_type, "{name}");
        assert_eq!(reparsed.name, node.name, "{name}");
        assert_eq!(reparsed.server, node.server, "{name}");
        assert_eq!(reparsed.port, node.port, "{name}");
        assert_eq!(reparsed.transport, node.transport, "{name}");
        assert_eq!(reparsed.tls, node.tls, "{name}");
        assert_eq!(reparsed_credential, credential, "{name}");
    }
}

#[test]
fn the_exported_link_carries_the_node_name_as_its_fragment() {
    let (node, credential) = parse(&format!("vless://{USER_ID}@edge.example.com:443#东京 01"));

    let exported = ShareLinkSerializer::serialize(&node, &credential).unwrap();

    // The name is percent-encoded, and comes back decoded.
    assert!(exported.contains('#'), "no fragment in {exported}");
    let (reparsed, _) = parse(&exported);
    assert_eq!(reparsed.name.as_str(), "东京 01");
}

#[test]
fn a_credential_that_does_not_match_the_node_is_refused() {
    let (node, _) = parse(&format!("vless://{USER_ID}@edge.example.com:443"));
    let (_, shadowsocks) = parse("ss://aes-256-gcm:hunter2@edge.example.com:8388");

    // Exporting a link built from the wrong secret would produce something that
    // looks valid and cannot connect.
    assert_eq!(
        ShareLinkSerializer::serialize(&node, &shadowsocks),
        Err(ShareLinkSerializerError::CredentialProtocolMismatch {
            expected: ProxyProtocol::Vless,
            actual: ProxyProtocol::Shadowsocks,
        })
    );
}

#[test]
fn the_scheme_matches_what_the_parser_claims() {
    for (link, scheme) in [
        (
            format!("vless://{USER_ID}@edge.example.com:443"),
            "vless://",
        ),
        (
            format!("vmess://{USER_ID}@edge.example.com:443"),
            "vmess://",
        ),
        ("trojan://p@edge.example.com:443".to_owned(), "trojan://"),
        (
            "ss://aes-256-gcm:p@edge.example.com:8388".to_owned(),
            "ss://",
        ),
        (
            "hysteria2://p@edge.example.com:5555".to_owned(),
            "hysteria2://",
        ),
        ("socks://edge.example.com:1080".to_owned(), "socks://"),
        ("http://edge.example.com:8080".to_owned(), "http://"),
        ("https://edge.example.com".to_owned(), "https://"),
        ("anytls://p@edge.example.com:443".to_owned(), "anytls://"),
    ] {
        let (node, credential) = parse(&link);

        let exported = ShareLinkSerializer::serialize(&node, &credential).unwrap();

        assert!(
            exported.starts_with(scheme),
            "{exported} does not start with {scheme}"
        );
    }
}

#[test]
fn a_vmess_node_the_document_cannot_hold_is_refused() {
    // Reality has no field in the VMess sharing document, and the parser that
    // reads the document back rejects `pcs`. Writing either would produce a link
    // that reparses into a different node.
    let (mut node, credential) = parse(&format!(
        "vmess://{USER_ID}@edge.example.com:443?type=grpc&serviceName=api"
    ));
    node.tls = Some(TlsConfig::Reality {
        server_name: "www.example.com".to_owned(),
        public_key: "public-key".to_owned(),
        short_id: None,
        fingerprint: None,
        alpn: Vec::new(),
        spider_x: None,
    });

    assert_eq!(
        ShareLinkSerializer::serialize(&node, &credential),
        Err(ShareLinkSerializerError::UnrepresentableVmessTls)
    );
}

#[test]
fn a_vmess_grpc_mode_the_document_cannot_hold_is_refused() {
    let (mut node, credential) = parse(&format!(
        "vmess://{USER_ID}@edge.example.com:443?type=grpc&serviceName=api"
    ));
    node.transport = Some(TransportConfig::Grpc {
        service_name: "api".to_owned(),
        mode: GrpcMode::Multi,
        authority: None,
    });

    // The document has one field for the service name and none for the mode, so
    // multi mode would silently come back as gun.
    assert_eq!(
        ShareLinkSerializer::serialize(&node, &credential),
        Err(ShareLinkSerializerError::UnrepresentableVmessTransport)
    );
}

#[test]
fn a_vmess_alter_id_survives_the_export() {
    let (node, credential) = parse(&legacy_vmess());

    let exported = ShareLinkSerializer::serialize(&node, &credential).unwrap();
    let (_, reparsed) = parse(&exported);

    // The URI form has no field for this, which is why VMess uses the document.
    let StoredNodeCredential::Vmess(vmess) = reparsed else {
        panic!("a VMess link must parse to a VMess credential");
    };
    assert_eq!(vmess.alter_id(), 4);
}

#[test]
fn custom_nodes_cannot_be_exported_as_sharing_links() {
    use magies_domain::{NodeName, ProxyNode, ServerAddress};
    use magies_profiles::CustomCredential;
    use std::num::NonZeroU16;

    let node = ProxyNode {
        id: Uuid::parse_str(NODE_ID).unwrap(),
        name: NodeName::new("Custom").unwrap(),
        protocol_type: ProxyProtocol::Custom,
        server: ServerAddress::new("127.0.0.1").unwrap(),
        port: NonZeroU16::new(443).unwrap(),
        credential_ref: CredentialRef::new("node/custom").unwrap(),
        transport: None,
        tls: None,
        udp_enabled: false,
        subscription_id: None,
        group_id: None,
        latency_ms: None,
        last_tested_at: None,
        enabled: true,
        xray_finalmask_json: None,
    };
    let credential = StoredNodeCredential::Custom(CustomCredential {
        core: magies_domain::CoreType::SingBox,
        document: r#"{"outbounds":[]}"#.to_owned(),
    });

    assert_eq!(
        ShareLinkSerializer::serialize(&node, &credential),
        Err(ShareLinkSerializerError::UnrepresentableCustomNode)
    );
}
