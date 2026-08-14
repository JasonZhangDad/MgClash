use magies_routing::{
    Network, RouteConfigError, RouteOutbound, RouteProfile, RoutingMode, RoutingRule,
    RuleProviderFormat, SingBoxRouteConfigGenerator,
};
use serde_json::json;

#[test]
fn generates_global_and_direct_modes_without_user_rules() {
    let global = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let direct = RouteProfile::new(RoutingMode::Direct, Vec::new(), RouteOutbound::Direct).unwrap();

    assert_eq!(global.mode(), RoutingMode::Global);
    assert_eq!(direct.mode(), RoutingMode::Direct);

    assert_eq!(
        SingBoxRouteConfigGenerator::generate(&global).json(),
        &json!({
            "rules": [{
                "ip_is_private": true,
                "action": "route",
                "outbound": "direct"
            }],
            "final": "proxy"
        })
    );
    assert_eq!(
        SingBoxRouteConfigGenerator::generate(&direct).json(),
        &json!({ "rules": [], "final": "direct" })
    );
}

#[test]
fn orders_explicit_before_geo_rules_and_emits_every_v01_matcher() {
    let rules = vec![
        RoutingRule::geo_ip("CN", RouteOutbound::Direct, 1, true).unwrap(),
        RoutingRule::domain("example.com", RouteOutbound::Proxy, 20, true).unwrap(),
        RoutingRule::domain_keyword("disabled", RouteOutbound::Direct, 5, false).unwrap(),
        RoutingRule::domain_suffix("cn", RouteOutbound::Direct, 10, true).unwrap(),
        RoutingRule::ip_cidr("10.0.0.0/8", RouteOutbound::Direct, 30, true).unwrap(),
        RoutingRule::ip_cidr6("2001:db8::/32", RouteOutbound::Proxy, 40, true).unwrap(),
        RoutingRule::port(443, RouteOutbound::Proxy, 50, true).unwrap(),
        RoutingRule::network(Network::Udp, RouteOutbound::Direct, 60, true),
        RoutingRule::geo_site("private", RouteOutbound::Direct, 2, true).unwrap(),
    ];
    let profile = RouteProfile::new(RoutingMode::Rule, rules, RouteOutbound::Proxy).unwrap();

    let config = SingBoxRouteConfigGenerator::generate(&profile);

    assert_eq!(
        config.json(),
        &json!({
            "rules": [
                {
                    "ip_is_private": true,
                    "action": "route",
                    "outbound": "direct"
                },
                {
                    "domain_suffix": [".cn"],
                    "action": "route",
                    "outbound": "direct"
                },
                {
                    "domain": ["example.com"],
                    "action": "route",
                    "outbound": "proxy"
                },
                {
                    "ip_cidr": ["10.0.0.0/8"],
                    "action": "route",
                    "outbound": "direct"
                },
                {
                    "ip_cidr": ["2001:db8::/32"],
                    "action": "route",
                    "outbound": "proxy"
                },
                {
                    "port": [443],
                    "action": "route",
                    "outbound": "proxy"
                },
                {
                    "network": "udp",
                    "action": "route",
                    "outbound": "direct"
                },
                {
                    "rule_set": ["geoip-cn"],
                    "action": "route",
                    "outbound": "direct"
                },
                {
                    "rule_set": ["geosite-private"],
                    "action": "route",
                    "outbound": "direct"
                }
            ],
            "rule_set": [
                {
                    "type": "remote",
                    "tag": "geoip-cn",
                    "format": "binary",
                    "url": "https://raw.githubusercontent.com/SagerNet/sing-geoip/rule-set/geoip-cn.srs",
                    "download_detour": "direct"
                },
                {
                    "type": "remote",
                    "tag": "geosite-private",
                    "format": "binary",
                    "url": "https://raw.githubusercontent.com/SagerNet/sing-geosite/rule-set/geosite-private.srs",
                    "download_detour": "direct"
                }
            ],
            "final": "proxy"
        })
    );
}

#[test]
fn preserves_input_order_for_equal_priorities_and_deduplicates_geo_sources() {
    let rules = vec![
        RoutingRule::domain("first.example", RouteOutbound::Proxy, 10, true).unwrap(),
        RoutingRule::domain("second.example", RouteOutbound::Direct, 10, true).unwrap(),
        RoutingRule::geo_ip("cn", RouteOutbound::Direct, 20, true).unwrap(),
        RoutingRule::geo_ip("cn", RouteOutbound::Proxy, 30, true).unwrap(),
    ];
    let profile = RouteProfile::new(RoutingMode::Rule, rules, RouteOutbound::Direct).unwrap();
    let config = SingBoxRouteConfigGenerator::generate(&profile);

    assert_eq!(
        config.json()["rules"][1]["domain"],
        json!(["first.example"])
    );
    assert_eq!(
        config.json()["rules"][2]["domain"],
        json!(["second.example"])
    );
    assert_eq!(config.json()["rule_set"].as_array().unwrap().len(), 1);
    assert_eq!(config.json()["final"], "direct");
}

#[test]
fn rejects_invalid_matchers_and_families() {
    assert_eq!(
        RoutingRule::domain("not a domain", RouteOutbound::Proxy, 0, true),
        Err(RouteConfigError::InvalidDomain {
            value: "not a domain".to_owned()
        })
    );
    assert_eq!(
        RoutingRule::domain_keyword("  ", RouteOutbound::Proxy, 0, true),
        Err(RouteConfigError::EmptyValue)
    );
    assert_eq!(
        RoutingRule::ip_cidr("2001:db8::/32", RouteOutbound::Proxy, 0, true),
        Err(RouteConfigError::ExpectedIpv4 {
            value: "2001:db8::/32".to_owned()
        })
    );
    assert_eq!(
        RoutingRule::ip_cidr6("10.0.0.0/8", RouteOutbound::Proxy, 0, true),
        Err(RouteConfigError::ExpectedIpv6 {
            value: "10.0.0.0/8".to_owned()
        })
    );
    assert_eq!(
        RoutingRule::ip_cidr("invalid", RouteOutbound::Proxy, 0, true),
        Err(RouteConfigError::InvalidCidr {
            value: "invalid".to_owned()
        })
    );
    assert_eq!(
        RoutingRule::port(0, RouteOutbound::Proxy, 0, true),
        Err(RouteConfigError::InvalidPort { port: 0 })
    );
    assert_eq!(
        RoutingRule::geo_site("../private", RouteOutbound::Direct, 0, true),
        Err(RouteConfigError::InvalidGeoCode {
            value: "../private".to_owned()
        })
    );
}

#[test]
fn rejects_rules_outside_rule_mode_and_incompatible_mode_finals() {
    let rule = RoutingRule::network(Network::Tcp, RouteOutbound::Proxy, 0, true);
    assert_eq!(
        RouteProfile::new(RoutingMode::Global, vec![rule], RouteOutbound::Proxy),
        Err(RouteConfigError::RulesRequireRuleMode)
    );
    assert_eq!(
        RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Direct),
        Err(RouteConfigError::GlobalModeRequiresProxyFinal)
    );
    assert_eq!(
        RouteProfile::new(RoutingMode::Direct, Vec::new(), RouteOutbound::Proxy),
        Err(RouteConfigError::DirectModeRequiresDirectFinal)
    );
}

#[test]
fn declares_a_remote_rule_provider_and_the_rule_that_uses_it() {
    let rules = vec![
        RoutingRule::rule_provider(
            "ads",
            "https://example.com/ads.srs",
            RuleProviderFormat::Binary,
            RouteOutbound::Direct,
            10,
            true,
        )
        .unwrap(),
        RoutingRule::rule_provider(
            "ads",
            "https://example.com/ads.srs",
            RuleProviderFormat::Binary,
            RouteOutbound::Direct,
            20,
            true,
        )
        .unwrap(),
    ];
    let profile = RouteProfile::new(RoutingMode::Rule, rules, RouteOutbound::Proxy).unwrap();
    let config = SingBoxRouteConfigGenerator::generate(&profile);

    assert_eq!(
        config.json()["rules"][1],
        json!({ "rule_set": ["ads"], "action": "route", "outbound": "direct" })
    );
    // The same provider used twice is still downloaded once.
    assert_eq!(
        config.json()["rule_set"],
        json!([{
            "type": "remote",
            "tag": "ads",
            "format": "binary",
            "url": "https://example.com/ads.srs",
            "download_detour": "direct"
        }])
    );
}

#[test]
fn rejects_a_rule_provider_that_is_not_a_safe_tag_or_https_source() {
    assert!(
        RoutingRule::rule_provider(
            "ads/../etc",
            "https://example.com/ads.srs",
            RuleProviderFormat::Binary,
            RouteOutbound::Direct,
            0,
            true,
        )
        .is_err()
    );
    assert!(
        RoutingRule::rule_provider(
            "ads",
            "file:///etc/passwd",
            RuleProviderFormat::Binary,
            RouteOutbound::Direct,
            0,
            true,
        )
        .is_err()
    );
}
