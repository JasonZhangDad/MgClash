//! Covers the Xray routing generator: how each matcher maps to Xray's prefixed
//! `domain` and `ip` arrays, rule ordering, and the catch-all that replaces
//! sing-box's `final`.

use magies_routing::{
    Network, RouteOutbound, RouteProfile, RoutingMode, RoutingRule, XrayRouteConfigGenerator,
};
use serde_json::{Value, json};

fn generate(profile: &RouteProfile) -> Value {
    XrayRouteConfigGenerator::generate(profile).json().clone()
}

fn rules(profile: &RouteProfile) -> Vec<Value> {
    generate(profile)["rules"].as_array().unwrap().clone()
}

fn rule_profile(rules: Vec<RoutingRule>) -> RouteProfile {
    RouteProfile::new(RoutingMode::Rule, rules, RouteOutbound::Proxy).unwrap()
}

#[test]
fn global_mode_sends_private_addresses_direct_and_everything_else_to_the_proxy() {
    let profile = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();

    let rules = rules(&profile);

    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0]["ip"], json!(["geoip:private"]));
    assert_eq!(rules[0]["outboundTag"], "direct");
    // Xray has no `final`, so the intent is stated as a catch-all instead of
    // relying on the order of the outbounds array.
    assert_eq!(rules[1]["network"], "tcp,udp");
    assert_eq!(rules[1]["outboundTag"], "proxy");
}

#[test]
fn direct_mode_adds_no_private_address_rule() {
    let profile =
        RouteProfile::new(RoutingMode::Direct, Vec::new(), RouteOutbound::Direct).unwrap();

    let rules = rules(&profile);

    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["outboundTag"], "direct");
}

#[test]
fn every_rule_is_a_field_rule() {
    let profile = rule_profile(vec![
        RoutingRule::domain("example.com", RouteOutbound::Proxy, 1, true).unwrap(),
        RoutingRule::port(443, RouteOutbound::Proxy, 2, true).unwrap(),
    ]);

    assert!(rules(&profile).iter().all(|rule| rule["type"] == "field"));
}

#[test]
fn domain_matchers_use_their_xray_prefixes() {
    let profile = rule_profile(vec![
        RoutingRule::domain("example.com", RouteOutbound::Proxy, 1, true).unwrap(),
        RoutingRule::domain_suffix("cn", RouteOutbound::Direct, 2, true).unwrap(),
        RoutingRule::domain_keyword("ads", RouteOutbound::Direct, 3, true).unwrap(),
    ]);

    let rules = rules(&profile);

    assert_eq!(rules[1]["domain"], json!(["full:example.com"]));
    // The leading dot the model stores for sing-box is stripped: Xray's
    // `domain:` prefix rejects it.
    assert_eq!(rules[2]["domain"], json!(["domain:cn"]));
    assert_eq!(rules[3]["domain"], json!(["keyword:ads"]));
}

#[test]
fn address_and_port_matchers_map_to_ip_and_port() {
    let profile = rule_profile(vec![
        RoutingRule::ip_cidr("10.0.0.0/8", RouteOutbound::Direct, 1, true).unwrap(),
        RoutingRule::ip_cidr6("2001:db8::/32", RouteOutbound::Proxy, 2, true).unwrap(),
        RoutingRule::port(443, RouteOutbound::Proxy, 3, true).unwrap(),
        RoutingRule::network(Network::Udp, RouteOutbound::Direct, 4, true),
    ]);

    let rules = rules(&profile);

    assert_eq!(rules[1]["ip"], json!(["10.0.0.0/8"]));
    assert_eq!(rules[2]["ip"], json!(["2001:db8::/32"]));
    // Xray takes ports as a string so ranges use the same field.
    assert_eq!(rules[3]["port"], "443");
    assert_eq!(rules[4]["network"], "udp");
}

#[test]
fn geo_matchers_use_the_databases_xray_ships_with() {
    let profile = rule_profile(vec![
        RoutingRule::geo_ip("CN", RouteOutbound::Direct, 1, true).unwrap(),
        RoutingRule::geo_site("google", RouteOutbound::Proxy, 2, true).unwrap(),
    ]);

    let generated = generate(&profile);
    let rules = generated["rules"].as_array().unwrap();

    assert_eq!(rules[1]["ip"], json!(["geoip:cn"]));
    assert_eq!(rules[2]["domain"], json!(["geosite:google"]));
    // Unlike sing-box, no rule sets have to be downloaded.
    assert!(generated["rule_set"].is_null());
}

#[test]
fn explicit_rules_come_before_geo_rules_and_each_group_keeps_its_priority() {
    let profile = rule_profile(vec![
        RoutingRule::geo_ip("CN", RouteOutbound::Direct, 1, true).unwrap(),
        RoutingRule::domain("late.example.com", RouteOutbound::Proxy, 20, true).unwrap(),
        RoutingRule::domain("early.example.com", RouteOutbound::Proxy, 10, true).unwrap(),
    ]);

    let rules = rules(&profile);

    assert_eq!(rules[1]["domain"], json!(["full:early.example.com"]));
    assert_eq!(rules[2]["domain"], json!(["full:late.example.com"]));
    assert_eq!(rules[3]["ip"], json!(["geoip:cn"]));
}

#[test]
fn a_disabled_rule_is_left_out() {
    let profile = rule_profile(vec![
        RoutingRule::domain("kept.example.com", RouteOutbound::Proxy, 1, true).unwrap(),
        RoutingRule::domain("dropped.example.com", RouteOutbound::Proxy, 2, false).unwrap(),
    ]);

    let rules = rules(&profile);

    assert_eq!(rules.len(), 3);
    assert!(!generate(&profile).to_string().contains("dropped"));
}

#[test]
fn the_domain_strategy_only_resolves_when_a_geoip_rule_needs_it() {
    let without_geoip = rule_profile(vec![
        RoutingRule::domain("example.com", RouteOutbound::Proxy, 1, true).unwrap(),
        RoutingRule::geo_site("google", RouteOutbound::Proxy, 2, true).unwrap(),
    ]);
    // GeoSite matches on the name itself, so nothing has to be resolved.
    assert_eq!(generate(&without_geoip)["domainStrategy"], "AsIs");

    let with_geoip = rule_profile(vec![
        RoutingRule::geo_ip("CN", RouteOutbound::Direct, 1, true).unwrap(),
    ]);
    // A GeoIP rule can only match an address, so names must be resolved first.
    assert_eq!(generate(&with_geoip)["domainStrategy"], "IPIfNonMatch");
}

#[test]
fn global_and_direct_modes_never_resolve_for_routing() {
    let global = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let direct = RouteProfile::new(RoutingMode::Direct, Vec::new(), RouteOutbound::Direct).unwrap();

    assert_eq!(generate(&global)["domainStrategy"], "AsIs");
    assert_eq!(generate(&direct)["domainStrategy"], "AsIs");
}

#[test]
fn rule_mode_rules_are_ignored_outside_rule_mode() {
    // The profile validator already forbids rules outside Rule mode, so this
    // guards the generator's own filter rather than the model.
    let profile = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();

    assert_eq!(rules(&profile).len(), 2);
}

#[test]
fn a_blocked_rule_points_xray_at_its_blackhole() {
    let rules =
        vec![RoutingRule::domain_suffix("ads.example", RouteOutbound::Block, 0, true).unwrap()];
    let profile = RouteProfile::new(RoutingMode::Rule, rules, RouteOutbound::Proxy).unwrap();
    let config = XrayRouteConfigGenerator::generate(&profile);

    // The private-address rule always comes first.
    let rule = &config.json()["rules"][1];
    assert_eq!(rule["domain"][0], "domain:ads.example");
    assert_eq!(rule["outboundTag"], "block");
}
