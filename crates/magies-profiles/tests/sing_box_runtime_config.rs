use std::num::NonZeroU16;

use magies_domain::{
    CoreType, CredentialRef, ProxyNode, ProxyProtocol, ServerAddress, TransportConfig,
};
use magies_platform::OperatingSystem;
use magies_profiles::{
    DnsProfile, DnsServer, DnsStrategy, GroupProbe, LocalHttpProfile, LocalSocksProfile,
    NodeCredential, NodeGroupStrategy, OutboundConfigError, RuntimeConfigError, ShadowsocksParser,
    SingBoxRuntimeConfigGenerator, SingBoxRuntimeProfile, TunProfile, VlessParser,
};
use magies_routing::{RouteOutbound, RouteProfile, RoutingMode, RoutingRule, SniffedProtocol};
use serde_json::json;
use uuid::Uuid;

#[test]
fn composes_local_proxies_selected_node_dns_and_ordered_route() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:proxy-secret@edge.example.com:8388")
        .unwrap();
    let node = shadowsocks_node(true);
    let dns = system_dns();
    let route = RouteProfile::new(
        RoutingMode::Rule,
        vec![RoutingRule::domain_suffix("cn", RouteOutbound::Direct, 10, true).unwrap()],
        RouteOutbound::Proxy,
    )
    .unwrap();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    )
    .with_local_proxies(
        LocalSocksProfile::new(12_080).unwrap(),
        LocalHttpProfile::new(12_081).unwrap(),
    )
    .unwrap();

    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();

    assert_eq!(generated.core_type(), CoreType::SingBox);
    assert_eq!(
        generated.json(),
        &json!({
            "log": { "level": "warn" },
            "dns": {
                "servers": [{ "type": "local", "tag": "system" }],
                "rules": [],
                "final": "system",
                "strategy": "prefer_ipv4"
            },
            "inbounds": [
                {
                    "type": "socks",
                    "tag": "socks-in",
                    "listen": "127.0.0.1",
                    "listen_port": 12080
                },
                {
                    "type": "http",
                    "tag": "http-in",
                    "listen": "127.0.0.1",
                    "listen_port": 12081,
                    "set_system_proxy": false
                }
            ],
            "outbounds": [
                {
                    "type": "shadowsocks",
                    "tag": "proxy",
                    "server": "edge.example.com",
                    "server_port": 8388,
                    "method": "aes-256-gcm",
                    "password": "proxy-secret"
                },
                { "type": "direct", "tag": "direct" }
            ],
            "route": {
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
                    }
                ],
                "final": "proxy",
                "auto_detect_interface": true,
                "default_domain_resolver": "system"
            }
        })
    );
}

#[test]
fn enables_a_loopback_clash_api_without_sharing_listener_ports() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:proxy-secret@edge.example.com:8388")
        .unwrap();
    let node = shadowsocks_node(true);
    let dns = system_dns();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    )
    .with_clash_api_port(NonZeroU16::new(9_090).unwrap())
    .unwrap();

    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();

    assert_eq!(
        generated.json()["experimental"]["clash_api"],
        json!({ "external_controller": "127.0.0.1:9090" })
    );
    assert!(matches!(
        SingBoxRuntimeProfile::new(
            &node,
            NodeCredential::from(parsed.credential()),
            &dns,
            &route,
        )
        .with_clash_api_port(NonZeroU16::new(10_808).unwrap()),
        Err(RuntimeConfigError::DuplicateLocalPort { port: 10_808 })
    ));
}

#[test]
fn enables_outbound_multiplex_when_requested() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:proxy-secret@edge.example.com:8388")
        .unwrap();
    let node = shadowsocks_node(true);
    let dns = system_dns();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    )
    .with_mux(true);

    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();
    let proxy = generated.json()["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|outbound| outbound["tag"] == "proxy")
        .unwrap();
    assert_eq!(
        proxy["multiplex"],
        json!({ "enabled": true, "protocol": "h2mux" })
    );
}

#[test]
fn enables_tls_fragment_on_a_tls_carrying_outbound_when_requested() {
    let user_id = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";
    let parsed = VlessParser
        .parse(&format!(
            "vless://{user_id}@edge.example.com:443?type=tcp&security=tls&sni=www.example.com"
        ))
        .unwrap();
    let mut node = shadowsocks_node(true);
    node.protocol_type = ProxyProtocol::Vless;
    node.transport = Some(parsed.transport().clone());
    node.tls = parsed.tls().cloned();
    let dns = system_dns();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    )
    .with_fragment(true);

    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();
    let proxy = generated.json()["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|outbound| outbound["tag"] == "proxy")
        .unwrap();
    assert_eq!(proxy["tls"]["fragment"], true);
    assert_eq!(proxy["tls"]["record_fragment"], true);
}

#[test]
fn enables_final_fragment_route_options_when_requested() {
    let user_id = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";
    let parsed = VlessParser
        .parse(&format!(
            "vless://{user_id}@edge.example.com:443?type=tcp&security=tls&sni=www.example.com"
        ))
        .unwrap();
    let mut node = shadowsocks_node(true);
    node.protocol_type = ProxyProtocol::Vless;
    node.transport = Some(parsed.transport().clone());
    node.tls = parsed.tls().cloned();
    let dns = system_dns();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    )
    .with_final_fragment(true);

    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();
    let rule = &generated.json()["route"]["rules"][0];
    assert_eq!(rule["action"], "route-options");
    assert_eq!(rule["tls_record_fragment"], true);
}

#[test]
fn fragment_is_skipped_silently_when_the_outbound_has_no_tls() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:proxy-secret@edge.example.com:8388")
        .unwrap();
    let node = shadowsocks_node(true);
    let dns = system_dns();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    )
    .with_fragment(true);

    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();
    let proxy = generated.json()["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|outbound| outbound["tag"] == "proxy")
        .unwrap();
    assert!(proxy["tls"].is_null());
}

#[test]
fn adds_tun_and_prepends_sniff_and_dns_hijack_actions() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:password@192.0.2.1:8388")
        .unwrap();
    let node = shadowsocks_node(true);
    let dns = system_dns();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let tun = TunProfile::new(OperatingSystem::Windows, false, 1500, true, true).unwrap();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    )
    .with_tun(&tun, true);

    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();

    assert_eq!(generated.json()["inbounds"].as_array().unwrap().len(), 3);
    assert_eq!(generated.json()["inbounds"][2]["type"], "tun");
    assert_eq!(generated.json()["inbounds"][2]["interface_name"], "MgClash");
    assert_eq!(
        generated.json()["route"]["rules"],
        json!([
            { "action": "sniff" },
            { "protocol": "dns", "action": "hijack-dns" },
            {
                "ip_is_private": true,
                "action": "route",
                "outbound": "direct"
            }
        ])
    );
    assert_eq!(
        generated.json()["route"]["default_domain_resolver"],
        "system"
    );
    assert_eq!(generated.json()["route"]["final"], "proxy");
}

#[test]
fn rejects_duplicate_local_ports_and_propagates_outbound_errors() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:password@edge.example.com:8388")
        .unwrap();
    let mut node = shadowsocks_node(false);
    let dns = system_dns();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    );
    assert!(matches!(
        profile.with_local_proxies(
            LocalSocksProfile::new(10_808).unwrap(),
            LocalHttpProfile::new(10_808).unwrap()
        ),
        Err(RuntimeConfigError::DuplicateLocalPort { port: 10_808 })
    ));

    node.enabled = false;
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    );
    assert_eq!(
        SingBoxRuntimeConfigGenerator::generate(&profile),
        Err(RuntimeConfigError::Outbound(
            OutboundConfigError::NodeDisabled
        ))
    );
}

#[test]
fn direct_mode_runs_without_a_selected_node_but_proxy_routes_do_not() {
    let dns = system_dns();
    let direct_route =
        RouteProfile::new(RoutingMode::Direct, Vec::new(), RouteOutbound::Direct).unwrap();
    let direct = SingBoxRuntimeProfile::without_selected_node(&dns, &direct_route);
    let generated = SingBoxRuntimeConfigGenerator::generate(&direct).unwrap();

    assert_eq!(
        generated.json()["outbounds"],
        json!([{ "type": "direct", "tag": "direct" }])
    );
    assert_eq!(generated.json()["route"]["final"], "direct");

    let global_route =
        RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let missing = SingBoxRuntimeProfile::without_selected_node(&dns, &global_route);
    assert_eq!(
        SingBoxRuntimeConfigGenerator::generate(&missing),
        Err(RuntimeConfigError::MissingSelectedNode)
    );
}

#[test]
fn urltest_group_emits_member_outbounds_and_a_proxy_urltest() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:proxy-secret@edge.example.com:8388")
        .unwrap();
    let second = ShadowsocksParser
        .parse("ss://aes-256-gcm:other-secret@other.example.com:8388")
        .unwrap();
    let node = shadowsocks_node(true);
    let mut other = shadowsocks_node(true);
    other.id = Uuid::parse_str("018f78b5-08ee-7caa-94f3-1d5d781aba23").unwrap();
    other.server = ServerAddress::new("other.example.com").unwrap();
    let dns = system_dns();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let probe = GroupProbe::default();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    )
    .with_urltest(
        vec![
            (&node, NodeCredential::from(parsed.credential())),
            (&other, NodeCredential::from(second.credential())),
        ],
        &probe,
    );

    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();
    let outbounds = generated.json()["outbounds"].as_array().unwrap();
    assert_eq!(outbounds[0]["tag"], format!("node-{}", node.id));
    assert_eq!(outbounds[1]["tag"], format!("node-{}", other.id));
    assert_eq!(outbounds[2]["type"], "urltest");
    assert_eq!(outbounds[2]["tag"], "proxy");
    assert_eq!(
        outbounds[2]["outbounds"],
        json!([format!("node-{}", node.id), format!("node-{}", other.id)])
    );
    assert_eq!(outbounds[3]["tag"], "direct");
}

#[test]
fn fallback_group_emits_member_outbounds_and_a_proxy_fallback() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:proxy-secret@edge.example.com:8388")
        .unwrap();
    let second = ShadowsocksParser
        .parse("ss://aes-256-gcm:other-secret@other.example.com:8388")
        .unwrap();
    let node = shadowsocks_node(true);
    let mut other = shadowsocks_node(true);
    other.id = Uuid::parse_str("018f78b5-08ee-7caa-94f3-1d5d781aba23").unwrap();
    other.server = ServerAddress::new("other.example.com").unwrap();
    let dns = system_dns();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let probe = GroupProbe::default();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    )
    .with_group_outbound(
        NodeGroupStrategy::Fallback,
        vec![
            (&node, NodeCredential::from(parsed.credential())),
            (&other, NodeCredential::from(second.credential())),
        ],
        &probe,
    );

    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();
    let outbounds = generated.json()["outbounds"].as_array().unwrap();
    assert_eq!(outbounds[2]["type"], "fallback");
    assert_eq!(outbounds[2]["tag"], "proxy");
}

#[test]
fn loadbalance_group_emits_member_outbounds_and_a_proxy_loadbalance() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:proxy-secret@edge.example.com:8388")
        .unwrap();
    let second = ShadowsocksParser
        .parse("ss://aes-256-gcm:other-secret@other.example.com:8388")
        .unwrap();
    let node = shadowsocks_node(true);
    let mut other = shadowsocks_node(true);
    other.id = Uuid::parse_str("018f78b5-08ee-7caa-94f3-1d5d781aba23").unwrap();
    other.server = ServerAddress::new("other.example.com").unwrap();
    let dns = system_dns();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let probe = GroupProbe::default();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    )
    .with_group_outbound(
        NodeGroupStrategy::LoadBalance,
        vec![
            (&node, NodeCredential::from(parsed.credential())),
            (&other, NodeCredential::from(second.credential())),
        ],
        &probe,
    );

    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();
    let outbounds = generated.json()["outbounds"].as_array().unwrap();
    assert_eq!(outbounds[2]["type"], "loadbalance");
    assert_eq!(outbounds[2]["strategy"], "round-robin");
    assert_eq!(outbounds[2]["tag"], "proxy");
}

#[test]
fn generated_runtime_debug_never_exposes_node_credentials() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:top-secret@edge.example.com:8388")
        .unwrap();
    let node = shadowsocks_node(true);
    let dns = system_dns();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    );
    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();

    let debug = format!("{generated:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("top-secret"));
}

fn system_dns() -> DnsProfile {
    DnsProfile::new(
        vec![DnsServer::system("system").unwrap()],
        Vec::new(),
        "system",
        DnsStrategy::PreferIpv4,
        false,
        false,
    )
    .unwrap()
}

fn shadowsocks_node(enabled: bool) -> ProxyNode {
    let mut node = ProxyNode::new(
        Uuid::parse_str("018f78b5-08ee-7caa-94f3-1d5d781aba22").unwrap(),
        "Selected node",
        ProxyProtocol::Shadowsocks,
        "edge.example.com",
        8388,
        Some(CredentialRef::new("keychain://nodes/selected").unwrap()),
    )
    .unwrap();
    node.transport = Some(TransportConfig::Tcp);
    node.enabled = enabled;
    node
}

#[test]
fn the_urltest_group_uses_the_configured_interval_and_tolerance() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:proxy-secret@edge.example.com:8388")
        .unwrap();
    let second = ShadowsocksParser
        .parse("ss://aes-256-gcm:other-secret@other.example.com:8388")
        .unwrap();
    let node = shadowsocks_node(true);
    let mut other = shadowsocks_node(true);
    other.id = Uuid::parse_str("018f78b5-08ee-7caa-94f3-1d5d781aba23").unwrap();
    other.server = ServerAddress::new("other.example.com").unwrap();
    let dns = system_dns();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let probe = GroupProbe::new("https://probe.example.com", 600, 150).unwrap();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    )
    .with_urltest(
        vec![
            (&node, NodeCredential::from(parsed.credential())),
            (&other, NodeCredential::from(second.credential())),
        ],
        &probe,
    );

    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();
    let group = &generated.json()["outbounds"][2];
    assert_eq!(group["url"], "https://probe.example.com");
    assert_eq!(group["interval"], "10m");
    assert_eq!(group["tolerance"], 150);
}

#[test]
fn a_probe_the_core_would_reject_is_refused() {
    // sing-box polls on this interval and compares against this tolerance; the
    // window must not be able to ask for a value the Core will not accept.
    assert!(GroupProbe::new("https://probe.example.com", 0, 50).is_err());
    assert!(GroupProbe::new("https://probe.example.com", 600, 60_000).is_err());
    assert!(GroupProbe::new("not-a-url", 600, 50).is_err());
    // An empty URL keeps the built-in probe rather than failing.
    assert_eq!(
        GroupProbe::new("", 180, 50).unwrap().url(),
        "https://www.gstatic.com/generate_204"
    );
}

#[test]
fn a_front_node_becomes_the_detour_the_proxy_dials_through() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:proxy-secret@edge.example.com:8388")
        .unwrap();
    let front_parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:front-secret@front.example.com:8388")
        .unwrap();
    let node = shadowsocks_node(true);
    let mut front = shadowsocks_node(true);
    front.id = Uuid::parse_str("018f78b5-08ee-7caa-94f3-1d5d781aba24").unwrap();
    front.server = ServerAddress::new("front.example.com").unwrap();
    let dns = system_dns();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    )
    .with_front_node(&front, NodeCredential::from(front_parsed.credential()));

    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();
    let outbounds = generated.json()["outbounds"].as_array().unwrap();
    let front_tag = format!("front-{}", front.id);
    // The front dials out directly; the proxy reaches the internet through it.
    let front_outbound = outbounds
        .iter()
        .find(|outbound| outbound["tag"] == front_tag)
        .expect("the front node has its own outbound");
    assert_eq!(front_outbound["server"], "front.example.com");
    assert!(front_outbound["detour"].is_null());
    let proxy = outbounds
        .iter()
        .find(|outbound| outbound["tag"] == "proxy")
        .unwrap();
    assert_eq!(proxy["detour"], front_tag);
}

#[test]
fn a_protocol_rule_turns_sniffing_on_without_a_tun_inbound() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:proxy-secret@edge.example.com:8388")
        .unwrap();
    let node = shadowsocks_node(true);
    let dns = system_dns();
    let route = RouteProfile::new(
        RoutingMode::Rule,
        vec![RoutingRule::protocol(
            SniffedProtocol::Bittorrent,
            RouteOutbound::Direct,
            10,
            true,
        )],
        RouteOutbound::Proxy,
    )
    .unwrap();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    );

    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();

    // Without this action sing-box never looks at what the traffic is, and the
    // rule below it silently matches nothing.
    assert_eq!(
        generated.json()["route"]["rules"][0],
        json!({ "action": "sniff" })
    );
    assert_eq!(
        generated.json()["route"]["rules"][2]["protocol"],
        json!(["bittorrent"])
    );
}

#[test]
fn a_route_that_asks_nothing_about_the_traffic_does_not_sniff() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:proxy-secret@edge.example.com:8388")
        .unwrap();
    let node = shadowsocks_node(true);
    let dns = system_dns();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    );

    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();

    let rules = generated.json()["route"]["rules"]
        .as_array()
        .unwrap()
        .clone();
    assert!(
        !rules.iter().any(|rule| rule["action"] == "sniff"),
        "{rules:?}"
    );
}
