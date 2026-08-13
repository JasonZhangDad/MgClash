//! Covers the manual node creation form: field validation, the transport and
//! TLS rules each protocol imposes, and the camelCase JSON contract the desktop
//! UI sends.

use magies_domain::{
    CredentialRef, GrpcMode, NodeModelError, ProxyNode, ProxyProtocol, TlsConfig, TransportConfig,
};
use magies_profiles::{
    Hysteria2ObfuscationMethod, ManualCredentialDraft, ManualNodeDraft, ManualNodeDraftError,
    ManualObfuscationDraft, SingBoxOutboundConfigGenerator, StoredNodeCredential, VmessSecurity,
};
use uuid::Uuid;

const USER_ID: &str = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";
const NODE_ID: &str = "018f78b5-08ee-7caa-94f3-1d5d781aba22";

fn node_id() -> Uuid {
    Uuid::parse_str(NODE_ID).unwrap()
}

fn user_id() -> Uuid {
    Uuid::parse_str(USER_ID).unwrap()
}

fn credential_ref() -> CredentialRef {
    CredentialRef::new("node/manual").unwrap()
}

/// A valid draft that individual tests mutate to isolate one rule at a time.
fn draft(credential: ManualCredentialDraft) -> ManualNodeDraft {
    ManualNodeDraft {
        name: "Tokyo".to_owned(),
        server: "edge.example.com".to_owned(),
        port: 443,
        udp_enabled: true,
        transport: None,
        tls: None,
        credential,
    }
}

fn vless() -> ManualCredentialDraft {
    ManualCredentialDraft::Vless {
        user_id: user_id(),
        flow: None,
    }
}

fn hysteria2() -> ManualCredentialDraft {
    ManualCredentialDraft::Hysteria2 {
        authentication: Some("token".to_owned()),
        obfuscation: None,
    }
}

/// Hysteria2 is the one protocol that requires TLS up front.
fn hysteria2_draft() -> ManualNodeDraft {
    let mut value = draft(hysteria2());
    value.tls = Some(plain_tls());
    value
}

fn anytls() -> ManualCredentialDraft {
    ManualCredentialDraft::AnyTls {
        password: "hunter2".to_owned(),
    }
}

/// `AnyTLS` is the other protocol that requires TLS (or Reality) up front.
fn anytls_draft() -> ManualNodeDraft {
    let mut value = draft(anytls());
    value.tls = Some(plain_tls());
    value
}

fn wireguard() -> ManualCredentialDraft {
    ManualCredentialDraft::WireGuard {
        private_key: "private-key".to_owned(),
        peer_public_key: "peer-public-key".to_owned(),
        pre_shared_key: None,
        local_address: vec!["10.0.0.2/32".to_owned()],
        mtu: None,
        reserved: None,
    }
}

fn plain_tls() -> TlsConfig {
    TlsConfig::Tls {
        server_name: Some("edge.example.com".to_owned()),
        allow_insecure: false,
        alpn: vec!["h3".to_owned()],
        fingerprint: None,
        pinned_sha256: None,
    }
}

fn build(
    draft: ManualNodeDraft,
) -> Result<(ProxyNode, StoredNodeCredential), ManualNodeDraftError> {
    draft.build(node_id(), credential_ref())
}

#[test]
fn builds_a_vless_node_with_prd_defaults() {
    let (node, credential) = build(draft(vless())).unwrap();

    assert_eq!(node.id, node_id());
    assert_eq!(node.name.as_str(), "Tokyo");
    assert_eq!(node.protocol_type, ProxyProtocol::Vless);
    assert_eq!(node.server.as_str(), "edge.example.com");
    assert_eq!(node.port.get(), 443);
    assert_eq!(node.credential_ref.as_str(), "node/manual");
    assert!(node.udp_enabled);
    assert!(node.enabled);
    assert_eq!(node.transport, Some(TransportConfig::Tcp));
    assert!(node.tls.is_none());
    assert!(node.subscription_id.is_none());
    assert!(node.group_id.is_none());

    let StoredNodeCredential::Vless(credential) = credential else {
        panic!("expected a VLESS credential");
    };
    assert_eq!(credential.user_id(), user_id());
    assert_eq!(credential.encryption(), "none");
    assert_eq!(credential.flow(), None);
}

#[test]
fn builds_a_vmess_node() {
    let (node, credential) = build(draft(ManualCredentialDraft::Vmess {
        user_id: user_id(),
        security: VmessSecurity::Aes128Gcm,
        alter_id: 0,
    }))
    .unwrap();

    assert_eq!(node.protocol_type, ProxyProtocol::Vmess);
    let StoredNodeCredential::Vmess(credential) = credential else {
        panic!("expected a VMess credential");
    };
    assert_eq!(credential.user_id(), user_id());
    assert_eq!(credential.security(), VmessSecurity::Aes128Gcm);
    assert_eq!(credential.alter_id(), 0);
}

#[test]
fn builds_a_trojan_node() {
    let (node, credential) = build(draft(ManualCredentialDraft::Trojan {
        password: "hunter2".to_owned(),
    }))
    .unwrap();

    assert_eq!(node.protocol_type, ProxyProtocol::Trojan);
    let StoredNodeCredential::Trojan(credential) = credential else {
        panic!("expected a Trojan credential");
    };
    assert_eq!(credential.password(), "hunter2");
    // The outbound generator refuses any Trojan flow, so the form stores none.
    assert_eq!(credential.flow(), None);
}

#[test]
fn builds_a_shadowsocks_node() {
    let (node, credential) = build(draft(ManualCredentialDraft::Shadowsocks {
        method: "aes-256-gcm".to_owned(),
        password: "hunter2".to_owned(),
    }))
    .unwrap();

    assert_eq!(node.protocol_type, ProxyProtocol::Shadowsocks);
    assert_eq!(node.transport, Some(TransportConfig::Tcp));
    let StoredNodeCredential::Shadowsocks(credential) = credential else {
        panic!("expected a Shadowsocks credential");
    };
    assert_eq!(credential.method(), "aes-256-gcm");
    assert_eq!(credential.password(), "hunter2");
}

#[test]
fn builds_a_hysteria2_node_with_obfuscation() {
    let mut value = hysteria2_draft();
    value.credential = ManualCredentialDraft::Hysteria2 {
        authentication: Some("token".to_owned()),
        obfuscation: Some(ManualObfuscationDraft {
            method: Hysteria2ObfuscationMethod::Salamander,
            password: "obfs-secret".to_owned(),
        }),
    };

    let (node, credential) = build(value).unwrap();

    assert_eq!(node.protocol_type, ProxyProtocol::Hysteria2);
    // Hysteria2 carries its own QUIC transport.
    assert!(node.transport.is_none());
    let StoredNodeCredential::Hysteria2(credential) = credential else {
        panic!("expected a Hysteria2 credential");
    };
    assert_eq!(credential.authentication(), Some("token"));
    let obfuscation = credential.obfuscation().unwrap();
    assert_eq!(obfuscation.method(), Hysteria2ObfuscationMethod::Salamander);
    assert_eq!(obfuscation.password(), "obfs-secret");
    assert_eq!(obfuscation.min_packet_size(), None);
    assert_eq!(obfuscation.max_packet_size(), None);
}

#[test]
fn builds_a_socks_node() {
    let (node, credential) = build(draft(ManualCredentialDraft::Socks {
        username: Some("alice".to_owned()),
        password: Some("hunter2".to_owned()),
    }))
    .unwrap();

    assert_eq!(node.protocol_type, ProxyProtocol::Socks);
    assert_eq!(node.transport, Some(TransportConfig::Tcp));
    assert!(node.tls.is_none());
    let StoredNodeCredential::Socks(credential) = credential else {
        panic!("expected a SOCKS credential");
    };
    assert_eq!(credential.username(), Some("alice"));
    assert_eq!(credential.password(), Some("hunter2"));
}

#[test]
fn builds_an_anonymous_socks_node() {
    let (_, credential) = build(draft(ManualCredentialDraft::Socks {
        username: None,
        password: None,
    }))
    .unwrap();

    let StoredNodeCredential::Socks(credential) = credential else {
        panic!("expected a SOCKS credential");
    };
    assert_eq!(credential.username(), None);
    assert_eq!(credential.password(), None);
}

#[test]
fn builds_an_http_node_with_tls() {
    let mut value = draft(ManualCredentialDraft::Http {
        username: Some("alice".to_owned()),
        password: Some("hunter2".to_owned()),
    });
    value.tls = Some(plain_tls());

    let (node, credential) = build(value).unwrap();

    assert_eq!(node.protocol_type, ProxyProtocol::Http);
    assert_eq!(node.transport, Some(TransportConfig::Tcp));
    assert!(matches!(node.tls, Some(TlsConfig::Tls { .. })));
    let StoredNodeCredential::Http(credential) = credential else {
        panic!("expected an HTTP credential");
    };
    assert_eq!(credential.username(), Some("alice"));
}

#[test]
fn builds_a_wireguard_node() {
    let (node, credential) = build(draft(ManualCredentialDraft::WireGuard {
        private_key: "private-key".to_owned(),
        peer_public_key: "peer-public-key".to_owned(),
        pre_shared_key: Some("psk".to_owned()),
        local_address: vec!["10.0.0.2/32".to_owned(), "fd00::1/128".to_owned()],
        mtu: Some(1420),
        reserved: Some([1, 2, 3]),
    }))
    .unwrap();

    assert_eq!(node.protocol_type, ProxyProtocol::WireGuard);
    // WireGuard is its own tunnel: no stream transport, no TLS.
    assert!(node.transport.is_none());
    assert!(node.tls.is_none());
    let StoredNodeCredential::WireGuard(credential) = credential else {
        panic!("expected a WireGuard credential");
    };
    assert_eq!(credential.private_key(), "private-key");
    assert_eq!(credential.peer_public_key(), "peer-public-key");
    assert_eq!(credential.pre_shared_key(), Some("psk"));
    assert_eq!(
        credential.local_address(),
        &["10.0.0.2/32".to_owned(), "fd00::1/128".to_owned()]
    );
    assert_eq!(credential.mtu(), Some(1420));
    assert_eq!(credential.reserved(), Some([1, 2, 3]));
}

#[test]
fn builds_an_anytls_node_with_tls() {
    let (node, credential) = build(anytls_draft()).unwrap();

    assert_eq!(node.protocol_type, ProxyProtocol::AnyTls);
    // AnyTLS is TLS from the first byte: no stream transport of its own.
    assert!(node.transport.is_none());
    assert!(matches!(node.tls, Some(TlsConfig::Tls { .. })));
    let StoredNodeCredential::AnyTls(credential) = credential else {
        panic!("expected an AnyTLS credential");
    };
    assert_eq!(credential.password(), "hunter2");
}

#[test]
fn builds_an_anytls_node_with_reality() {
    let mut value = draft(anytls());
    value.tls = Some(TlsConfig::Reality {
        server_name: "edge.example.com".to_owned(),
        public_key: "key".to_owned(),
        short_id: None,
        fingerprint: None,
        alpn: Vec::new(),
        spider_x: None,
    });

    let (node, _) = build(value).unwrap();

    assert!(matches!(node.tls, Some(TlsConfig::Reality { .. })));
}

#[test]
fn rejects_a_transport_for_anytls() {
    let mut value = anytls_draft();
    value.transport = Some(TransportConfig::Tcp);

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::AnyTlsRejectsTransport
    );
}

#[test]
fn rejects_anytls_without_tls_or_reality() {
    let mut value = anytls_draft();
    value.tls = None;

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::AnyTlsRequiresTls
    );
}

#[test]
fn rejects_an_empty_anytls_password() {
    let mut value = anytls_draft();
    value.credential = ManualCredentialDraft::AnyTls {
        password: "  ".to_owned(),
    };

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::MissingAnyTlsPassword
    );
}

fn naive() -> ManualCredentialDraft {
    ManualCredentialDraft::Naive {
        username: Some("alice".to_owned()),
        password: Some("hunter2".to_owned()),
        quic: false,
        quic_congestion_control: None,
    }
}

fn naive_draft() -> ManualNodeDraft {
    let mut value = draft(naive());
    value.transport = None;
    value.tls = Some(TlsConfig::Tls {
        server_name: Some("cdn.example.com".to_owned()),
        allow_insecure: false,
        alpn: Vec::new(),
        fingerprint: None,
        pinned_sha256: None,
    });
    value
}

#[test]
fn builds_a_naive_node_with_quic() {
    let mut value = naive_draft();
    value.credential = ManualCredentialDraft::Naive {
        username: Some("alice".to_owned()),
        password: Some("hunter2".to_owned()),
        quic: true,
        quic_congestion_control: Some(magies_profiles::NaiveCongestionControl::Bbr),
    };
    let (node, credential) = build(value).unwrap();

    assert_eq!(node.protocol_type, ProxyProtocol::Naive);
    assert!(node.transport.is_none());
    assert!(matches!(node.tls, Some(TlsConfig::Tls { .. })));
    let StoredNodeCredential::Naive(credential) = credential else {
        panic!("expected a Naive credential");
    };
    assert!(credential.quic());
    assert_eq!(
        credential.quic_congestion_control(),
        Some(magies_profiles::NaiveCongestionControl::Bbr)
    );
}

#[test]
fn rejects_a_transport_for_naive() {
    let mut value = naive_draft();
    value.transport = Some(TransportConfig::Tcp);

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::NaiveRejectsTransport
    );
}

#[test]
fn rejects_naive_without_tls() {
    let mut value = naive_draft();
    value.tls = None;

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::NaiveRequiresTls
    );
}

#[test]
fn rejects_naive_reality_and_tls_extras() {
    let mut reality = naive_draft();
    reality.tls = Some(TlsConfig::Reality {
        server_name: "edge.example.com".to_owned(),
        public_key: "key".to_owned(),
        short_id: None,
        fingerprint: None,
        alpn: Vec::new(),
        spider_x: None,
    });
    assert_eq!(
        build(reality).unwrap_err(),
        ManualNodeDraftError::NaiveRejectsReality
    );

    let mut extras = naive_draft();
    extras.tls = Some(TlsConfig::Tls {
        server_name: None,
        allow_insecure: true,
        alpn: Vec::new(),
        fingerprint: None,
        pinned_sha256: None,
    });
    assert_eq!(
        build(extras).unwrap_err(),
        ManualNodeDraftError::NaiveRejectsTlsExtras
    );
}

#[test]
fn rejects_a_transport_for_wireguard() {
    let mut value = draft(wireguard());
    value.transport = Some(TransportConfig::Tcp);

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::WireGuardRejectsTransport
    );
}

#[test]
fn rejects_tls_for_wireguard() {
    let mut value = draft(wireguard());
    value.tls = Some(plain_tls());

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::WireGuardRejectsTls
    );
}

#[test]
fn rejects_empty_wireguard_fields() {
    let missing_private_key = draft(ManualCredentialDraft::WireGuard {
        private_key: "  ".to_owned(),
        peer_public_key: "peer-public-key".to_owned(),
        pre_shared_key: None,
        local_address: vec!["10.0.0.2/32".to_owned()],
        mtu: None,
        reserved: None,
    });
    assert_eq!(
        build(missing_private_key).unwrap_err(),
        ManualNodeDraftError::MissingWireGuardPrivateKey
    );

    let missing_peer_key = draft(ManualCredentialDraft::WireGuard {
        private_key: "private-key".to_owned(),
        peer_public_key: String::new(),
        pre_shared_key: None,
        local_address: vec!["10.0.0.2/32".to_owned()],
        mtu: None,
        reserved: None,
    });
    assert_eq!(
        build(missing_peer_key).unwrap_err(),
        ManualNodeDraftError::MissingWireGuardPeerPublicKey
    );

    let missing_address = draft(ManualCredentialDraft::WireGuard {
        private_key: "private-key".to_owned(),
        peer_public_key: "peer-public-key".to_owned(),
        pre_shared_key: None,
        local_address: vec!["  ".to_owned()],
        mtu: None,
        reserved: None,
    });
    assert_eq!(
        build(missing_address).unwrap_err(),
        ManualNodeDraftError::MissingWireGuardLocalAddress
    );
}

#[test]
fn rejects_socks_tls() {
    let mut value = draft(ManualCredentialDraft::Socks {
        username: None,
        password: None,
    });
    value.tls = Some(plain_tls());

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::SocksRejectsTls
    );
}

#[test]
fn rejects_a_non_tcp_transport_for_socks_and_http() {
    let mut socks = draft(ManualCredentialDraft::Socks {
        username: None,
        password: None,
    });
    socks.transport = Some(TransportConfig::WebSocket {
        path: "/ray".to_owned(),
        host: None,
    });
    assert_eq!(
        build(socks).unwrap_err(),
        ManualNodeDraftError::SocksRequiresTcpTransport
    );

    let mut http = draft(ManualCredentialDraft::Http {
        username: None,
        password: None,
    });
    http.transport = Some(TransportConfig::WebSocket {
        path: "/ray".to_owned(),
        host: None,
    });
    assert_eq!(
        build(http).unwrap_err(),
        ManualNodeDraftError::HttpRequiresTcpTransport
    );
}

#[test]
fn rejects_reality_for_http() {
    let mut value = draft(ManualCredentialDraft::Http {
        username: None,
        password: None,
    });
    value.tls = Some(TlsConfig::Reality {
        server_name: "edge.example.com".to_owned(),
        public_key: "key".to_owned(),
        short_id: None,
        fingerprint: None,
        alpn: Vec::new(),
        spider_x: None,
    });

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::HttpRejectsReality
    );
}

#[test]
fn rejects_a_password_without_a_username_for_socks_and_http() {
    let socks = draft(ManualCredentialDraft::Socks {
        username: None,
        password: Some("hunter2".to_owned()),
    });
    assert_eq!(
        build(socks).unwrap_err(),
        ManualNodeDraftError::SocksPasswordRequiresUsername
    );

    let http = draft(ManualCredentialDraft::Http {
        username: None,
        password: Some("hunter2".to_owned()),
    });
    assert_eq!(
        build(http).unwrap_err(),
        ManualNodeDraftError::HttpPasswordRequiresUsername
    );
}

/// The guarantee that matters: anything the form accepts must survive outbound
/// generation, otherwise the node only fails once the user hits connect.
#[test]
fn every_accepted_draft_generates_a_sing_box_outbound() {
    let drafts = [
        draft(vless()),
        draft(ManualCredentialDraft::Vmess {
            user_id: user_id(),
            security: VmessSecurity::Auto,
            alter_id: 0,
        }),
        draft(ManualCredentialDraft::Trojan {
            password: "hunter2".to_owned(),
        }),
        draft(ManualCredentialDraft::Shadowsocks {
            method: "aes-256-gcm".to_owned(),
            password: "hunter2".to_owned(),
        }),
        hysteria2_draft(),
        anytls_draft(),
        naive_draft(),
        draft(ManualCredentialDraft::Socks {
            username: Some("alice".to_owned()),
            password: Some("hunter2".to_owned()),
        }),
        draft(ManualCredentialDraft::Http {
            username: None,
            password: None,
        }),
        draft(wireguard()),
    ];

    for value in drafts {
        let (node, credential) = build(value).unwrap();
        let protocol = node.protocol_type;
        SingBoxOutboundConfigGenerator::generate(&node, credential.as_node_credential())
            .unwrap_or_else(|error| panic!("{protocol:?} outbound generation failed: {error}"));
    }
}

#[test]
fn generates_an_outbound_for_a_websocket_tls_node() {
    let mut value = draft(vless());
    value.transport = Some(TransportConfig::WebSocket {
        path: "/ray".to_owned(),
        host: Some("cdn.example.com".to_owned()),
    });
    value.tls = Some(plain_tls());

    let (node, credential) = build(value).unwrap();

    assert!(
        SingBoxOutboundConfigGenerator::generate(&node, credential.as_node_credential()).is_ok()
    );
}

#[test]
fn generates_an_outbound_for_a_grpc_node() {
    let mut value = draft(vless());
    value.transport = Some(TransportConfig::Grpc {
        service_name: "tunnel".to_owned(),
        mode: GrpcMode::Gun,
        authority: None,
    });

    let (node, credential) = build(value).unwrap();

    assert!(
        SingBoxOutboundConfigGenerator::generate(&node, credential.as_node_credential()).is_ok()
    );
}

#[test]
fn builds_a_vless_node_with_kcp_transport() {
    let mut value = draft(vless());
    value.transport = Some(TransportConfig::Kcp {
        mtu: Some(1350),
        tti: Some(50),
        uplink_capacity: None,
        downlink_capacity: None,
        congestion: false,
        header_type: Some("none".to_owned()),
        seed: Some("s3cr3t".to_owned()),
    });

    let (node, credential) = build(value).unwrap();

    assert_eq!(
        node.transport,
        Some(TransportConfig::Kcp {
            mtu: Some(1350),
            tti: Some(50),
            uplink_capacity: None,
            downlink_capacity: None,
            congestion: false,
            header_type: Some("none".to_owned()),
            seed: Some("s3cr3t".to_owned()),
        })
    );

    // The pinned sing-box build has no mKCP transport at all; the capability
    // matrix (covered separately) is what routes this node to Xray instead.
    assert_eq!(
        SingBoxOutboundConfigGenerator::generate(&node, credential.as_node_credential()),
        Err(magies_profiles::OutboundConfigError::KcpUnsupported)
    );
}

#[test]
fn carries_transport_and_tls_onto_the_node() {
    let mut value = draft(vless());
    value.transport = Some(TransportConfig::WebSocket {
        path: "/ray".to_owned(),
        host: Some("cdn.example.com".to_owned()),
    });
    value.tls = Some(plain_tls());

    let (node, _) = build(value).unwrap();

    assert_eq!(
        node.transport,
        Some(TransportConfig::WebSocket {
            path: "/ray".to_owned(),
            host: Some("cdn.example.com".to_owned()),
        })
    );
    assert!(matches!(node.tls, Some(TlsConfig::Tls { .. })));
}

#[test]
fn honours_a_disabled_udp_toggle() {
    let mut value = draft(vless());
    value.udp_enabled = false;

    let (node, _) = build(value).unwrap();

    assert!(!node.udp_enabled);
}

#[test]
fn trims_surrounding_whitespace_from_shared_fields() {
    let mut value = draft(vless());
    value.name = "  Tokyo  ".to_owned();
    value.server = "  edge.example.com  ".to_owned();

    let (node, _) = build(value).unwrap();

    assert_eq!(node.name.as_str(), "Tokyo");
    assert_eq!(node.server.as_str(), "edge.example.com");
}

#[test]
fn treats_a_blank_optional_field_as_absent() {
    let (_, credential) = build(draft(ManualCredentialDraft::Vless {
        user_id: user_id(),
        flow: Some("   ".to_owned()),
    }))
    .unwrap();

    let StoredNodeCredential::Vless(credential) = credential else {
        panic!("expected a VLESS credential");
    };
    assert_eq!(credential.flow(), None);
}

#[test]
fn rejects_an_empty_name() {
    let mut value = draft(vless());
    value.name = "   ".to_owned();

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::Node(NodeModelError::EmptyName)
    );
}

#[test]
fn rejects_an_empty_server() {
    let mut value = draft(vless());
    value.server = String::new();

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::Node(NodeModelError::EmptyServer)
    );
}

#[test]
fn rejects_a_port_outside_the_valid_range() {
    for port in [0, 65_536] {
        let mut value = draft(vless());
        value.port = port;

        assert_eq!(
            build(value).unwrap_err(),
            ManualNodeDraftError::Node(NodeModelError::InvalidPort { port })
        );
    }
}

#[test]
fn rejects_an_empty_trojan_password() {
    let value = draft(ManualCredentialDraft::Trojan {
        password: "  ".to_owned(),
    });

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::MissingTrojanPassword
    );
}

#[test]
fn rejects_empty_shadowsocks_credentials() {
    let missing_method = draft(ManualCredentialDraft::Shadowsocks {
        method: String::new(),
        password: "hunter2".to_owned(),
    });
    assert_eq!(
        build(missing_method).unwrap_err(),
        ManualNodeDraftError::MissingShadowsocksMethod
    );

    let missing_password = draft(ManualCredentialDraft::Shadowsocks {
        method: "aes-256-gcm".to_owned(),
        password: String::new(),
    });
    assert_eq!(
        build(missing_password).unwrap_err(),
        ManualNodeDraftError::MissingShadowsocksPassword
    );
}

#[test]
fn rejects_an_unsupported_shadowsocks_method() {
    let value = draft(ManualCredentialDraft::Shadowsocks {
        method: "des-cbc".to_owned(),
        password: "hunter2".to_owned(),
    });

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::UnsupportedShadowsocksMethod {
            value: "des-cbc".to_owned()
        }
    );
}

#[test]
fn rejects_a_non_tcp_transport_for_shadowsocks() {
    let mut value = draft(ManualCredentialDraft::Shadowsocks {
        method: "aes-256-gcm".to_owned(),
        password: "hunter2".to_owned(),
    });
    value.transport = Some(TransportConfig::WebSocket {
        path: "/ray".to_owned(),
        host: None,
    });

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::ShadowsocksRequiresTcpTransport
    );
}

#[test]
fn rejects_a_transport_for_hysteria2() {
    let mut value = hysteria2_draft();
    value.transport = Some(TransportConfig::Tcp);

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::Hysteria2RejectsTransport
    );
}

#[test]
fn rejects_hysteria2_without_standard_tls() {
    let mut missing = hysteria2_draft();
    missing.tls = None;
    assert_eq!(
        build(missing).unwrap_err(),
        ManualNodeDraftError::Hysteria2RequiresTls
    );

    let mut reality = hysteria2_draft();
    reality.tls = Some(TlsConfig::Reality {
        server_name: "edge.example.com".to_owned(),
        public_key: "key".to_owned(),
        short_id: None,
        fingerprint: None,
        alpn: Vec::new(),
        spider_x: None,
    });
    assert_eq!(
        build(reality).unwrap_err(),
        ManualNodeDraftError::Hysteria2RequiresTls
    );
}

#[test]
fn rejects_obfuscation_without_a_password() {
    let mut value = hysteria2_draft();
    value.credential = ManualCredentialDraft::Hysteria2 {
        authentication: Some("token".to_owned()),
        obfuscation: Some(ManualObfuscationDraft {
            method: Hysteria2ObfuscationMethod::Salamander,
            password: String::new(),
        }),
    };

    assert_eq!(
        build(value).unwrap_err(),
        ManualNodeDraftError::MissingObfuscationPassword
    );
}

#[test]
fn deserializes_the_camel_case_payload_the_ui_sends() {
    let payload = serde_json::json!({
        "name": "Tokyo",
        "server": "edge.example.com",
        "port": 8443,
        "udpEnabled": false,
        "transport": { "type": "websocket", "path": "/ray", "host": null },
        "tls": {
            "type": "tls",
            "serverName": "edge.example.com",
            "allowInsecure": false,
            "alpn": ["h2"],
            "fingerprint": null
        },
        "credential": {
            "protocol": "trojan",
            "password": "hunter2"
        }
    });

    let value: ManualNodeDraft = serde_json::from_value(payload).unwrap();
    let (node, credential) = build(value).unwrap();

    assert_eq!(node.protocol_type, ProxyProtocol::Trojan);
    assert_eq!(node.port.get(), 8443);
    assert!(!node.udp_enabled);
    assert!(matches!(
        node.transport,
        Some(TransportConfig::WebSocket { .. })
    ));
    assert!(matches!(credential, StoredNodeCredential::Trojan(_)));
}

#[test]
fn defaults_optional_json_fields_when_omitted() {
    let payload = serde_json::json!({
        "name": "Tokyo",
        "server": "edge.example.com",
        "port": 443,
        "credential": { "protocol": "vless", "userId": USER_ID }
    });

    let value: ManualNodeDraft = serde_json::from_value(payload).unwrap();
    let (node, _) = build(value).unwrap();

    assert!(node.udp_enabled);
    assert_eq!(node.transport, Some(TransportConfig::Tcp));
    assert!(node.tls.is_none());
}

#[test]
fn builds_a_custom_node_with_placeholder_endpoint() {
    let document = r#"{"inbounds":[],"outbounds":[{"type":"direct"}]}"#;
    let value = ManualNodeDraft {
        name: "My JSON".to_owned(),
        server: "ignored.example.com".to_owned(),
        port: 8443,
        udp_enabled: true,
        transport: None,
        tls: None,
        credential: ManualCredentialDraft::Custom {
            core: magies_domain::CoreType::SingBox,
            document: document.to_owned(),
        },
    };
    let (node, credential) = value.build(node_id(), credential_ref()).unwrap();
    assert_eq!(node.protocol_type, ProxyProtocol::Custom);
    assert_eq!(node.server.as_str(), "127.0.0.1");
    assert_eq!(node.port.get(), 443);
    assert!(node.transport.is_none());
    assert!(node.tls.is_none());
    let StoredNodeCredential::Custom(stored) = credential else {
        panic!("expected a Custom credential");
    };
    assert_eq!(stored.core(), magies_domain::CoreType::SingBox);
    assert_eq!(stored.document(), document);
}

#[test]
fn rejects_custom_without_document() {
    let value = ManualNodeDraft {
        name: "Empty".to_owned(),
        server: "127.0.0.1".to_owned(),
        port: 443,
        udp_enabled: false,
        transport: None,
        tls: None,
        credential: ManualCredentialDraft::Custom {
            core: magies_domain::CoreType::Xray,
            document: "   ".to_owned(),
        },
    };
    assert_eq!(
        value.build(node_id(), credential_ref()).unwrap_err(),
        ManualNodeDraftError::MissingCustomDocument
    );
}

#[test]
fn rejects_custom_with_invalid_json() {
    let value = ManualNodeDraft {
        name: "Bad".to_owned(),
        server: "127.0.0.1".to_owned(),
        port: 443,
        udp_enabled: false,
        transport: None,
        tls: None,
        credential: ManualCredentialDraft::Custom {
            core: magies_domain::CoreType::SingBox,
            document: "{not json".to_owned(),
        },
    };
    assert!(matches!(
        value.build(node_id(), credential_ref()).unwrap_err(),
        ManualNodeDraftError::InvalidCustomDocument { .. }
    ));
}

#[test]
fn rejects_custom_with_non_object_json() {
    let value = ManualNodeDraft {
        name: "Array".to_owned(),
        server: "127.0.0.1".to_owned(),
        port: 443,
        udp_enabled: false,
        transport: None,
        tls: None,
        credential: ManualCredentialDraft::Custom {
            core: magies_domain::CoreType::SingBox,
            document: "[]".to_owned(),
        },
    };
    assert_eq!(
        value.build(node_id(), credential_ref()).unwrap_err(),
        ManualNodeDraftError::InvalidCustomDocumentNotObject
    );
}

#[test]
fn rejects_transport_and_tls_for_custom() {
    let mut value = ManualNodeDraft {
        name: "Custom".to_owned(),
        server: "127.0.0.1".to_owned(),
        port: 443,
        udp_enabled: false,
        transport: None,
        tls: None,
        credential: ManualCredentialDraft::Custom {
            core: magies_domain::CoreType::SingBox,
            document: r#"{"outbounds":[]}"#.to_owned(),
        },
    };
    value.transport = Some(TransportConfig::Tcp);
    assert_eq!(
        value.clone().build(node_id(), credential_ref()).unwrap_err(),
        ManualNodeDraftError::CustomRejectsTransport
    );
    value.transport = None;
    value.tls = Some(plain_tls());
    assert_eq!(
        value.build(node_id(), credential_ref()).unwrap_err(),
        ManualNodeDraftError::CustomRejectsTls
    );
}

#[test]
fn redacts_the_draft_debug_output() {
    let value = draft(ManualCredentialDraft::Trojan {
        password: "hunter2".to_owned(),
    });

    assert_eq!(format!("{value:?}"), "ManualNodeDraft([REDACTED])");
}
