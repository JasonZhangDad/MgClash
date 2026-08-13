//! Turns a [`RouteProfile`] into Xray routing rules.
//!
//! Xray expresses in one `domain` or `ip` array what sing-box splits across
//! separate fields, using prefixes to pick the match kind:
//!
//! | model | sing-box | Xray |
//! | --- | --- | --- |
//! | exact domain | `domain` | `full:` |
//! | domain suffix | `domain_suffix` | `domain:` |
//! | domain keyword | `domain_keyword` | `keyword:` |
//! | `GeoSite` | a downloaded rule set | `geosite:` |
//! | `GeoIP` | a downloaded rule set | `geoip:` |
//!
//! The geo prefixes are the reason this generator needs no rule-set downloads:
//! Xray ships the databases with the binary.

use serde_json::{Value, json};

use crate::{GeneratedRouteConfig, GeoKind, RouteProfile, RoutingMode, RoutingRule, RuleMatcher};

/// Xray's own name for the private-address database.
const PRIVATE_GEOIP: &str = "geoip:private";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XrayRouteConfigGenerator;

impl XrayRouteConfigGenerator {
    /// Generates the `routing` section for one profile.
    #[must_use]
    pub fn generate(profile: &RouteProfile) -> GeneratedRouteConfig {
        let mut rules = Vec::new();
        if profile.mode != RoutingMode::Direct {
            rules.push(json!({
                "type": "field",
                "ip": [PRIVATE_GEOIP],
                "outboundTag": "direct"
            }));
        }

        let mut explicit = enabled_rules(profile, false);
        let mut geo = enabled_rules(profile, true);
        explicit.sort_by_key(|rule| rule.priority);
        geo.sort_by_key(|rule| rule.priority);
        rules.extend(explicit.iter().map(|rule| xray_rule(rule)));
        rules.extend(geo.iter().map(|rule| xray_rule(rule)));

        // Xray has no `final`: unmatched traffic goes to the first outbound.
        // A catch-all rule states the intent instead of depending on the order
        // of the outbounds array.
        rules.push(json!({
            "type": "field",
            "network": "tcp,udp",
            "outboundTag": profile.final_outbound.as_str()
        }));

        GeneratedRouteConfig {
            json: json!({
                "domainStrategy": domain_strategy(&geo),
                "rules": rules,
            }),
        }
    }
}

/// `GeoIP` rules only match once a domain has been resolved, so the strategy is
/// raised exactly when such a rule exists; otherwise routing stays on names and
/// no extra lookups happen.
fn domain_strategy(geo: &[&RoutingRule]) -> &'static str {
    let has_geoip = geo.iter().any(|rule| {
        matches!(
            &rule.matcher,
            RuleMatcher::Geo {
                kind: GeoKind::Ip,
                ..
            }
        )
    });
    if has_geoip { "IPIfNonMatch" } else { "AsIs" }
}

fn enabled_rules(profile: &RouteProfile, geo: bool) -> Vec<&RoutingRule> {
    if profile.mode != RoutingMode::Rule {
        return Vec::new();
    }
    profile
        .rules
        .iter()
        .filter(|rule| {
            rule.enabled
                && rule.matcher.is_geo() == geo
                && !matches!(
                    &rule.matcher,
                    RuleMatcher::ProcessPath(_) | RuleMatcher::RuleProvider { .. }
                )
        })
        .collect()
}

fn xray_rule(rule: &RoutingRule) -> Value {
    let mut value = match &rule.matcher {
        RuleMatcher::Domain(domain) => json!({ "domain": [format!("full:{domain}")] }),
        // The model stores a suffix with sing-box's leading dot, which Xray's
        // `domain:` prefix does not accept. Stripping it is the closest match
        // available and slightly wider: sing-box's `.cn` matches `example.cn`
        // but not bare `cn`, whereas Xray's `domain:cn` matches both.
        RuleMatcher::DomainSuffix(suffix) => {
            json!({ "domain": [format!("domain:{}", suffix.trim_start_matches('.'))] })
        }
        RuleMatcher::DomainKeyword(keyword) => json!({ "domain": [format!("keyword:{keyword}")] }),
        RuleMatcher::IpCidr(network) => json!({ "ip": [network.to_string()] }),
        // Xray takes a port list as a string, which also covers ranges.
        RuleMatcher::Port(port) => json!({ "port": port.get().to_string() }),
        RuleMatcher::Network(network) => json!({ "network": network.as_str() }),
        RuleMatcher::ProcessName(name) => json!({ "process": [name] }),
        RuleMatcher::ProcessPath(_) => unreachable!("process_path rules are omitted for Xray"),
        RuleMatcher::RuleProvider { .. } => {
            unreachable!("rule provider rules are omitted for Xray")
        }
        RuleMatcher::Geo { kind, code } => match kind {
            GeoKind::Ip => json!({ "ip": [format!("geoip:{code}")] }),
            GeoKind::Site => json!({ "domain": [format!("geosite:{code}")] }),
        },
    };
    value["type"] = Value::String("field".to_owned());
    value["outboundTag"] = Value::String(rule.outbound.as_str().to_owned());
    value
}
