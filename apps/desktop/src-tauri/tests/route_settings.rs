use std::fs;
use std::process::id;
use std::sync::atomic::{AtomicUsize, Ordering};

use magies_desktop_lib::route_settings::{
    DesktopRouteOutbound, RouteRuleKind, RouteRuleSetting, RouteSettings, RouteSettingsError,
    RouteSettingsStoreError, RuleProviderFormatSetting, RuleProviderSetting,
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
                RouteRuleKind::ProcessName,
                "chrome",
                DesktopRouteOutbound::Proxy,
            ),
            rule(
                RouteRuleKind::ProcessPath,
                "/Applications/Safari.app",
                DesktopRouteOutbound::Direct,
            ),
            rule(
                RouteRuleKind::GeoSite,
                "private",
                DesktopRouteOutbound::Direct,
            ),
        ],
        providers: Vec::new(),
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
    assert_eq!(config.json()["rules"][8]["process_name"][0], "chrome");
    assert_eq!(
        config.json()["rules"][9]["process_path"][0],
        "/Applications/Safari.app"
    );
    assert_eq!(config.json()["rules"][10]["rule_set"][0], "geoip-cn");
    assert_eq!(config.json()["rules"][11]["rule_set"][0], "geosite-private");
    assert_eq!(config.json()["final"], "direct");
}

#[test]
fn migrates_legacy_settings_into_a_default_scheme_bundle() {
    let path = database_path("legacy-bundle");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE route_settings (id INTEGER PRIMARY KEY, settings_json TEXT NOT NULL);
             INSERT INTO route_settings (id, settings_json) VALUES (1, '{\"rules\":[],\"finalOutbound\":\"proxy\"}');",
        )
        .unwrap();
    drop(connection);

    let store = SqliteRouteSettingsStore::open(&path).unwrap();
    let bundle = store.load_bundle().unwrap();
    assert_eq!(bundle.active_scheme_id, "default");
    assert_eq!(bundle.schemes.len(), 1);
    assert_eq!(bundle.active_settings(), RouteSettings::default());

    drop(store);
    fs::remove_file(path).unwrap();
}

#[test]
fn global_and_direct_modes_ignore_saved_rules_and_final() {
    let settings = RouteSettings {
        rules: vec![rule(
            RouteRuleKind::Domain,
            "example.com",
            DesktopRouteOutbound::Direct,
        )],
        providers: Vec::new(),
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
        providers: Vec::new(),
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

#[test]
fn rule_providers_become_remote_rule_sets_after_the_explicit_rules() {
    let settings = RouteSettings {
        rules: vec![RouteRuleSetting {
            kind: RouteRuleKind::DomainSuffix,
            value: "cn".to_owned(),
            outbound: DesktopRouteOutbound::Direct,
            enabled: true,
        }],
        providers: vec![RuleProviderSetting {
            name: "ads".to_owned(),
            url: "https://example.com/ads.srs".to_owned(),
            format: RuleProviderFormatSetting::Binary,
            outbound: DesktopRouteOutbound::Direct,
            enabled: true,
        }],
        final_outbound: DesktopRouteOutbound::Proxy,
    };

    let profile = settings.profile(RoutingMode::Rule).unwrap();
    let config = SingBoxRouteConfigGenerator::generate(&profile);

    assert_eq!(config.json()["rules"][1]["domain_suffix"][0], ".cn");
    assert_eq!(config.json()["rules"][2]["rule_set"][0], "ads");
    assert_eq!(
        config.json()["rule_set"][0]["url"],
        "https://example.com/ads.srs"
    );
    assert_eq!(config.json()["rule_set"][0]["format"], "binary");
}

#[test]
fn a_rule_provider_with_a_bad_url_is_a_typed_error() {
    let settings = RouteSettings {
        rules: Vec::new(),
        providers: vec![RuleProviderSetting {
            name: "ads".to_owned(),
            url: "not-a-url".to_owned(),
            format: RuleProviderFormatSetting::Source,
            outbound: DesktopRouteOutbound::Direct,
            enabled: true,
        }],
        final_outbound: DesktopRouteOutbound::Proxy,
    };

    assert!(settings.profile(RoutingMode::Rule).is_err());
}

#[test]
fn a_blocked_rule_and_rule_set_reach_the_generated_config() {
    let settings = RouteSettings {
        rules: vec![RouteRuleSetting {
            kind: RouteRuleKind::DomainSuffix,
            value: "ads.example".to_owned(),
            outbound: DesktopRouteOutbound::Block,
            enabled: true,
        }],
        providers: vec![RuleProviderSetting {
            name: "ads".to_owned(),
            url: "https://example.com/ads.srs".to_owned(),
            format: RuleProviderFormatSetting::Binary,
            outbound: DesktopRouteOutbound::Block,
            enabled: true,
        }],
        final_outbound: DesktopRouteOutbound::Proxy,
    };

    let profile = settings.profile(RoutingMode::Rule).unwrap();
    let config = SingBoxRouteConfigGenerator::generate(&profile);

    assert_eq!(config.json()["rules"][1]["action"], "reject");
    assert!(config.json()["rules"][1]["outbound"].is_null());
    assert_eq!(config.json()["rules"][2]["rule_set"][0], "ads");
    assert_eq!(config.json()["rules"][2]["action"], "reject");
}

#[test]
fn blocking_by_default_is_refused_by_the_desktop_settings() {
    let settings = RouteSettings {
        rules: Vec::new(),
        providers: Vec::new(),
        final_outbound: DesktopRouteOutbound::Block,
    };

    assert!(settings.profile(RoutingMode::Rule).is_err());
}

#[test]
fn a_cached_rule_set_is_read_from_disk_and_an_uncached_one_from_its_url() {
    let settings = RouteSettings {
        rules: Vec::new(),
        providers: vec![
            RuleProviderSetting {
                name: "ads".to_owned(),
                url: "https://example.com/ads.srs".to_owned(),
                format: RuleProviderFormatSetting::Binary,
                outbound: DesktopRouteOutbound::Block,
                enabled: true,
            },
            RuleProviderSetting {
                name: "cn".to_owned(),
                url: "https://example.com/cn.srs".to_owned(),
                format: RuleProviderFormatSetting::Binary,
                outbound: DesktopRouteOutbound::Direct,
                enabled: true,
            },
        ],
        final_outbound: DesktopRouteOutbound::Proxy,
    };
    let mut cached = std::collections::HashMap::new();
    cached.insert("ads".to_owned(), "/cache/ads.srs".to_owned());

    let profile = settings
        .profile_with_cached_sets(RoutingMode::Rule, &cached)
        .unwrap();
    let config = SingBoxRouteConfigGenerator::generate(&profile);

    let sets = config.json()["rule_set"].as_array().unwrap();
    assert_eq!(sets[0]["type"], "local");
    assert_eq!(sets[0]["path"], "/cache/ads.srs");
    // Nothing downloaded yet, so the Core still fetches this one itself.
    assert_eq!(sets[1]["type"], "remote");
    assert_eq!(sets[1]["url"], "https://example.com/cn.srs");
}

#[test]
fn builds_a_protocol_rule_and_rejects_a_protocol_the_cores_cannot_sniff() {
    let settings = RouteSettings {
        rules: vec![rule(
            RouteRuleKind::Protocol,
            "bittorrent",
            DesktopRouteOutbound::Direct,
        )],
        providers: Vec::new(),
        final_outbound: DesktopRouteOutbound::Proxy,
    };

    let config =
        SingBoxRouteConfigGenerator::generate(&settings.profile(RoutingMode::Rule).unwrap());

    assert_eq!(config.json()["rules"][1]["protocol"][0], "bittorrent");

    let unsupported = RouteSettings {
        rules: vec![rule(
            RouteRuleKind::Protocol,
            "ssh",
            DesktopRouteOutbound::Direct,
        )],
        providers: Vec::new(),
        final_outbound: DesktopRouteOutbound::Proxy,
    };

    // sing-box knows more protocols than Xray does; offering one the other
    // Core cannot match would make the rule depend on which Core ran.
    assert!(matches!(
        unsupported.profile(RoutingMode::Rule),
        Err(RouteSettingsError::InvalidProtocol { .. })
    ));
}

#[test]
fn builds_an_inbound_rule_and_rejects_an_inbound_the_app_never_creates() {
    let settings = RouteSettings {
        rules: vec![
            rule(
                RouteRuleKind::Inbound,
                "socks",
                DesktopRouteOutbound::Direct,
            ),
            rule(
                RouteRuleKind::Inbound,
                "tun-in",
                DesktopRouteOutbound::Proxy,
            ),
        ],
        providers: Vec::new(),
        final_outbound: DesktopRouteOutbound::Proxy,
    };

    let config =
        SingBoxRouteConfigGenerator::generate(&settings.profile(RoutingMode::Rule).unwrap());

    // Written either way the user might type it: the short name or the tag the
    // config actually carries.
    assert_eq!(config.json()["rules"][1]["inbound"][0], "socks-in");
    assert_eq!(config.json()["rules"][2]["inbound"][0], "tun-in");

    let unknown = RouteSettings {
        rules: vec![rule(
            RouteRuleKind::Inbound,
            "wireguard",
            DesktopRouteOutbound::Direct,
        )],
        providers: Vec::new(),
        final_outbound: DesktopRouteOutbound::Proxy,
    };

    assert!(matches!(
        unknown.profile(RoutingMode::Rule),
        Err(RouteSettingsError::InvalidInbound { .. })
    ));
}
