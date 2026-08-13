use std::fs;
use std::process::id;
use std::sync::atomic::{AtomicUsize, Ordering};

use magies_desktop_lib::dns_settings::{
    DesktopDnsStrategy, DnsMode, DnsSettings, DnsSettingsStoreError, SqliteDnsSettingsStore,
};
use magies_profiles::SingBoxDnsConfigGenerator;
use rusqlite::Connection;

static DATABASE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

#[test]
fn defaults_to_the_system_resolver() {
    let settings = DnsSettings::default();
    let config = SingBoxDnsConfigGenerator::generate(&settings.profile().unwrap());

    assert_eq!(settings.mode, DnsMode::System);
    assert_eq!(config.json()["servers"][0]["type"], "local");
    assert_eq!(config.json()["final"], "system");
    assert_eq!(config.json()["strategy"], "prefer_ipv4");
}

#[test]
fn builds_every_supported_upstream_transport() {
    for (mode, server, port, expected_type) in [
        (DnsMode::PlainUdp, "1.1.1.1", 53, "udp"),
        (DnsMode::PlainTcp, "1.1.1.1", 53, "tcp"),
        (DnsMode::Doh, "cloudflare-dns.com", 443, "https"),
        (DnsMode::Dot, "1dot1dot1dot1.cloudflare-dns.com", 853, "tls"),
    ] {
        let settings = DnsSettings {
            mode,
            server: server.to_owned(),
            port,
            ..DnsSettings::default()
        };
        let config = SingBoxDnsConfigGenerator::generate(&settings.profile().unwrap());

        assert_eq!(config.json()["servers"][1]["type"], expected_type);
        assert_eq!(config.json()["servers"][1]["server"], server);
        assert_eq!(config.json()["servers"][1]["server_port"], port);
        assert_eq!(config.json()["final"], "primary");
    }
}

#[test]
fn builds_doh_path_strategy_fake_ip_and_system_domain_rules() {
    let settings = DnsSettings {
        mode: DnsMode::Doh,
        server: "cloudflare-dns.com".to_owned(),
        port: 443,
        doh_path: "/custom-query".to_owned(),
        strategy: DesktopDnsStrategy::Ipv6Only,
        fake_ip_enabled: true,
        ipv6_enabled: true,
        system_domains: vec!["lan".to_owned(), "example.local".to_owned()],
        ..DnsSettings::default()
    };

    let config = SingBoxDnsConfigGenerator::generate(&settings.profile().unwrap());

    assert_eq!(config.json()["servers"][1]["path"], "/custom-query");
    assert_eq!(config.json()["servers"][1]["domain_resolver"], "system");
    assert_eq!(config.json()["servers"][2]["type"], "fakeip");
    assert_eq!(config.json()["strategy"], "ipv6_only");
    assert_eq!(
        config.json()["rules"][0]["domain_suffix"],
        serde_json::json!(["lan", "example.local"])
    );
    assert_eq!(config.json()["rules"][0]["server"], "system");
}

#[test]
fn rejects_invalid_settings_before_they_reach_the_runtime() {
    let invalid_port = DnsSettings {
        mode: DnsMode::PlainUdp,
        port: 0,
        ..DnsSettings::default()
    };
    assert!(invalid_port.profile().is_err());

    let invalid_path = DnsSettings {
        mode: DnsMode::Doh,
        doh_path: "dns-query".to_owned(),
        ..DnsSettings::default()
    };
    assert!(invalid_path.profile().is_err());

    let unavailable_ipv6 = DnsSettings {
        strategy: DesktopDnsStrategy::PreferIpv6,
        ipv6_enabled: false,
        ..DnsSettings::default()
    };
    assert!(unavailable_ipv6.profile().is_err());

    let empty_rule = DnsSettings {
        system_domains: vec![" ".to_owned()],
        ..DnsSettings::default()
    };
    assert!(empty_rule.profile().is_err());

    let store = SqliteDnsSettingsStore::open_in_memory().unwrap();
    assert!(matches!(
        store.save(&invalid_port),
        Err(DnsSettingsStoreError::InvalidSettings(_))
    ));
}

#[test]
fn defaults_and_persists_the_complete_settings() {
    let path = database_path("persists");
    let store = SqliteDnsSettingsStore::open(&path).unwrap();
    assert_eq!(store.load().unwrap(), DnsSettings::default());
    let settings = DnsSettings {
        mode: DnsMode::Dot,
        server: "dns.example.com".to_owned(),
        port: 853,
        strategy: DesktopDnsStrategy::Ipv4Only,
        fake_ip_enabled: true,
        system_domains: vec!["corp.example".to_owned()],
        ..DnsSettings::default()
    };

    store.save(&settings).unwrap();
    drop(store);

    let reopened = SqliteDnsSettingsStore::open(&path).unwrap();
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
            "CREATE TABLE dns_settings (id INTEGER PRIMARY KEY, settings_json TEXT NOT NULL);
             INSERT INTO dns_settings (id, settings_json) VALUES (1, '{not-json}');",
        )
        .unwrap();
    drop(connection);

    let store = SqliteDnsSettingsStore::open(&path).unwrap();
    assert!(matches!(
        store.load(),
        Err(DnsSettingsStoreError::InvalidStoredJson(_))
    ));

    drop(store);
    fs::remove_file(path).unwrap();
}

fn database_path(name: &str) -> std::path::PathBuf {
    let sequence = DATABASE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mgclash-dns-settings-{name}-{}-{sequence}.sqlite",
        id()
    ))
}
