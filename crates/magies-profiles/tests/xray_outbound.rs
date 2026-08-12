//! Covers the Xray outbound generator: the schema shape for each protocol,
//! stream settings, TLS and Reality, and the refusals it shares with the
//! capability matrix.

use std::num::NonZeroU16;

use magies_domain::{
    CoreType, CredentialRef, GrpcMode, ProxyNode, ProxyProtocol, TlsConfig, TransportConfig,
};
use magies_platform::CpuArchitecture;
use magies_profiles::{
    CoreCapabilityMatrix, CoreRequirements, ManualCredentialDraft, ManualNodeDraft,
    SingBoxOutboundConfigGenerator, StoredNodeCredential, VmessSecurity,
    XrayOutboundConfigGenerator, XrayOutboundError,
};
use serde_json::{Value, json};
use uuid::Uuid;

const USER_ID: &str = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";

fn user_id() -> Uuid {
    Uuid::parse_str(USER_ID).unwrap()
}

/// Builds a node and credential the same way the app does, through the manual
/// draft, so the generator is fed exactly what a real node looks like.
fn build_node(
    credential: ManualCredentialDraft,
    tls: Option<TlsConfig>,
) -> (ProxyNode, StoredNodeCredential) {
    ManualNodeDraft {
        name: "Tokyo".to_owned(),
        server: "edge.example.com".to_owned(),
        port: 443,
        udp_enabled: true,
        transport: None,
        tls,
        credential,
    }
    .build(Uuid::new_v4(), CredentialRef::new("node/test").unwrap())
    .unwrap()
}

fn vless() -> ManualCredentialDraft {
    ManualCredentialDraft::Vless {
        user_id: user_id(),
        flow: None,
    }
}

fn generate(node: &ProxyNode, credential: &StoredNodeCredential) -> Value {
    XrayOutboundConfigGenerator::generate(node, credential.as_node_credential())
        .unwrap()
        .json()
        .clone()
}

fn plain_tls() -> TlsConfig {
    TlsConfig::Tls {
        server_name: Some("sni.example.com".to_owned()),
        allow_insecure: false,
        alpn: vec!["h2".to_owned()],
        fingerprint: Some("chrome".to_owned()),
    }
}

#[test]
fn a_vless_node_becomes_a_vnext_outbound() {
    let (node, credential) = build_node(vless(), None);

    let outbound = generate(&node, &credential);

    assert_eq!(outbound["protocol"], "vless");
    assert_eq!(outbound["tag"], "proxy");
    let user = &outbound["settings"]["vnext"][0];
    assert_eq!(user["address"], "edge.example.com");
    assert_eq!(user["port"], 443);
    assert_eq!(user["users"][0]["id"], USER_ID);
    assert_eq!(user["users"][0]["encryption"], "none");
    assert_eq!(outbound["streamSettings"]["network"], "tcp");
}

#[test]
fn a_vless_flow_reaches_the_user_entry() {
    let (node, credential) = build_node(
        ManualCredentialDraft::Vless {
            user_id: user_id(),
            flow: Some("xtls-rprx-vision".to_owned()),
        },
        None,
    );

    let outbound = generate(&node, &credential);

    assert_eq!(
        outbound["settings"]["vnext"][0]["users"][0]["flow"],
        "xtls-rprx-vision"
    );
}

#[test]
fn a_vmess_node_carries_its_security_and_alter_id() {
    let (node, credential) = build_node(
        ManualCredentialDraft::Vmess {
            user_id: user_id(),
            security: VmessSecurity::Chacha20Poly1305,
            alter_id: 4,
        },
        None,
    );

    let outbound = generate(&node, &credential);

    assert_eq!(outbound["protocol"], "vmess");
    let user = &outbound["settings"]["vnext"][0]["users"][0];
    assert_eq!(user["id"], USER_ID);
    assert_eq!(user["security"], "chacha20-poly1305");
    assert_eq!(user["alterId"], 4);
}

#[test]
fn trojan_and_shadowsocks_use_the_servers_array() {
    let (trojan_node, trojan_credential) = build_node(
        ManualCredentialDraft::Trojan {
            password: "hunter2".to_owned(),
        },
        None,
    );
    let trojan = generate(&trojan_node, &trojan_credential);
    assert_eq!(trojan["protocol"], "trojan");
    assert_eq!(trojan["settings"]["servers"][0]["password"], "hunter2");
    assert_eq!(
        trojan["settings"]["servers"][0]["address"],
        "edge.example.com"
    );

    let (ss_node, ss_credential) = build_node(
        ManualCredentialDraft::Shadowsocks {
            method: "aes-256-gcm".to_owned(),
            password: "hunter2".to_owned(),
        },
        None,
    );
    let shadowsocks = generate(&ss_node, &ss_credential);
    assert_eq!(shadowsocks["protocol"], "shadowsocks");
    assert_eq!(
        shadowsocks["settings"]["servers"][0]["method"],
        "aes-256-gcm"
    );
    assert_eq!(shadowsocks["settings"]["servers"][0]["password"], "hunter2");
}

#[test]
fn a_websocket_transport_becomes_ws_settings() {
    let (mut node, credential) = build_node(vless(), None);
    node.transport = Some(TransportConfig::WebSocket {
        path: "/ray".to_owned(),
        host: Some("cdn.example.com".to_owned()),
    });

    let outbound = generate(&node, &credential);

    let stream = &outbound["streamSettings"];
    assert_eq!(stream["network"], "ws");
    assert_eq!(stream["wsSettings"]["path"], "/ray");
    assert_eq!(stream["wsSettings"]["headers"]["Host"], "cdn.example.com");
}

#[test]
fn a_websocket_without_a_host_omits_the_header() {
    let (mut node, credential) = build_node(vless(), None);
    node.transport = Some(TransportConfig::WebSocket {
        path: "/ray".to_owned(),
        host: None,
    });

    let outbound = generate(&node, &credential);

    assert!(outbound["streamSettings"]["wsSettings"]["headers"].is_null());
}

#[test]
fn grpc_multi_mode_maps_to_the_multi_mode_flag() {
    let (mut node, credential) = build_node(vless(), None);
    node.transport = Some(TransportConfig::Grpc {
        service_name: "tunnel".to_owned(),
        mode: GrpcMode::Multi,
        authority: Some("authority.example.com".to_owned()),
    });

    let outbound = generate(&node, &credential);

    let settings = &outbound["streamSettings"]["grpcSettings"];
    assert_eq!(outbound["streamSettings"]["network"], "grpc");
    assert_eq!(settings["serviceName"], "tunnel");
    assert_eq!(settings["multiMode"], true);
    assert_eq!(settings["authority"], "authority.example.com");
}

#[test]
fn grpc_gun_mode_is_not_multi_mode() {
    let (mut node, credential) = build_node(vless(), None);
    node.transport = Some(TransportConfig::Grpc {
        service_name: "tunnel".to_owned(),
        mode: GrpcMode::Gun,
        authority: None,
    });

    let outbound = generate(&node, &credential);

    assert_eq!(
        outbound["streamSettings"]["grpcSettings"]["multiMode"],
        false
    );
    assert!(outbound["streamSettings"]["grpcSettings"]["authority"].is_null());
}

#[test]
fn tls_becomes_tls_settings() {
    let (node, credential) = build_node(vless(), Some(plain_tls()));

    let outbound = generate(&node, &credential);

    let stream = &outbound["streamSettings"];
    assert_eq!(stream["security"], "tls");
    assert_eq!(stream["tlsSettings"]["serverName"], "sni.example.com");
    assert_eq!(stream["tlsSettings"]["allowInsecure"], false);
    assert_eq!(stream["tlsSettings"]["alpn"], json!(["h2"]));
    assert_eq!(stream["tlsSettings"]["fingerprint"], "chrome");
}

#[test]
fn reality_becomes_reality_settings() {
    let (node, credential) = build_node(
        vless(),
        Some(TlsConfig::Reality {
            server_name: "edge.example.com".to_owned(),
            public_key: "public-key".to_owned(),
            short_id: Some("ab12".to_owned()),
            fingerprint: Some("chrome".to_owned()),
            alpn: vec!["h2".to_owned()],
            spider_x: Some("/path".to_owned()),
        }),
    );

    let outbound = generate(&node, &credential);

    let stream = &outbound["streamSettings"];
    assert_eq!(stream["security"], "reality");
    assert_eq!(stream["realitySettings"]["serverName"], "edge.example.com");
    assert_eq!(stream["realitySettings"]["publicKey"], "public-key");
    assert_eq!(stream["realitySettings"]["shortId"], "ab12");
    assert_eq!(stream["realitySettings"]["spiderX"], "/path");
    // Reality has no ALPN of its own, so it is dropped rather than forwarded.
    assert!(stream["realitySettings"]["alpn"].is_null());
    assert!(stream["tlsSettings"].is_null());
}

#[test]
fn a_node_without_tls_carries_no_security() {
    let (node, credential) = build_node(vless(), None);

    let outbound = generate(&node, &credential);

    assert!(outbound["streamSettings"]["security"].is_null());
}

#[test]
fn a_disabled_node_is_refused() {
    let (mut node, credential) = build_node(vless(), None);
    node.enabled = false;

    assert_eq!(
        XrayOutboundConfigGenerator::generate(&node, credential.as_node_credential()).unwrap_err(),
        XrayOutboundError::NodeDisabled
    );
}

#[test]
fn a_credential_from_another_protocol_is_refused() {
    let (node, _) = build_node(vless(), None);
    let (_, trojan_credential) = build_node(
        ManualCredentialDraft::Trojan {
            password: "hunter2".to_owned(),
        },
        None,
    );

    assert_eq!(
        XrayOutboundConfigGenerator::generate(&node, trojan_credential.as_node_credential())
            .unwrap_err(),
        XrayOutboundError::CredentialProtocolMismatch {
            expected: ProxyProtocol::Vless,
            actual: ProxyProtocol::Trojan,
        }
    );
}

#[test]
fn a_node_without_a_transport_is_refused() {
    let (mut node, credential) = build_node(vless(), None);
    node.transport = None;

    assert_eq!(
        XrayOutboundConfigGenerator::generate(&node, credential.as_node_credential()).unwrap_err(),
        XrayOutboundError::MissingTransport {
            protocol: ProxyProtocol::Vless,
        }
    );
}

#[test]
fn hysteria2_is_refused_the_same_way_the_matrix_refuses_it() {
    // The manual draft never produces a Hysteria2 node with a transport, so the
    // node is assembled directly to reach the generator's own guard.
    let (mut node, _) = build_node(vless(), None);
    node.protocol_type = ProxyProtocol::Hysteria2;
    node.port = NonZeroU16::new(8443).unwrap();

    let (_, credential) = hysteria2_node();

    assert_eq!(
        XrayOutboundConfigGenerator::generate(&node, credential.as_node_credential()).unwrap_err(),
        XrayOutboundError::ProtocolUnsupported {
            protocol: ProxyProtocol::Hysteria2,
        }
    );
}

fn hysteria2_node() -> (ProxyNode, StoredNodeCredential) {
    ManualNodeDraft {
        name: "Tokyo".to_owned(),
        server: "edge.example.com".to_owned(),
        port: 8443,
        udp_enabled: true,
        transport: None,
        tls: Some(plain_tls()),
        credential: ManualCredentialDraft::Hysteria2 {
            authentication: Some("token".to_owned()),
            obfuscation: None,
        },
    }
    .build(Uuid::new_v4(), CredentialRef::new("node/hy2").unwrap())
    .unwrap()
}

/// The generator and the matrix must never disagree: anything the matrix says
/// Xray can run has to generate, and anything it rejects has to be refused.
#[test]
fn the_generator_agrees_with_the_capability_matrix() {
    let cases = [
        (ProxyProtocol::Vless, vless()),
        (
            ProxyProtocol::Vmess,
            ManualCredentialDraft::Vmess {
                user_id: user_id(),
                security: VmessSecurity::Auto,
                alter_id: 0,
            },
        ),
        (
            ProxyProtocol::Trojan,
            ManualCredentialDraft::Trojan {
                password: "hunter2".to_owned(),
            },
        ),
        (
            ProxyProtocol::Shadowsocks,
            ManualCredentialDraft::Shadowsocks {
                method: "aes-256-gcm".to_owned(),
                password: "hunter2".to_owned(),
            },
        ),
    ];

    for (protocol, draft) in cases {
        let allowed = CoreCapabilityMatrix::supports(
            CoreType::Xray,
            CoreRequirements::new(protocol, false, CpuArchitecture::Aarch64),
        );
        assert!(allowed, "matrix should allow {protocol:?} on Xray");

        let (node, credential) = build_node(draft, None);
        assert!(
            XrayOutboundConfigGenerator::generate(&node, credential.as_node_credential()).is_ok(),
            "{protocol:?} is allowed by the matrix but the generator refused it"
        );
    }

    // The one protocol the matrix excludes.
    assert!(!CoreCapabilityMatrix::supports(
        CoreType::Xray,
        CoreRequirements::new(ProxyProtocol::Hysteria2, false, CpuArchitecture::Aarch64)
    ));
}

/// Both generators must accept the same node, so switching Cores never needs a
/// different node model.
#[test]
fn a_node_that_generates_for_sing_box_also_generates_for_xray() {
    let (node, credential) = build_node(vless(), Some(plain_tls()));

    assert!(
        SingBoxOutboundConfigGenerator::generate(&node, credential.as_node_credential()).is_ok()
    );
    assert!(XrayOutboundConfigGenerator::generate(&node, credential.as_node_credential()).is_ok());
}
