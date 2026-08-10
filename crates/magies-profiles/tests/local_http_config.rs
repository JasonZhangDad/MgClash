use magies_domain::CoreType;
use magies_profiles::{
    DEFAULT_HTTP_PORT, LocalHttpConfigGenerator, LocalHttpProfile, LocalProxyConfigError,
};
use serde_json::json;

#[test]
fn uses_the_prd_default_loopback_http_profile() {
    let profile = LocalHttpProfile::default();

    assert_eq!(profile.port().get(), DEFAULT_HTTP_PORT);
}

#[test]
fn rejects_http_ports_outside_the_tcp_range() {
    assert_eq!(
        LocalHttpProfile::new(0).unwrap_err(),
        LocalProxyConfigError::InvalidPort { port: 0 }
    );
    assert_eq!(
        LocalHttpProfile::new(65_536).unwrap_err(),
        LocalProxyConfigError::InvalidPort { port: 65_536 }
    );
}

#[test]
fn generates_a_loopback_xray_http_inbound() {
    let profile = LocalHttpProfile::new(18_090).unwrap();
    let generated = LocalHttpConfigGenerator::generate(CoreType::Xray, &profile);

    assert_eq!(generated.core_type(), CoreType::Xray);
    assert_eq!(
        generated.json(),
        &json!({
            "log": { "loglevel": "warning" },
            "inbounds": [{
                "tag": "http-in",
                "listen": "127.0.0.1",
                "port": 18_090,
                "protocol": "http",
                "settings": {
                    "allowTransparent": false
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
fn generates_a_loopback_sing_box_http_inbound() {
    let profile = LocalHttpProfile::new(18_091).unwrap();
    let generated = LocalHttpConfigGenerator::generate(CoreType::SingBox, &profile);

    assert_eq!(generated.core_type(), CoreType::SingBox);
    assert_eq!(
        generated.json(),
        &json!({
            "log": { "level": "warn" },
            "inbounds": [{
                "type": "http",
                "tag": "http-in",
                "listen": "127.0.0.1",
                "listen_port": 18_091,
                "set_system_proxy": false
            }],
            "outbounds": [{
                "type": "direct",
                "tag": "direct"
            }]
        })
    );
}
