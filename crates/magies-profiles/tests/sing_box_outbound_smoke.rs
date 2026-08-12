use std::fs;
use std::path::PathBuf;
use std::process::Command;

use magies_domain::{CredentialRef, ProxyNode, ProxyProtocol, TlsConfig, TransportConfig};
use magies_profiles::{
    Hysteria2Parser, NodeCredential, ShadowsocksParser, SingBoxOutboundConfigGenerator,
    TrojanParser, VlessParser, VmessParser,
};
use serde_json::{Value, json};
use uuid::Uuid;

const USER_ID: &str = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";

#[test]
#[ignore = "requires MAGIES_SING_BOX_CONFIG_BIN pointing to official sing-box 1.13.18"]
fn official_sing_box_accepts_every_p0_outbound() {
    let binary = PathBuf::from(
        std::env::var_os("MAGIES_SING_BOX_CONFIG_BIN")
            .expect("MAGIES_SING_BOX_CONFIG_BIN must point to official sing-box"),
    );
    let mut outbounds = generated_outbounds();
    for (outbound, tag) in outbounds.iter_mut().zip([
        "proxy-vless",
        "proxy-vmess",
        "proxy-trojan",
        "proxy-shadowsocks",
        "proxy-hysteria2",
    ]) {
        outbound["tag"] = Value::String(tag.to_owned());
    }
    outbounds.push(json!({ "type": "direct", "tag": "direct" }));
    let config = json!({
        "log": { "level": "warn" },
        "inbounds": [{
            "type": "socks",
            "tag": "socks-in",
            "listen": "127.0.0.1",
            "listen_port": 10808
        }],
        "outbounds": outbounds,
        "route": { "final": "proxy-vless" }
    });
    let path = std::env::temp_dir().join(format!(
        "magies-outbound-smoke-{}-{}.json",
        std::process::id(),
        std::env::consts::OS
    ));
    fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();

    let output = Command::new(binary)
        .arg("check")
        .arg("-c")
        .arg(&path)
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();

    assert!(
        output.status.success(),
        "sing-box rejected generated outbounds: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn generated_outbounds() -> Vec<Value> {
    let websocket_node = VlessParser
        .parse(&format!(
            "vless://{USER_ID}@edge.example.com:443?type=ws&security=tls\
             &host=cdn.example.com&path=%2Fproxy&sni=www.example.com&fp=chrome"
        ))
        .unwrap();
    let grpc_node = VmessParser
        .parse(&format!(
            "vmess://{USER_ID}@edge.example.com:443?type=grpc&serviceName=api\
             &mode=gun&security=tls&encryption=auto&sni=www.example.com"
        ))
        .unwrap();
    let trojan = TrojanParser
        .parse("trojan://password@edge.example.com:443")
        .unwrap();
    let shadowsocks = ShadowsocksParser
        .parse("ss://aes-256-gcm:password@edge.example.com:443")
        .unwrap();
    let hysteria2 = Hysteria2Parser
        .parse(
            "hy2://password@edge.example.com:443?obfs=salamander\
             &obfs-password=secret&sni=www.example.com",
        )
        .unwrap();

    vec![
        generate(
            ProxyProtocol::Vless,
            Some(websocket_node.transport().clone()),
            websocket_node.tls().cloned(),
            NodeCredential::from(websocket_node.credential()),
        ),
        generate(
            ProxyProtocol::Vmess,
            Some(grpc_node.transport().clone()),
            grpc_node.tls().cloned(),
            NodeCredential::from(grpc_node.credential()),
        ),
        generate(
            ProxyProtocol::Trojan,
            Some(trojan.transport().clone()),
            trojan.tls().cloned(),
            NodeCredential::from(trojan.credential()),
        ),
        generate(
            ProxyProtocol::Shadowsocks,
            Some(TransportConfig::Tcp),
            None,
            NodeCredential::from(shadowsocks.credential()),
        ),
        generate(
            ProxyProtocol::Hysteria2,
            None,
            hysteria2.tls().cloned(),
            NodeCredential::from(hysteria2.credential()),
        ),
    ]
}

fn generate(
    protocol: ProxyProtocol,
    transport: Option<TransportConfig>,
    tls: Option<TlsConfig>,
    credential: NodeCredential<'_>,
) -> Value {
    let mut node = ProxyNode::new(
        Uuid::parse_str("018f78b5-08ee-7caa-94f3-1d5d781aba22").unwrap(),
        "Smoke node",
        protocol,
        "192.0.2.1",
        443,
        Some(CredentialRef::new("keychain://nodes/smoke").unwrap()),
    )
    .unwrap();
    node.transport = transport;
    node.tls = tls;
    SingBoxOutboundConfigGenerator::generate(&node, credential)
        .unwrap()
        .json()
        .clone()
}

/// sing-box has no SHA-256 certificate pin, which is why the generator refuses
/// a pinned node instead of dropping the pin.
///
/// This asserts the absence, so a later sing-box that gains the field fails the
/// test and the refusal can be replaced by real support.
#[test]
#[ignore = "requires MAGIES_SING_BOX_CONFIG_BIN pointing to official sing-box 1.13.18"]
fn official_sing_box_still_has_no_pinned_sha256() {
    let binary = PathBuf::from(
        std::env::var_os("MAGIES_SING_BOX_CONFIG_BIN")
            .expect("MAGIES_SING_BOX_CONFIG_BIN must point to official sing-box"),
    );
    let config = json!({
        "log": { "level": "warn" },
        "outbounds": [{
            "type": "vless",
            "tag": "proxy",
            "server": "edge.example.com",
            "server_port": 443,
            "uuid": USER_ID,
            "tls": {
                "enabled": true,
                "server_name": "edge.example.com",
                "pinned_sha256": "6ff212bbab490b686b06209c6074865f9340f4c0f9c4aa7d34d568c2a2cebe73"
            }
        }]
    });
    let path = std::env::temp_dir().join(format!(
        "magies-pin-smoke-{}-{}.json",
        std::process::id(),
        std::env::consts::OS
    ));
    fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();

    let output = Command::new(binary)
        .arg("check")
        .arg("-c")
        .arg(&path)
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();

    let reason = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "sing-box accepted a pinned digest, so it can serve pinned nodes now"
    );
    assert!(
        reason.contains("pinned_sha256"),
        "sing-box refused for an unrelated reason: {reason}"
    );
}
