//! Feeds a complete generated Xray configuration to a real Xray binary.
//!
//! The outbound smoke test checks one section in isolation; this checks that
//! the assembled document — inbounds, outbounds, routing, DNS, and the optional
//! stats API — is something Xray will actually start with.

use std::fs;
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};
use std::process::Command;

use magies_domain::{CredentialRef, ProxyNode, TlsConfig};
use magies_profiles::{
    DnsProfile, DnsRule, DnsServer, DnsStrategy, ManualCredentialDraft, ManualNodeDraft,
    PlainDnsTransport, StoredNodeCredential, XrayRuntimeConfigGenerator, XrayRuntimeProfile,
};
use magies_routing::{Network, RouteOutbound, RouteProfile, RoutingMode, RoutingRule};
use serde_json::Value;
use uuid::Uuid;

const USER_ID: &str = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";

fn binary() -> PathBuf {
    PathBuf::from(
        std::env::var_os("MAGIES_XRAY_BIN").expect("MAGIES_XRAY_BIN must point to official Xray"),
    )
}

fn node() -> (ProxyNode, StoredNodeCredential) {
    ManualNodeDraft {
        name: "Smoke".to_owned(),
        server: "edge.example.com".to_owned(),
        port: 443,
        udp_enabled: true,
        transport: None,
        tls: Some(TlsConfig::Tls {
            server_name: Some("edge.example.com".to_owned()),
            allow_insecure: false,
            alpn: vec!["h2".to_owned()],
            fingerprint: Some("chrome".to_owned()),
        }),
        credential: ManualCredentialDraft::Vless {
            user_id: Uuid::parse_str(USER_ID).unwrap(),
            flow: None,
        },
    }
    .build(Uuid::new_v4(), CredentialRef::new("node/smoke").unwrap())
    .unwrap()
}

/// A profile exercising every DNS server kind the model supports.
fn rich_dns(fake_ip: bool) -> DnsProfile {
    DnsProfile::new(
        vec![
            DnsServer::system("system").unwrap(),
            DnsServer::plain("china", PlainDnsTransport::Udp, "223.5.5.5", 53, None).unwrap(),
            DnsServer::doh("doh", "dns.google", 443, "/dns-query", "system").unwrap(),
            DnsServer::dot("dot", "one.one.one.one", 853, "system").unwrap(),
        ],
        vec![
            DnsRule::domain_suffix(vec![".cn".to_owned()], "china").unwrap(),
            DnsRule::domain_keyword(vec!["google".to_owned()], "doh").unwrap(),
        ],
        "dot",
        DnsStrategy::PreferIpv4,
        fake_ip,
        false,
    )
    .unwrap()
}

/// A profile exercising every routing matcher.
fn rich_route() -> RouteProfile {
    RouteProfile::new(
        RoutingMode::Rule,
        vec![
            RoutingRule::domain("example.com", RouteOutbound::Proxy, 10, true).unwrap(),
            RoutingRule::domain_suffix("cn", RouteOutbound::Direct, 20, true).unwrap(),
            RoutingRule::domain_keyword("ads", RouteOutbound::Direct, 30, true).unwrap(),
            RoutingRule::ip_cidr("10.0.0.0/8", RouteOutbound::Direct, 40, true).unwrap(),
            RoutingRule::ip_cidr6("2001:db8::/32", RouteOutbound::Proxy, 50, true).unwrap(),
            RoutingRule::port(443, RouteOutbound::Proxy, 60, true).unwrap(),
            RoutingRule::network(Network::Udp, RouteOutbound::Direct, 70, true),
            RoutingRule::geo_ip("CN", RouteOutbound::Direct, 80, true).unwrap(),
            RoutingRule::geo_site("google", RouteOutbound::Proxy, 90, true).unwrap(),
        ],
        RouteOutbound::Proxy,
    )
    .unwrap()
}

fn check(config: &Value, name: &str) -> (bool, String) {
    let path = std::env::temp_dir().join(format!(
        "magies-xray-runtime-{name}-{}.json",
        std::process::id()
    ));
    fs::write(&path, serde_json::to_vec_pretty(config).unwrap()).unwrap();
    let result = run_check(&binary(), &path);
    let _ = fs::remove_file(&path);
    result
}

fn run_check(binary: &Path, path: &Path) -> (bool, String) {
    let output = Command::new(binary)
        .arg("run")
        .arg("-test")
        .arg("-c")
        .arg(path)
        .output()
        .expect("Xray should be runnable");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.success(), text)
}

#[test]
#[ignore = "requires MAGIES_XRAY_BIN pointing to an official Xray build"]
fn official_xray_accepts_a_global_configuration() {
    let (node, credential) = node();
    let dns = rich_dns(false);
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let config = XrayRuntimeConfigGenerator::generate(&XrayRuntimeProfile::new(
        &node,
        credential.as_node_credential(),
        &dns,
        &route,
    ))
    .unwrap();

    let (accepted, output) = check(config.json(), "global");

    assert!(
        accepted,
        "Xray rejected the global configuration:\n{output}"
    );
}

#[test]
#[ignore = "requires MAGIES_XRAY_BIN pointing to an official Xray build"]
fn official_xray_accepts_every_routing_matcher_and_dns_kind() {
    let (node, credential) = node();
    let dns = rich_dns(false);
    let route = rich_route();
    let config = XrayRuntimeConfigGenerator::generate(&XrayRuntimeProfile::new(
        &node,
        credential.as_node_credential(),
        &dns,
        &route,
    ))
    .unwrap();

    let (accepted, output) = check(config.json(), "rule-mode");

    assert!(
        accepted,
        "Xray rejected the rule-mode configuration:\n{output}"
    );
}

#[test]
#[ignore = "requires MAGIES_XRAY_BIN pointing to an official Xray build"]
fn official_xray_accepts_fake_dns_and_the_stats_api() {
    let (node, credential) = node();
    let dns = rich_dns(true);
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let config = XrayRuntimeConfigGenerator::generate(
        &XrayRuntimeProfile::new(&node, credential.as_node_credential(), &dns, &route)
            .with_api_port(NonZeroU16::new(9_090).unwrap()),
    )
    .unwrap();

    let (accepted, output) = check(config.json(), "fakedns-api");

    assert!(
        accepted,
        "Xray rejected the fake-DNS and stats configuration:\n{output}"
    );
}

#[test]
#[ignore = "requires MAGIES_XRAY_BIN pointing to an official Xray build"]
fn official_xray_accepts_a_direct_only_configuration() {
    let dns = rich_dns(false);
    let route = RouteProfile::new(RoutingMode::Direct, Vec::new(), RouteOutbound::Direct).unwrap();
    let config = XrayRuntimeConfigGenerator::generate(&XrayRuntimeProfile::without_selected_node(
        &dns, &route,
    ))
    .unwrap();

    let (accepted, output) = check(config.json(), "direct");

    assert!(
        accepted,
        "Xray rejected the direct configuration:\n{output}"
    );
}
