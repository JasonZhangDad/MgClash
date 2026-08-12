//! Covers the Xray DNS generator: how tagged servers and separate rules become
//! Xray's per-server `domains`, how the final server becomes the catch-all, and
//! where the model's meaning cannot be carried across.

use magies_profiles::{
    DnsProfile, DnsRule, DnsServer, DnsStrategy, PlainDnsTransport, XrayDnsConfigGenerator,
};
use serde_json::{Value, json};

fn generate(profile: &DnsProfile) -> Value {
    XrayDnsConfigGenerator::generate(profile).json().clone()
}

fn servers(profile: &DnsProfile) -> Vec<Value> {
    generate(profile)["servers"].as_array().unwrap().clone()
}

fn plain(tag: &str, server: &str) -> DnsServer {
    DnsServer::plain(tag, PlainDnsTransport::Udp, server, 53, None).unwrap()
}

fn profile(servers: Vec<DnsServer>, rules: Vec<DnsRule>, final_server: &str) -> DnsProfile {
    DnsProfile::new(
        servers,
        rules,
        final_server,
        DnsStrategy::PreferIpv4,
        false,
        false,
    )
    .unwrap()
}

#[test]
fn a_plain_server_becomes_an_address_and_port() {
    let profile = profile(vec![plain("primary", "1.1.1.1")], Vec::new(), "primary");

    assert_eq!(
        servers(&profile),
        vec![json!({ "address": "1.1.1.1", "port": 53 })]
    );
}

#[test]
fn the_system_resolver_becomes_localhost() {
    let profile = profile(
        vec![DnsServer::system("system").unwrap()],
        Vec::new(),
        "system",
    );

    assert_eq!(servers(&profile), vec![json!("localhost")]);
}

#[test]
fn tcp_doh_and_dot_carry_their_scheme_on_the_address() {
    let tcp = profile(
        vec![DnsServer::plain("tcp", PlainDnsTransport::Tcp, "9.9.9.9", 53, None).unwrap()],
        Vec::new(),
        "tcp",
    );
    assert_eq!(
        servers(&tcp),
        vec![json!({ "address": "tcp://9.9.9.9", "port": 53 })]
    );

    // DoH and DoT need a bootstrap resolver that exists in the same profile.
    let doh = profile(
        vec![
            DnsServer::system("system").unwrap(),
            DnsServer::doh("doh", "dns.google", 443, "/dns-query", "system").unwrap(),
        ],
        Vec::new(),
        "doh",
    );
    assert_eq!(
        servers(&doh),
        vec![
            json!("localhost"),
            json!({ "address": "https://dns.google/dns-query", "port": 443 }),
        ]
    );

    let dot = profile(
        vec![
            DnsServer::system("system").unwrap(),
            DnsServer::dot("dot", "one.one.one.one", 853, "system").unwrap(),
        ],
        Vec::new(),
        "dot",
    );
    assert_eq!(
        servers(&dot),
        vec![
            json!("localhost"),
            json!({ "address": "tls://one.one.one.one", "port": 853 }),
        ]
    );
}

#[test]
fn rules_become_the_domains_of_the_server_they_target() {
    let profile = profile(
        vec![plain("china", "223.5.5.5"), plain("global", "1.1.1.1")],
        vec![
            DnsRule::domain(vec!["example.cn".to_owned()], "china").unwrap(),
            DnsRule::domain_suffix(vec![".cn".to_owned()], "china").unwrap(),
            DnsRule::domain_keyword(vec!["baidu".to_owned()], "china").unwrap(),
        ],
        "global",
    );

    let servers = servers(&profile);
    let china = servers
        .iter()
        .find(|server| server["address"] == "223.5.5.5")
        .unwrap();

    assert_eq!(
        china["domains"],
        json!(["full:example.cn", "domain:cn", "keyword:baidu"])
    );
}

#[test]
fn the_final_server_answers_everything_and_comes_last() {
    let profile = profile(
        vec![plain("china", "223.5.5.5"), plain("global", "1.1.1.1")],
        vec![DnsRule::domain_suffix(vec![".cn".to_owned()], "china").unwrap()],
        "global",
    );

    let servers = servers(&profile);

    // Xray tries servers in order and a server without `domains` answers
    // anything, so the fallback has to be last and unrestricted.
    assert_eq!(servers.last().unwrap()["address"], "1.1.1.1");
    assert!(servers.last().unwrap()["domains"].is_null());
    assert!(!servers[0]["domains"].is_null());
}

#[test]
fn a_final_server_keeps_no_domains_even_when_rules_name_it() {
    let profile = profile(
        vec![plain("only", "1.1.1.1")],
        vec![DnsRule::domain_suffix(vec![".cn".to_owned()], "only").unwrap()],
        "only",
    );

    let servers = servers(&profile);

    // Restricting the fallback would leave everything else unanswered.
    assert!(servers[0]["domains"].is_null());
}

#[test]
fn fake_ip_puts_the_fakedns_server_first() {
    let profile = DnsProfile::new(
        vec![plain("global", "1.1.1.1")],
        Vec::new(),
        "global",
        DnsStrategy::PreferIpv4,
        true,
        false,
    )
    .unwrap();

    let servers = servers(&profile);

    assert_eq!(servers[0], json!("fakedns"));
    assert_eq!(servers[1]["address"], "1.1.1.1");
}

#[test]
fn fake_ip_still_places_the_final_server_last() {
    let profile = DnsProfile::new(
        vec![plain("china", "223.5.5.5"), plain("global", "1.1.1.1")],
        vec![DnsRule::domain_suffix(vec![".cn".to_owned()], "china").unwrap()],
        "global",
        DnsStrategy::PreferIpv4,
        true,
        false,
    )
    .unwrap();

    let servers = servers(&profile);

    assert_eq!(servers[0], json!("fakedns"));
    assert_eq!(servers.last().unwrap()["address"], "1.1.1.1");
}

#[test]
fn hard_strategies_map_directly() {
    for (strategy, expected) in [
        (DnsStrategy::Ipv4Only, "UseIPv4"),
        (DnsStrategy::Ipv6Only, "UseIPv6"),
    ] {
        let profile = DnsProfile::new(
            vec![plain("only", "1.1.1.1")],
            Vec::new(),
            "only",
            strategy,
            false,
            true,
        )
        .unwrap();

        assert_eq!(generate(&profile)["queryStrategy"], expected);
    }
}

#[test]
fn preference_strategies_collapse_because_xray_has_no_equivalent() {
    for strategy in [DnsStrategy::PreferIpv4, DnsStrategy::PreferIpv6] {
        let profile = DnsProfile::new(
            vec![plain("only", "1.1.1.1")],
            Vec::new(),
            "only",
            strategy,
            false,
            true,
        )
        .unwrap();

        // Xray offers only hard choices, so the ordering hint is lost. This is
        // pinned so the loss stays visible rather than being rediscovered.
        assert_eq!(generate(&profile)["queryStrategy"], "UseIP");
    }
}

#[test]
fn the_generated_section_has_no_tags_or_final_field() {
    let profile = profile(
        vec![plain("china", "223.5.5.5"), plain("global", "1.1.1.1")],
        vec![DnsRule::domain_suffix(vec![".cn".to_owned()], "china").unwrap()],
        "global",
    );

    let generated = generate(&profile);

    // Both are sing-box concepts with no Xray counterpart.
    assert!(generated["final"].is_null());
    assert!(!generated.to_string().contains("\"tag\""));
}
