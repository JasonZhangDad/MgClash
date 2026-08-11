use std::fs;
use std::process::id;
use std::sync::atomic::{AtomicUsize, Ordering};

use magies_desktop_lib::route_settings::{
    DesktopRouteOutbound, RouteRuleKind, RouteRuleSetting, RouteSettings, RouteSettingsStoreError,
    SqliteRouteSettingsStore,
};
use magies_routing::{RoutingMode, SingBoxRouteConfigGenerator};
use rusqlite::Connection;

static DATABASE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

#[test]
fn builds_every_v01_matcher_in_runtime_order() {
    let settings = RouteSettings {
        rules: vec![
            rule(RouteRuleKind::GeoIp, "CN", DesktopRouteOutbound::Direct),
            rule(
                RouteRuleKind::Domain,
                "example.com",
                DesktopRouteOutbound::Proxy,
            ),
            rule(
                RouteRuleKind::DomainSuffix,
                "cn",
                DesktopRouteOutbound::Direct,
            ),
            rule(
                RouteRuleKind::DomainKeyword,
                "video",
                DesktopRouteOutbound::Proxy,
            ),
            rule(
                RouteRuleKind::IpCidr,
                "10.0.0.0/8",
                DesktopRouteOutbound::Direct,
            ),
            rule(
                RouteRuleKind::IpCidr6,
                "2001:db8::/32",
                DesktopRouteOutbound::Proxy,
            ),
            rule(RouteRuleKind::Port, "443", DesktopRouteOutbound::Proxy),
            rule(RouteRuleKind::Network, "udp", DesktopRouteOutbound::Direct),
            rule(
                RouteRuleKind::GeoSite,
                "private",
                DesktopRouteOutbound::Direct,
            ),
        ],
        final_outbound: DesktopRouteOutbound::Direct,
    };

    let config =
        SingBoxRouteConfigGenerator::generate(&settings.profile(RoutingMode::Rule).unwrap());

    assert_eq!(config.json()["rules"][1]["domain"][0], "example.com");
    assert_eq!(config.json()["rules"][2]["domain_suffix"][0], ".cn");
    assert_eq!(config.json()["rules"][3]["domain_keyword"][0], "video");
    assert_eq!(config.json()["rules"][4]["ip_cidr"][0], "10.0.0.0/8");
    assert_eq!(config.json()["rules"][5]["ip_cidr"][0], "2001:db8::/32");
    assert_eq!(config.json()["rules"][6]["port"][0], 443);
    assert_eq!(config.json()["rules"][7]["network"], "udp");
    assert_eq!(config.json()["rules"][8]["rule_set"][0], "geoip-cn");
    assert_eq!(config.json()["rules"][9]["rule_set"][0], "geosite-private");
    assert_eq!(config.json()["final"], "direct");
}

#[test]
fn global_and_direct_modes_ignore_saved_rules_and_final() {
    let settings = RouteSettings {
        rules: vec![rule(
            RouteRuleKind::Domain,
            "example.com",
            DesktopRouteOutbound::Direct,
        )],
        final_outbound: DesktopRouteOutbound::Direct,
    };

    let global =
        SingBoxRouteConfigGenerator::generate(&settings.profile(RoutingMode::Global).unwrap());
    let direct =
        SingBoxRouteConfigGenerator::generate(&settings.profile(RoutingMode::Direct).unwrap());

    assert_eq!(global.json()["rules"].as_array().unwrap().len(), 1);
    assert_eq!(global.json()["final"], "proxy");
    assert_eq!(direct.json()["rules"].as_array().unwrap().len(), 0);
    assert_eq!(direct.json()["final"], "direct");
}

#[test]
fn rejects_invalid_rule_values() {
    for settings in [
        RouteSettings {
            rules: vec![rule(
                RouteRuleKind::Domain,
                "not a domain",
                DesktopRouteOutbound::Proxy,
            )],
            ..RouteSettings::default()
        },
        RouteSettings {
            rules: vec![rule(
                RouteRuleKind::Port,
                "zero",
                DesktopRouteOutbound::Proxy,
            )],
            ..RouteSettings::default()
        },
        RouteSettings {
            rules: vec![rule(
                RouteRuleKind::Network,
                "quic",
                DesktopRouteOutbound::Proxy,
            )],
            ..RouteSettings::default()
        },
    ] {
        assert!(settings.profile(RoutingMode::Rule).is_err());
    }
}

#[test]
fn defaults_and_persists_the_complete_settings() {
    let path = database_path("persists");
    let store = SqliteRouteSettingsStore::open(&path).unwrap();
    let defaults = store.load().unwrap();
    assert_eq!(defaults, RouteSettings::default());
    let settings = RouteSettings {
        rules: vec![rule(
            RouteRuleKind::GeoSite,
            "private",
            DesktopRouteOutbound::Direct,
        )],
        final_outbound: DesktopRouteOutbound::Direct,
    };

    store.save(&settings).unwrap();
    drop(store);

    let reopened = SqliteRouteSettingsStore::open(&path).unwrap();
    assert_eq!(reopened.load().unwrap(), settings);

    drop(reopened);
    fs::remove_file(path).unwrap();
}

#[test]
fn rejects_invalid_json_already_in_the_database() {
    let path = database_path("invalid");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE route_settings (id INTEGER PRIMARY KEY, settings_json TEXT NOT NULL);
             INSERT INTO route_settings (id, settings_json) VALUES (1, '{not-json}');",
        )
        .unwrap();
    drop(connection);

    let store = SqliteRouteSettingsStore::open(&path).unwrap();
    assert!(matches!(
        store.load(),
        Err(RouteSettingsStoreError::InvalidStoredJson(_))
    ));

    drop(store);
    fs::remove_file(path).unwrap();
}

fn rule(kind: RouteRuleKind, value: &str, outbound: DesktopRouteOutbound) -> RouteRuleSetting {
    RouteRuleSetting {
        kind,
        value: value.to_owned(),
        outbound,
        enabled: true,
    }
}

fn database_path(name: &str) -> std::path::PathBuf {
    let sequence = DATABASE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mgclash-route-settings-{name}-{}-{sequence}.sqlite",
        id()
    ))
}
