use magies_domain::{
    CertificatePin, CredentialRef, GrpcMode, ProxyNode, ProxyProtocol, TlsConfig, TransportConfig,
};
use magies_profiles::{
    Hysteria2Parser, NodeCredential, OutboundConfigError, ShadowsocksParser,
    SingBoxOutboundConfigGenerator, TrojanParser, VlessParser, VmessParser,
};
use serde_json::json;
use uuid::Uuid;

const USER_ID: &str = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";

#[test]
fn generates_vless_websocket_tls_and_tcp_only_network() {
    let parsed = VlessParser
        .parse(&format!(
            "vless://{USER_ID}@edge.example.com:443?type=ws&security=tls\
             &host=cdn.example.com&path=%2Fproxy&sni=www.example.com\
             &fp=chrome&alpn=h2%2Chttp%2F1.1"
        ))
        .unwrap();
    let mut node = node(ProxyProtocol::Vless);
    node.transport = Some(parsed.transport().clone());
    node.tls = parsed.tls().cloned();
    node.udp_enabled = false;

    let outbound =
        SingBoxOutboundConfigGenerator::generate(&node, NodeCredential::from(parsed.credential()))
            .unwrap();

    assert_eq!(
        outbound.json(),
        &json!({
            "type": "vless",
            "tag": "proxy",
            "server": "edge.example.com",
            "server_port": 443,
            "uuid": USER_ID,
            "network": "tcp",
            "tls": {
                "enabled": true,
                "server_name": "www.example.com",
                "alpn": ["h2", "http/1.1"],
                "utls": { "enabled": true, "fingerprint": "chrome" }
            },
            "transport": {
                "type": "ws",
                "path": "/proxy",
                "headers": { "Host": "cdn.example.com" }
            }
        })
    );
}

#[test]
fn generates_vmess_grpc_reality() {
    let parsed = VmessParser
        .parse(&format!(
            "vmess://{USER_ID}@edge.example.com:443?type=grpc&serviceName=api.v1\
             &mode=gun&security=reality&encryption=chacha20-poly1305\
             &sni=www.example.com&fp=chrome&pbk=example-public-key&sid=abcd"
        ))
        .unwrap();
    let mut node = node(ProxyProtocol::Vmess);
    node.transport = Some(parsed.transport().clone());
    node.tls = parsed.tls().cloned();

    let outbound =
        SingBoxOutboundConfigGenerator::generate(&node, NodeCredential::from(parsed.credential()))
            .unwrap();

    assert_eq!(
        outbound.json(),
        &json!({
            "type": "vmess",
            "tag": "proxy",
            "server": "edge.example.com",
            "server_port": 443,
            "uuid": USER_ID,
            "security": "chacha20-poly1305",
            "tls": {
                "enabled": true,
                "server_name": "www.example.com",
                "utls": { "enabled": true, "fingerprint": "chrome" },
                "reality": {
                    "enabled": true,
                    "public_key": "example-public-key",
                    "short_id": "abcd"
                }
            },
            "transport": { "type": "grpc", "service_name": "api.v1" }
        })
    );
}

#[test]
fn generates_trojan_shadowsocks_and_hysteria2_outbounds() {
    let trojan = TrojanParser
        .parse("trojan://secret%3Apassword@edge.example.com:443")
        .unwrap();
    let mut trojan_node = node(ProxyProtocol::Trojan);
    trojan_node.transport = Some(trojan.transport().clone());
    trojan_node.tls = trojan.tls().cloned();
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(
            &trojan_node,
            NodeCredential::from(trojan.credential())
        )
        .unwrap()
        .json(),
        &json!({
            "type": "trojan",
            "tag": "proxy",
            "server": "edge.example.com",
            "server_port": 443,
            "password": "secret:password",
            "tls": { "enabled": true }
        })
    );

    let shadowsocks = ShadowsocksParser
        .parse("ss://aes-256-gcm:ss-password@edge.example.com:443")
        .unwrap();
    let mut shadowsocks_node = node(ProxyProtocol::Shadowsocks);
    shadowsocks_node.transport = Some(TransportConfig::Tcp);
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(
            &shadowsocks_node,
            NodeCredential::from(shadowsocks.credential())
        )
        .unwrap()
        .json(),
        &json!({
            "type": "shadowsocks",
            "tag": "proxy",
            "server": "edge.example.com",
            "server_port": 443,
            "method": "aes-256-gcm",
            "password": "ss-password"
        })
    );

    let hysteria2 = Hysteria2Parser
        .parse(
            "hy2://hy2-auth@edge.example.com:443?obfs=salamander\
             &obfs-password=obfs-secret&sni=cdn.example.com&insecure=1\
             &alpn=h3&fp=chrome",
        )
        .unwrap();
    let mut hysteria2_node = node(ProxyProtocol::Hysteria2);
    hysteria2_node.transport = None;
    hysteria2_node.tls = hysteria2.tls().cloned();
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(
            &hysteria2_node,
            NodeCredential::from(hysteria2.credential())
        )
        .unwrap()
        .json(),
        &json!({
            "type": "hysteria2",
            "tag": "proxy",
            "server": "edge.example.com",
            "server_port": 443,
            "password": "hy2-auth",
            "obfs": { "type": "salamander", "password": "obfs-secret" },
            "tls": {
                "enabled": true,
                "server_name": "cdn.example.com",
                "insecure": true,
                "alpn": ["h3"],
                "utls": { "enabled": true, "fingerprint": "chrome" }
            }
        })
    );
}

#[test]
fn rejects_disabled_nodes_and_protocol_mismatches() {
    let parsed = VlessParser
        .parse(&format!("vless://{USER_ID}@edge.example.com:443"))
        .unwrap();
    let trojan = TrojanParser
        .parse("trojan://password@edge.example.com:443")
        .unwrap();
    let mut disabled = node(ProxyProtocol::Vless);
    disabled.transport = Some(TransportConfig::Tcp);
    disabled.enabled = false;

    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(
            &disabled,
            NodeCredential::from(parsed.credential())
        ),
        Err(OutboundConfigError::NodeDisabled)
    );

    let mut mismatch = node(ProxyProtocol::Vless);
    mismatch.transport = Some(TransportConfig::Tcp);
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(
            &mismatch,
            NodeCredential::from(trojan.credential())
        ),
        Err(OutboundConfigError::CredentialProtocolMismatch {
            expected: ProxyProtocol::Vless,
            actual: ProxyProtocol::Trojan,
        })
    );
}

#[test]
fn rejects_fields_that_sing_box_cannot_map_losslessly() {
    let encrypted = VlessParser
        .parse(&format!(
            "vless://{USER_ID}@edge.example.com:443?encryption=auto"
        ))
        .unwrap();
    let mut vless_node = node(ProxyProtocol::Vless);
    vless_node.transport = Some(TransportConfig::Tcp);
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(
            &vless_node,
            NodeCredential::from(encrypted.credential())
        ),
        Err(OutboundConfigError::UnsupportedVlessEncryption)
    );

    let flowed = TrojanParser
        .parse("trojan://password@edge.example.com:443?flow=xtls-rprx-vision")
        .unwrap();
    let mut trojan_node = node(ProxyProtocol::Trojan);
    trojan_node.transport = Some(TransportConfig::Tcp);
    trojan_node.tls = flowed.tls().cloned();
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(
            &trojan_node,
            NodeCredential::from(flowed.credential())
        ),
        Err(OutboundConfigError::UnsupportedTrojanFlow)
    );

    let grpc = VlessParser
        .parse(&format!(
            "vless://{USER_ID}@edge.example.com:443?type=grpc\
             &serviceName=api&mode=multi"
        ))
        .unwrap();
    vless_node.transport = Some(grpc.transport().clone());
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(
            &vless_node,
            NodeCredential::from(grpc.credential())
        ),
        Err(OutboundConfigError::UnsupportedGrpcMode {
            mode: GrpcMode::Multi
        })
    );

    let authority = VlessParser
        .parse(&format!(
            "vless://{USER_ID}@edge.example.com:443?type=grpc\
             &serviceName=api&mode=gun&authority=cdn.example.com"
        ))
        .unwrap();
    vless_node.transport = Some(authority.transport().clone());
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(
            &vless_node,
            NodeCredential::from(authority.credential())
        ),
        Err(OutboundConfigError::UnsupportedGrpcAuthority)
    );

    let spider = VlessParser
        .parse(&format!(
            "vless://{USER_ID}@edge.example.com:443?security=reality\
             &fp=chrome&pbk=example-public-key&spx=%2Fprobe"
        ))
        .unwrap();
    vless_node.transport = Some(TransportConfig::Tcp);
    vless_node.tls = spider.tls().cloned();
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(
            &vless_node,
            NodeCredential::from(spider.credential())
        ),
        Err(OutboundConfigError::UnsupportedRealitySpiderX)
    );

    let packet_sizes = Hysteria2Parser
        .parse(
            "hy2://password@edge.example.com:443?obfs=gecko\
             &obfs-password=secret&minPacketSize=512&maxPacketSize=1200",
        )
        .unwrap();
    let mut hysteria2_node = node(ProxyProtocol::Hysteria2);
    hysteria2_node.transport = None;
    hysteria2_node.tls = packet_sizes.tls().cloned();
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(
            &hysteria2_node,
            NodeCredential::from(packet_sizes.credential())
        ),
        Err(OutboundConfigError::UnsupportedHysteria2PacketSizes)
    );
}

#[test]
fn validates_protocol_transport_and_tls_invariants() {
    let vless = VlessParser
        .parse(&format!("vless://{USER_ID}@edge.example.com:443"))
        .unwrap();
    let missing_transport = node(ProxyProtocol::Vless);
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(
            &missing_transport,
            NodeCredential::from(vless.credential())
        ),
        Err(OutboundConfigError::MissingTransport {
            protocol: ProxyProtocol::Vless
        })
    );

    let shadowsocks = ShadowsocksParser
        .parse("ss://aes-256-gcm:password@edge.example.com:443")
        .unwrap();
    let mut invalid_shadowsocks = node(ProxyProtocol::Shadowsocks);
    invalid_shadowsocks.transport = Some(TransportConfig::WebSocket {
        path: "/".to_owned(),
        host: None,
    });
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(
            &invalid_shadowsocks,
            NodeCredential::from(shadowsocks.credential())
        ),
        Err(OutboundConfigError::UnsupportedTransport {
            protocol: ProxyProtocol::Shadowsocks
        })
    );
    invalid_shadowsocks.transport = Some(TransportConfig::Tcp);
    invalid_shadowsocks.tls = Some(TlsConfig::Tls {
        server_name: None,
        allow_insecure: false,
        alpn: Vec::new(),
        fingerprint: None,
        pinned_sha256: None,
    });
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(
            &invalid_shadowsocks,
            NodeCredential::from(shadowsocks.credential())
        ),
        Err(OutboundConfigError::UnsupportedTls {
            protocol: ProxyProtocol::Shadowsocks
        })
    );

    let hysteria2 = Hysteria2Parser
        .parse("hy2://password@edge.example.com:443")
        .unwrap();
    let mut missing_tls = node(ProxyProtocol::Hysteria2);
    missing_tls.transport = None;
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(
            &missing_tls,
            NodeCredential::from(hysteria2.credential())
        ),
        Err(OutboundConfigError::MissingTls {
            protocol: ProxyProtocol::Hysteria2
        })
    );
}

#[test]
fn redacts_credentials_from_debug_output() {
    let parsed = TrojanParser
        .parse("trojan://top-secret@edge.example.com:443")
        .unwrap();
    let mut node = node(ProxyProtocol::Trojan);
    node.transport = Some(TransportConfig::Tcp);
    node.tls = parsed.tls().cloned();
    let credential = NodeCredential::from(parsed.credential());
    let outbound = SingBoxOutboundConfigGenerator::generate(&node, credential).unwrap();

    for output in [format!("{credential:?}"), format!("{outbound:?}")] {
        assert!(output.contains("[REDACTED]"));
        assert!(!output.contains("top-secret"));
    }
}

fn node(protocol: ProxyProtocol) -> ProxyNode {
    ProxyNode::new(
        Uuid::parse_str("018f78b5-08ee-7caa-94f3-1d5d781aba22").unwrap(),
        "Test node",
        protocol,
        "edge.example.com",
        443,
        Some(CredentialRef::new("keychain://nodes/test").unwrap()),
    )
    .unwrap()
}

#[test]
fn a_pinned_certificate_is_refused_rather_than_dropped() {
    let vless = VlessParser
        .parse(&format!("vless://{USER_ID}@edge.example.com:443"))
        .unwrap();
    let mut pinned = node(ProxyProtocol::Vless);
    pinned.transport = Some(TransportConfig::Tcp);
    pinned.tls = Some(TlsConfig::Tls {
        server_name: Some("edge.example.com".to_owned()),
        allow_insecure: false,
        alpn: Vec::new(),
        fingerprint: None,
        pinned_sha256: Some(
            CertificatePin::new("6ff212bbab490b686b06209c6074865f9340f4c0f9c4aa7d34d568c2a2cebe73")
                .unwrap(),
        ),
    });

    // sing-box 1.13.18 has no SHA-256 pin: it offers a full certificate
    // instead. Dropping the pin would leave the connection verified only by the
    // CA chain the user pinned *around*, a weaker check than they asked for.
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(&pinned, NodeCredential::from(vless.credential())),
        Err(OutboundConfigError::CertificatePinUnsupported)
    );
}
