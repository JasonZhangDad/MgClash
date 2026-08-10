use magies_domain::CoreType;
use magies_platform::OperatingSystem;
use magies_profiles::{SingBoxTunConfigGenerator, TunProfile, TunProfileError, TunRouteSettings};
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

#[test]
fn emits_include_and_exclude_routes_without_reordering() {
    let routes = TunRouteSettings::new(
        vec!["0.0.0.0/1".to_owned(), "128.0.0.0/1".to_owned()],
        vec!["10.0.0.0/8".to_owned(), "192.168.0.0/16".to_owned()],
    )
    .unwrap();
    let profile = TunProfile::new(OperatingSystem::Windows, false, 1500, true, true)
        .unwrap()
        .with_routes(routes)
        .unwrap();

    let config = SingBoxTunConfigGenerator::generate(&profile);
    let inbound = &config.json()["inbounds"][0];

    assert_eq!(
        inbound["route_address"],
        json!(["0.0.0.0/1", "128.0.0.0/1"])
    );
    assert_eq!(
        inbound["route_exclude_address"],
        json!(["10.0.0.0/8", "192.168.0.0/16"])
    );
}

#[test]
fn rejects_invalid_conflicting_and_disabled_family_routes() {
    assert_eq!(
        TunRouteSettings::new(vec!["not-a-cidr".to_owned()], Vec::new()),
        Err(TunProfileError::InvalidRoute {
            value: "not-a-cidr".to_owned()
        })
    );
    assert_eq!(
        TunRouteSettings::new(vec!["10.0.0.0/8".to_owned()], vec!["10.0.0.0/8".to_owned()]),
        Err(TunProfileError::ConflictingRoute {
            cidr: "10.0.0.0/8".to_owned()
        })
    );

    let ipv6_route = TunRouteSettings::new(vec!["2001:db8::/32".to_owned()], Vec::new()).unwrap();
    let ipv4_only = TunProfile::new(OperatingSystem::Linux, false, 1500, true, true).unwrap();
    assert_eq!(
        ipv4_only.with_routes(ipv6_route),
        Err(TunProfileError::Ipv6RouteRequiresIpv6 {
            cidr: "2001:db8::/32".to_owned()
        })
    );

    let routes = TunRouteSettings::new(vec!["10.0.0.0/8".to_owned()], Vec::new()).unwrap();
    let no_auto_route = TunProfile::new(OperatingSystem::Linux, false, 1500, false, false).unwrap();
    assert_eq!(
        no_auto_route.with_routes(routes),
        Err(TunProfileError::RoutesRequireAutoRoute)
    );
}
