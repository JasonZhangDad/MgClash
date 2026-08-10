use magies_platform::OperatingSystem;
use magies_profiles::{
    DnsConfigError, DnsProfile, DnsRule, DnsServer, DnsStrategy, PlainDnsTransport,
    SingBoxDnsConfigGenerator, SingBoxTunConfigGenerator, TunProfile,
};
use serde_json::json;

fn complete_profile(fake_ip_enabled: bool, ipv6_enabled: bool) -> DnsProfile {
    DnsProfile::new(
        vec![
            DnsServer::system("system").unwrap(),
            DnsServer::plain("bootstrap", PlainDnsTransport::Udp, "1.1.1.1", 53, None).unwrap(),
            DnsServer::plain("plain-tcp", PlainDnsTransport::Tcp, "8.8.8.8", 53, None).unwrap(),
            DnsServer::doh("secure-doh", "dns.google", 443, "/dns-query", "bootstrap").unwrap(),
            DnsServer::dot("secure-dot", "one.one.one.one", 853, "bootstrap").unwrap(),
        ],
        vec![
            DnsRule::domain(vec!["example.com".to_owned()], "secure-doh").unwrap(),
            DnsRule::domain_suffix(vec![".cn".to_owned()], "system").unwrap(),
            DnsRule::domain_keyword(vec!["internal".to_owned()], "plain-tcp").unwrap(),
        ],
        "secure-doh",
        DnsStrategy::PreferIpv4,
        fake_ip_enabled,
        ipv6_enabled,
    )
    .unwrap()
}

#[test]
fn generates_all_v01_dns_server_types_rules_and_fake_ip() {
    let config = SingBoxDnsConfigGenerator::generate(&complete_profile(true, true));

    assert_eq!(
        config.json(),
        &json!({
            "servers": [
                { "type": "local", "tag": "system" },
                {
                    "type": "udp",
                    "tag": "bootstrap",
                    "server": "1.1.1.1",
                    "server_port": 53
                },
                {
                    "type": "tcp",
                    "tag": "plain-tcp",
                    "server": "8.8.8.8",
                    "server_port": 53
                },
                {
                    "type": "https",
                    "tag": "secure-doh",
                    "server": "dns.google",
                    "server_port": 443,
                    "path": "/dns-query",
                    "tls": { "enabled": true, "server_name": "dns.google" },
                    "domain_resolver": "bootstrap"
                },
                {
                    "type": "tls",
                    "tag": "secure-dot",
                    "server": "one.one.one.one",
                    "server_port": 853,
                    "tls": { "enabled": true, "server_name": "one.one.one.one" },
                    "domain_resolver": "bootstrap"
                },
                {
                    "type": "fakeip",
                    "tag": "fakeip",
                    "inet4_range": "198.18.0.0/15",
                    "inet6_range": "fc00::/18"
                }
            ],
            "rules": [
                {
                    "domain": ["example.com"],
                    "action": "route",
                    "server": "secure-doh"
                },
                {
                    "domain_suffix": [".cn"],
                    "action": "route",
                    "server": "system"
                },
                {
                    "domain_keyword": ["internal"],
                    "action": "route",
                    "server": "plain-tcp"
                },
                {
                    "query_type": ["A", "AAAA"],
                    "action": "route",
                    "server": "fakeip"
                }
            ],
            "final": "secure-doh",
            "strategy": "prefer_ipv4"
        })
    );
}

#[test]
fn integrates_dns_and_optional_hijack_into_tun_config() {
    let dns = complete_profile(false, false);
    let without_hijack = TunProfile::new(OperatingSystem::Windows, false, 1500, true, true)
        .unwrap()
        .with_dns(dns.clone(), false);
    let with_hijack = TunProfile::new(OperatingSystem::Linux, false, 1500, true, true)
        .unwrap()
        .with_dns(dns, true);

    let plain = SingBoxTunConfigGenerator::generate(&without_hijack);
    let hijacked = SingBoxTunConfigGenerator::generate(&with_hijack);

    assert_eq!(plain.json()["dns"]["strategy"], "prefer_ipv4");
    assert_eq!(
        plain.json()["route"]["default_domain_resolver"],
        "secure-doh"
    );
    assert!(plain.json()["route"].get("rules").is_none());
    assert_eq!(
        hijacked.json()["route"]["rules"],
        json!([
            { "action": "sniff" },
            { "protocol": "dns", "action": "hijack-dns" }
        ])
    );
    assert_eq!(hijacked.json()["dns"]["final"], "secure-doh");
}

#[test]
fn validates_server_and_rule_fields_before_profile_creation() {
    assert_eq!(DnsServer::system("  "), Err(DnsConfigError::InvalidTag));
    assert_eq!(
        DnsServer::plain("plain", PlainDnsTransport::Udp, "", 53, None),
        Err(DnsConfigError::InvalidServerAddress)
    );
    assert_eq!(
        DnsServer::plain("plain", PlainDnsTransport::Udp, "1.1.1.1", 0, None),
        Err(DnsConfigError::InvalidPort { port: 0 })
    );
    assert_eq!(
        DnsServer::plain("plain", PlainDnsTransport::Udp, "dns.example", 53, None),
        Err(DnsConfigError::BootstrapRequired {
            tag: "plain".to_owned()
        })
    );
    assert_eq!(
        DnsServer::doh("doh", "dns.example", 443, "dns-query", "bootstrap"),
        Err(DnsConfigError::InvalidDohPath)
    );
    assert_eq!(
        DnsRule::domain(Vec::new(), "system"),
        Err(DnsConfigError::EmptyRuleValues)
    );
}

#[test]
fn validates_profile_references_duplicates_and_ipv6_strategy() {
    let system = DnsServer::system("system").unwrap();
    assert_eq!(
        DnsProfile::new(
            Vec::new(),
            Vec::new(),
            "system",
            DnsStrategy::PreferIpv4,
            false,
            false,
        ),
        Err(DnsConfigError::NoServers)
    );
    assert_eq!(
        DnsProfile::new(
            vec![system.clone(), system.clone()],
            Vec::new(),
            "system",
            DnsStrategy::PreferIpv4,
            false,
            false,
        ),
        Err(DnsConfigError::DuplicateTag {
            tag: "system".to_owned()
        })
    );
    assert_eq!(
        DnsProfile::new(
            vec![system.clone()],
            Vec::new(),
            "missing",
            DnsStrategy::PreferIpv4,
            false,
            false,
        ),
        Err(DnsConfigError::UnknownServer {
            tag: "missing".to_owned()
        })
    );
    let rule = DnsRule::domain(vec!["example.com".to_owned()], "missing").unwrap();
    assert_eq!(
        DnsProfile::new(
            vec![system.clone()],
            vec![rule],
            "system",
            DnsStrategy::PreferIpv4,
            false,
            false,
        ),
        Err(DnsConfigError::UnknownServer {
            tag: "missing".to_owned()
        })
    );
    assert_eq!(
        DnsProfile::new(
            vec![system],
            Vec::new(),
            "system",
            DnsStrategy::Ipv6Only,
            false,
            false,
        ),
        Err(DnsConfigError::Ipv6StrategyRequiresIpv6)
    );
}

#[test]
fn rejects_missing_and_recursive_bootstrap_servers() {
    let missing = DnsServer::doh("doh", "dns.example", 443, "/dns-query", "missing").unwrap();
    assert_eq!(
        DnsProfile::new(
            vec![missing],
            Vec::new(),
            "doh",
            DnsStrategy::PreferIpv4,
            false,
            false,
        ),
        Err(DnsConfigError::UnknownBootstrap {
            tag: "missing".to_owned()
        })
    );

    let first = DnsServer::doh("first", "one.example", 443, "/dns-query", "second").unwrap();
    let second = DnsServer::dot("second", "two.example", 853, "first").unwrap();
    assert_eq!(
        DnsProfile::new(
            vec![first, second],
            Vec::new(),
            "first",
            DnsStrategy::PreferIpv4,
            false,
            false,
        ),
        Err(DnsConfigError::RecursiveBootstrap {
            tag: "first".to_owned()
        })
    );
}

#[test]
fn fake_ip_omits_ipv6_when_the_profile_disables_it() {
    let config = SingBoxDnsConfigGenerator::generate(&complete_profile(true, false));
    let fake_ip = config.json()["servers"].as_array().unwrap().last().unwrap();

    assert!(fake_ip.get("inet6_range").is_none());
    assert_eq!(config.json()["rules"][3]["query_type"], json!(["A"]));
}
