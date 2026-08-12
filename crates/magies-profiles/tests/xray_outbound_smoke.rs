//! Feeds generated Xray outbounds to a real Xray binary.
//!
//! This is the only check that the generated schema is actually what Xray
//! expects, and it doubles as the verification of the capability matrix's claim
//! that Xray has no Hysteria2 outbound.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use magies_domain::{CredentialRef, ProxyNode, TlsConfig, TransportConfig};
use magies_profiles::{
    ManualCredentialDraft, ManualNodeDraft, StoredNodeCredential, VmessSecurity,
    XrayOutboundConfigGenerator,
};
use serde_json::{Value, json};
use uuid::Uuid;

const USER_ID: &str = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";

fn binary() -> PathBuf {
    PathBuf::from(
        std::env::var_os("MAGIES_XRAY_BIN").expect("MAGIES_XRAY_BIN must point to official Xray"),
    )
}

fn build_node(
    credential: ManualCredentialDraft,
    transport: Option<TransportConfig>,
    tls: Option<TlsConfig>,
) -> (ProxyNode, StoredNodeCredential) {
    ManualNodeDraft {
        name: "Smoke".to_owned(),
        server: "edge.example.com".to_owned(),
        port: 443,
        udp_enabled: true,
        transport,
        tls,
        credential,
    }
    .build(Uuid::new_v4(), CredentialRef::new("node/smoke").unwrap())
    .unwrap()
}

fn user_id() -> Uuid {
    Uuid::parse_str(USER_ID).unwrap()
}

fn outbound(
    credential: ManualCredentialDraft,
    transport: Option<TransportConfig>,
    tls: Option<TlsConfig>,
) -> Value {
    let (node, credential) = build_node(credential, transport, tls);
    XrayOutboundConfigGenerator::generate(&node, credential.as_node_credential())
        .unwrap()
        .json()
        .clone()
}

/// Runs `xray run -test -c <path>`, returning stdout and stderr together.
fn check(config: &Value, name: &str) -> (bool, String) {
    let path = std::env::temp_dir().join(format!(
        "magies-xray-smoke-{name}-{}.json",
        std::process::id()
    ));
    fs::write(&path, serde_json::to_vec_pretty(config).unwrap()).unwrap();
    let result = run_check(&binary(), &path);
    let _ = fs::remove_file(&path);
    result
}

fn run_check(binary: &Path, path: &Path) -> (bool, String) {
    let output = Command::new(binary)
        .arg("run")
        .arg("-test")
        .arg("-c")
        .arg(path)
        .output()
        .expect("Xray should be runnable");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.success(), text)
}

/// A minimal config Xray will accept, with the generated outbound in place.
fn config_with(outbound: &Value) -> Value {
    json!({
        "log": { "loglevel": "warning" },
        "inbounds": [{
            "protocol": "socks",
            "tag": "socks-in",
            "listen": "127.0.0.1",
            "port": 10808,
            "settings": { "udp": true }
        }],
        "outbounds": [outbound]
    })
}

#[test]
#[ignore = "requires MAGIES_XRAY_BIN pointing to an official Xray build"]
fn official_xray_accepts_every_supported_outbound() {
    let cases: Vec<(&str, Value)> = vec![
        (
            "vless",
            outbound(
                ManualCredentialDraft::Vless {
                    user_id: user_id(),
                    flow: None,
                },
                None,
                None,
            ),
        ),
        (
            "vmess",
            outbound(
                ManualCredentialDraft::Vmess {
                    user_id: user_id(),
                    security: VmessSecurity::Auto,
                    alter_id: 0,
                },
                None,
                None,
            ),
        ),
        (
            "trojan",
            outbound(
                ManualCredentialDraft::Trojan {
                    password: "hunter2".to_owned(),
                },
                None,
                Some(TlsConfig::Tls {
                    server_name: Some("edge.example.com".to_owned()),
                    allow_insecure: false,
                    alpn: Vec::new(),
                    fingerprint: None,
                }),
            ),
        ),
        (
            "shadowsocks",
            outbound(
                ManualCredentialDraft::Shadowsocks {
                    method: "aes-256-gcm".to_owned(),
                    password: "hunter2".to_owned(),
                },
                None,
                None,
            ),
        ),
    ];

    for (name, generated) in cases {
        let (accepted, output) = check(&config_with(&generated), name);
        assert!(accepted, "Xray rejected the {name} outbound:\n{output}");
    }
}

#[test]
#[ignore = "requires MAGIES_XRAY_BIN pointing to an official Xray build"]
fn official_xray_accepts_the_stream_transports() {
    let cases: Vec<(&str, Value)> = vec![
        (
            "websocket",
            outbound(
                ManualCredentialDraft::Vless {
                    user_id: user_id(),
                    flow: None,
                },
                Some(TransportConfig::WebSocket {
                    path: "/ray".to_owned(),
                    host: Some("cdn.example.com".to_owned()),
                }),
                Some(TlsConfig::Tls {
                    server_name: Some("cdn.example.com".to_owned()),
                    allow_insecure: false,
                    alpn: vec!["h2".to_owned()],
                    fingerprint: Some("chrome".to_owned()),
                }),
            ),
        ),
        (
            "grpc",
            outbound(
                ManualCredentialDraft::Vless {
                    user_id: user_id(),
                    flow: None,
                },
                Some(TransportConfig::Grpc {
                    service_name: "tunnel".to_owned(),
                    mode: magies_domain::GrpcMode::Multi,
                    authority: None,
                }),
                None,
            ),
        ),
    ];

    for (name, generated) in cases {
        let (accepted, output) = check(&config_with(&generated), name);
        assert!(accepted, "Xray rejected the {name} transport:\n{output}");
    }
}

#[test]
#[ignore = "requires MAGIES_XRAY_BIN pointing to an official Xray build"]
fn official_xray_accepts_a_reality_outbound() {
    let generated = outbound(
        ManualCredentialDraft::Vless {
            user_id: user_id(),
            flow: Some("xtls-rprx-vision".to_owned()),
        },
        None,
        Some(TlsConfig::Reality {
            server_name: "edge.example.com".to_owned(),
            public_key: "sqPZ1nQz6zXqvCK3wBSMHnvJHUdKrPXY6JmuLHnQ0Cs".to_owned(),
            short_id: Some("ab12".to_owned()),
            fingerprint: Some("chrome".to_owned()),
            alpn: Vec::new(),
            spider_x: Some("/".to_owned()),
        }),
    );

    let (accepted, output) = check(&config_with(&generated), "reality");

    assert!(accepted, "Xray rejected the Reality outbound:\n{output}");
}

/// The capability matrix claims Xray has no Hysteria2 outbound. If a future
/// Xray gains one, this test starts failing and the matrix needs updating.
#[test]
#[ignore = "requires MAGIES_XRAY_BIN pointing to an official Xray build"]
fn official_xray_still_has_no_hysteria2_outbound() {
    let generated = json!({
        "protocol": "hysteria2",
        "tag": "proxy",
        "settings": {
            "servers": [{ "address": "edge.example.com", "port": 8443 }]
        }
    });

    let (accepted, output) = check(&config_with(&generated), "hysteria2");

    assert!(
        !accepted,
        "Xray accepted a hysteria2 outbound, so the capability matrix is wrong:\n{output}"
    );
}
