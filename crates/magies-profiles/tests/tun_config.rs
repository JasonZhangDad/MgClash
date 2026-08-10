use magies_domain::CoreType;
use magies_platform::OperatingSystem;
use magies_profiles::{SingBoxTunConfigGenerator, TunProfile, TunProfileError};
use serde_json::json;

#[test]
fn generates_recommended_windows_tun_config() {
    let profile = TunProfile::new(OperatingSystem::Windows, true, 1500, true, true).unwrap();

    let config = SingBoxTunConfigGenerator::generate(&profile);

    assert_eq!(config.core_type(), CoreType::SingBox);
    assert_eq!(
        config.json(),
        &json!({
            "log": { "level": "warn" },
            "inbounds": [{
                "type": "tun",
                "tag": "tun-in",
                "interface_name": "MgClash",
                "address": ["172.19.0.1/30", "fdfe:dcba:9876::1/126"],
                "mtu": 1500,
                "auto_route": true,
                "strict_route": true,
                "stack": "gvisor"
            }],
            "outbounds": [{ "type": "direct", "tag": "direct" }],
            "route": { "auto_detect_interface": true, "final": "direct" }
        })
    );
}

#[test]
fn linux_auto_redirect_is_enabled_only_with_auto_route() {
    let recommended = TunProfile::new(OperatingSystem::Linux, false, 1500, true, true).unwrap();
    let smoke = TunProfile::new(OperatingSystem::Linux, false, 1500, false, false).unwrap();

    assert_eq!(
        SingBoxTunConfigGenerator::generate(&recommended).json()["inbounds"][0]["auto_redirect"],
        true
    );
    assert!(
        SingBoxTunConfigGenerator::generate(&smoke).json()["inbounds"][0]
            .get("auto_redirect")
            .is_none()
    );
    assert_eq!(
        SingBoxTunConfigGenerator::generate(&smoke).json()["inbounds"][0]["interface_name"],
        "mgclash0"
    );
}

#[test]
fn validates_platform_mtu_and_strict_route() {
    assert_eq!(
        TunProfile::new(OperatingSystem::MacOs, false, 1500, true, true),
        Err(TunProfileError::UnsupportedPlatform(OperatingSystem::MacOs))
    );
    for mtu in [1279, 9001] {
        assert_eq!(
            TunProfile::new(OperatingSystem::Linux, false, mtu, true, true),
            Err(TunProfileError::InvalidMtu { mtu })
        );
    }
    assert_eq!(
        TunProfile::new(OperatingSystem::Windows, false, 1500, false, true),
        Err(TunProfileError::StrictRouteRequiresAutoRoute)
    );
}
