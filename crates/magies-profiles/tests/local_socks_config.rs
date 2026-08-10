use magies_domain::CoreType;
use magies_profiles::{
    DEFAULT_SOCKS_PORT, LocalProxyConfigError, LocalSocksConfigGenerator, LocalSocksProfile,
};
use serde_json::json;

#[test]
fn uses_the_prd_default_loopback_socks_profile() {
    let profile = LocalSocksProfile::default();

    assert_eq!(profile.port().get(), DEFAULT_SOCKS_PORT);
    assert!(profile.udp_enabled());
}

#[test]
fn rejects_ports_outside_the_tcp_range() {
    assert_eq!(
        LocalSocksProfile::new(0).unwrap_err(),
        LocalProxyConfigError::InvalidPort { port: 0 }
    );
    assert_eq!(
        LocalSocksProfile::new(65_536).unwrap_err(),
        LocalProxyConfigError::InvalidPort { port: 65_536 }
    );
}

#[test]
fn generates_a_loopback_xray_socks_inbound() {
    let profile = LocalSocksProfile::new(18_080).unwrap();
    let generated = LocalSocksConfigGenerator::generate(CoreType::Xray, &profile);

    assert_eq!(generated.core_type(), CoreType::Xray);
    assert_eq!(
        generated.json(),
        &json!({
            "log": { "loglevel": "warning" },
            "inbounds": [{
                "tag": "socks-in",
                "listen": "127.0.0.1",
                "port": 18_080,
                "protocol": "socks",
                "settings": {
                    "auth": "noauth",
                    "udp": true,
                    "ip": "127.0.0.1"
                }
            }],
            "outbounds": [{
                "tag": "direct",
                "protocol": "freedom",
                "settings": {}
            }]
        })
    );
}

#[test]
fn generates_a_loopback_sing_box_socks_inbound() {
    let profile = LocalSocksProfile::new(18_081).unwrap();
    let generated = LocalSocksConfigGenerator::generate(CoreType::SingBox, &profile);

    assert_eq!(generated.core_type(), CoreType::SingBox);
    assert_eq!(
        generated.json(),
        &json!({
            "log": { "level": "warn" },
            "inbounds": [{
                "type": "socks",
                "tag": "socks-in",
                "listen": "127.0.0.1",
                "listen_port": 18_081
            }],
            "outbounds": [{
                "type": "direct",
                "tag": "direct"
            }]
        })
    );
}
