//! Covers the assembled Xray runtime config: which sections appear, when a node
//! is required, how fake IP reaches both DNS and sniffing, and the ordering the
//! stats API depends on.

use std::num::NonZeroU16;

use magies_domain::{CoreType, CredentialRef, ProxyNode};
use magies_profiles::{
    DnsProfile, DnsServer, DnsStrategy, LocalHttpProfile, LocalSocksProfile, ManualCredentialDraft,
    ManualNodeDraft, PlainDnsTransport, StoredNodeCredential, XrayRuntimeConfigError,
    XrayRuntimeConfigGenerator, XrayRuntimeProfile,
};
use magies_routing::{RouteOutbound, RouteProfile, RoutingMode};
use serde_json::Value;
use uuid::Uuid;

const USER_ID: &str = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";

fn node() -> (ProxyNode, StoredNodeCredential) {
    ManualNodeDraft {
        name: "Tokyo".to_owned(),
        server: "edge.example.com".to_owned(),
        port: 443,
        udp_enabled: true,
        transport: None,
        tls: None,
        credential: ManualCredentialDraft::Vless {
            user_id: Uuid::parse_str(USER_ID).unwrap(),
            flow: None,
        },
    }
    .build(Uuid::new_v4(), CredentialRef::new("node/test").unwrap())
    .unwrap()
}

fn dns(fake_ip: bool) -> DnsProfile {
    DnsProfile::new(
        vec![DnsServer::plain("primary", PlainDnsTransport::Udp, "1.1.1.1", 53, None).unwrap()],
        Vec::new(),
        "primary",
        DnsStrategy::PreferIpv4,
        fake_ip,
        false,
    )
    .unwrap()
}

fn global_route() -> RouteProfile {
    RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap()
}

fn direct_route() -> RouteProfile {
    RouteProfile::new(RoutingMode::Direct, Vec::new(), RouteOutbound::Direct).unwrap()
}

fn generate(profile: &XrayRuntimeProfile<'_>) -> Value {
    let generated = XrayRuntimeConfigGenerator::generate(profile).unwrap();
    assert_eq!(generated.core_type(), CoreType::Xray);
    generated.json().clone()
}

#[test]
fn a_global_profile_produces_every_section() {
    let (node, credential) = node();
    let dns = dns(false);
    let route = global_route();

    let config = generate(&XrayRuntimeProfile::new(
        &node,
        credential.as_node_credential(),
        &dns,
        &route,
    ));

    assert_eq!(config["log"]["loglevel"], "warning");
    assert!(config["dns"]["servers"].is_array());
    assert_eq!(config["inbounds"].as_array().unwrap().len(), 2);
    assert!(config["routing"]["rules"].is_array());
    // No sing-box sections leak through.
    assert!(config["route"].is_null());
    assert!(config["experimental"].is_null());
}

#[test]
fn the_proxy_outbound_comes_first_and_direct_is_freedom() {
    let (node, credential) = node();
    let dns = dns(false);
    let route = global_route();

    let config = generate(&XrayRuntimeProfile::new(
        &node,
        credential.as_node_credential(),
        &dns,
        &route,
    ));

    let outbounds = config["outbounds"].as_array().unwrap();
    assert_eq!(outbounds.len(), 2);
    assert_eq!(outbounds[0]["tag"], "proxy");
    assert_eq!(outbounds[0]["protocol"], "vless");
    // Xray's direct outbound is the freedom protocol.
    assert_eq!(outbounds[1]["protocol"], "freedom");
    assert_eq!(outbounds[1]["tag"], "direct");
}

#[test]
fn a_direct_only_profile_needs_no_node() {
    let dns = dns(false);
    let route = direct_route();

    let config = generate(&XrayRuntimeProfile::without_selected_node(&dns, &route));

    let outbounds = config["outbounds"].as_array().unwrap();
    assert_eq!(outbounds.len(), 1);
    assert_eq!(outbounds[0]["protocol"], "freedom");
}

#[test]
fn a_route_needing_the_proxy_without_a_node_is_refused() {
    let dns = dns(false);
    let route = global_route();

    let error = XrayRuntimeConfigGenerator::generate(&XrayRuntimeProfile::without_selected_node(
        &dns, &route,
    ))
    .unwrap_err();

    assert_eq!(error, XrayRuntimeConfigError::MissingSelectedNode);
}

#[test]
fn both_local_listeners_appear_with_sniffing() {
    let (node, credential) = node();
    let dns = dns(false);
    let route = global_route();

    let config = generate(&XrayRuntimeProfile::new(
        &node,
        credential.as_node_credential(),
        &dns,
        &route,
    ));

    let inbounds = config["inbounds"].as_array().unwrap();
    assert_eq!(inbounds[0]["protocol"], "socks");
    assert_eq!(inbounds[1]["protocol"], "http");
    for inbound in inbounds {
        assert_eq!(inbound["sniffing"]["enabled"], true);
        // Without fake IP the sniffer only needs the real protocols.
        assert_eq!(
            inbound["sniffing"]["destOverride"],
            serde_json::json!(["http", "tls"])
        );
    }
}

#[test]
fn enables_outbound_mux_when_requested() {
    let (node, credential) = node();
    let dns = dns(false);
    let route = global_route();

    let config = generate(
        &XrayRuntimeProfile::new(&node, credential.as_node_credential(), &dns, &route)
            .with_mux(true),
    );
    let proxy = config["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|outbound| outbound["tag"] == "proxy")
        .unwrap();
    assert_eq!(
        proxy["mux"],
        serde_json::json!({ "enabled": true, "concurrency": 8 })
    );
}

#[test]
fn enables_fragment_dialer_and_adds_the_freedom_outbound_when_requested() {
    let (node, credential) = node();
    let dns = dns(false);
    let route = global_route();

    let config = generate(
        &XrayRuntimeProfile::new(&node, credential.as_node_credential(), &dns, &route)
            .with_fragment(true),
    );
    let outbounds = config["outbounds"].as_array().unwrap();
    let proxy = outbounds
        .iter()
        .find(|outbound| outbound["tag"] == "proxy")
        .unwrap();
    assert_eq!(proxy["streamSettings"]["sockopt"]["dialerProxy"], "fragment");

    let fragment = outbounds
        .iter()
        .find(|outbound| outbound["tag"] == "fragment")
        .unwrap();
    assert_eq!(fragment["protocol"], "freedom");
    assert_eq!(fragment["settings"]["fragment"]["packets"], "tlshello");
    assert_eq!(fragment["settings"]["fragment"]["length"], "100-200");
    assert_eq!(fragment["settings"]["fragment"]["interval"], "10-20");
}

#[test]
fn enables_udp_noise_on_the_freedom_outbound_when_requested() {
    let (node, credential) = node();
    let dns = dns(false);
    let route = global_route();

    let config = generate(
        &XrayRuntimeProfile::new(&node, credential.as_node_credential(), &dns, &route)
            .with_udp_noise(true),
    );
    let outbounds = config["outbounds"].as_array().unwrap();
    let proxy = outbounds
        .iter()
        .find(|outbound| outbound["tag"] == "proxy")
        .unwrap();
    assert_eq!(proxy["streamSettings"]["sockopt"]["dialerProxy"], "fragment");

    let fragment = outbounds
        .iter()
        .find(|outbound| outbound["tag"] == "fragment")
        .unwrap();
    assert_eq!(fragment["protocol"], "freedom");
    assert!(fragment["settings"]["fragment"].is_null());
    assert_eq!(
        fragment["settings"]["noises"],
        serde_json::json!([{
            "type": "rand",
            "packet": "10-20",
            "delay": "10-16"
        }])
    );
}

#[test]
fn combines_fragment_and_udp_noise_on_one_freedom_outbound() {
    let (node, credential) = node();
    let dns = dns(false);
    let route = global_route();

    let config = generate(
        &XrayRuntimeProfile::new(&node, credential.as_node_credential(), &dns, &route)
            .with_fragment(true)
            .with_udp_noise(true),
    );
    let fragment = config["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|outbound| outbound["tag"] == "fragment")
        .unwrap();
    assert_eq!(fragment["settings"]["fragment"]["packets"], "tlshello");
    assert_eq!(
        fragment["settings"]["noises"][0]["type"],
        "rand"
    );
}

#[test]
fn no_fragment_outbound_appears_when_fragment_is_off() {
    let (node, credential) = node();
    let dns = dns(false);
    let route = global_route();

    let config = generate(&XrayRuntimeProfile::new(
        &node,
        credential.as_node_credential(),
        &dns,
        &route,
    ));

    let outbounds = config["outbounds"].as_array().unwrap();
    assert!(
        !outbounds
            .iter()
            .any(|outbound| outbound["tag"] == "fragment")
    );
    let proxy = outbounds
        .iter()
        .find(|outbound| outbound["tag"] == "proxy")
        .unwrap();
    assert!(proxy["streamSettings"]["sockopt"].is_null());
}

#[test]
fn duplicate_local_ports_are_refused() {
    let dns = dns(false);
    let route = direct_route();
    let port = NonZeroU16::new(10_808).unwrap();

    let result = XrayRuntimeProfile::without_selected_node(&dns, &route).with_local_proxies(
        LocalSocksProfile::new(port.get().into()).unwrap(),
        LocalHttpProfile::new(port.get().into()).unwrap(),
    );

    match result {
        Ok(_) => panic!("sharing a port between SOCKS and HTTP must be refused"),
        Err(error) => assert_eq!(
            error,
            XrayRuntimeConfigError::DuplicateLocalPort { port: 10_808 }
        ),
    }
}

#[test]
fn fake_ip_reaches_both_the_dns_servers_and_the_sniffer() {
    let (node, credential) = node();
    let dns = dns(true);
    let route = global_route();

    let config = generate(&XrayRuntimeProfile::new(
        &node,
        credential.as_node_credential(),
        &dns,
        &route,
    ));

    assert_eq!(config["dns"]["servers"][0], "fakedns");
    // The pool is what the fakedns server draws from; without it the sniffer
    // would resolve nothing.
    assert_eq!(config["fakedns"]["ipPool"], "198.18.0.0/15");
    for inbound in config["inbounds"].as_array().unwrap() {
        assert_eq!(
            inbound["sniffing"]["destOverride"],
            serde_json::json!(["http", "tls", "fakedns"])
        );
    }
}

#[test]
fn no_fakedns_object_appears_when_fake_ip_is_off() {
    let (node, credential) = node();
    let dns = dns(false);
    let route = global_route();

    let config = generate(&XrayRuntimeProfile::new(
        &node,
        credential.as_node_credential(),
        &dns,
        &route,
    ));

    assert!(config["fakedns"].is_null());
}

#[test]
fn the_stats_api_gets_its_own_inbound_and_the_first_routing_rule() {
    let (node, credential) = node();
    let dns = dns(false);
    let route = global_route();

    let config = generate(
        &XrayRuntimeProfile::new(&node, credential.as_node_credential(), &dns, &route)
            .with_api_port(NonZeroU16::new(9_090).unwrap()),
    );

    assert!(config["stats"].is_object());
    assert_eq!(
        config["api"]["services"],
        serde_json::json!(["StatsService"])
    );

    let inbounds = config["inbounds"].as_array().unwrap();
    assert_eq!(inbounds.len(), 3);
    assert_eq!(inbounds[2]["tag"], "api-in");
    assert_eq!(inbounds[2]["port"], 9_090);

    // The rule has to be first: the catch-all the route generator appends would
    // otherwise send API requests to the proxy.
    let rules = config["routing"]["rules"].as_array().unwrap();
    assert_eq!(rules[0]["inboundTag"], serde_json::json!(["api-in"]));
    assert_eq!(rules[0]["outboundTag"], "api");
}

#[test]
fn no_api_sections_appear_without_a_port() {
    let (node, credential) = node();
    let dns = dns(false);
    let route = global_route();

    let config = generate(&XrayRuntimeProfile::new(
        &node,
        credential.as_node_credential(),
        &dns,
        &route,
    ));

    assert!(config["stats"].is_null());
    assert!(config["api"].is_null());
    assert!(config["policy"].is_null());
    assert_eq!(config["inbounds"].as_array().unwrap().len(), 2);
}

#[test]
fn the_generated_config_never_mentions_the_credential() {
    let (node, credential) = node();
    let dns = dns(false);
    let route = global_route();

    let config = generate(&XrayRuntimeProfile::new(
        &node,
        credential.as_node_credential(),
        &dns,
        &route,
    ));

    // The UUID is the credential here, so it must appear exactly where the
    // outbound needs it and nowhere else.
    let text = config.to_string();
    assert_eq!(text.matches(USER_ID).count(), 1);
}
