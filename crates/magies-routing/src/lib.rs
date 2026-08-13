//! Ordered route models and Core route generation for `MgClash`.

mod xray;

pub use xray::XrayRouteConfigGenerator;

use std::collections::HashSet;
use std::num::NonZeroU16;

use ipnet::IpNet;
use serde_json::{Value, json};
use url::Host;

const GEOIP_RULE_SET_BASE: &str = "https://raw.githubusercontent.com/SagerNet/sing-geoip/rule-set";
const GEOSITE_RULE_SET_BASE: &str =
    "https://raw.githubusercontent.com/SagerNet/sing-geosite/rule-set";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutingMode {
    Global,
    Rule,
    Direct,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteOutbound {
    Proxy,
    Direct,
}

impl RouteOutbound {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Proxy => "proxy",
            Self::Direct => "direct",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Network {
    Tcp,
    Udp,
}

impl Network {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GeoKind {
    Ip,
    Site,
}

impl GeoKind {
    const fn prefix(self) -> &'static str {
        match self {
            Self::Ip => "geoip",
            Self::Site => "geosite",
        }
    }

    const fn base_url(self) -> &'static str {
        match self {
            Self::Ip => GEOIP_RULE_SET_BASE,
            Self::Site => GEOSITE_RULE_SET_BASE,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RuleMatcher {
    Domain(String),
    DomainSuffix(String),
    DomainKeyword(String),
    IpCidr(IpNet),
    Port(NonZeroU16),
    Network(Network),
    ProcessName(String),
    ProcessPath(String),
    Geo { kind: GeoKind, code: String },
}

impl RuleMatcher {
    const fn is_geo(&self) -> bool {
        matches!(self, Self::Geo { .. })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutingRule {
    matcher: RuleMatcher,
    outbound: RouteOutbound,
    priority: i32,
    enabled: bool,
}

impl RoutingRule {
    /// Creates an exact-domain rule.
    ///
    /// # Errors
    ///
    /// Returns a typed error when `value` is not a domain name.
    pub fn domain(
        value: &str,
        outbound: RouteOutbound,
        priority: i32,
        enabled: bool,
    ) -> Result<Self, RouteConfigError> {
        Ok(Self::new(
            RuleMatcher::Domain(validated_domain(value)?),
            outbound,
            priority,
            enabled,
        ))
    }

    /// Creates a domain-suffix rule.
    ///
    /// # Errors
    ///
    /// Returns a typed error when `value` is not a domain suffix.
    pub fn domain_suffix(
        value: &str,
        outbound: RouteOutbound,
        priority: i32,
        enabled: bool,
    ) -> Result<Self, RouteConfigError> {
        let value = value.trim().trim_start_matches('.');
        Ok(Self::new(
            RuleMatcher::DomainSuffix(format!(".{}", validated_domain(value)?)),
            outbound,
            priority,
            enabled,
        ))
    }

    /// Creates a domain-keyword rule.
    ///
    /// # Errors
    ///
    /// Returns a typed error when `value` is empty.
    pub fn domain_keyword(
        value: &str,
        outbound: RouteOutbound,
        priority: i32,
        enabled: bool,
    ) -> Result<Self, RouteConfigError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(RouteConfigError::EmptyValue);
        }
        Ok(Self::new(
            RuleMatcher::DomainKeyword(value.to_owned()),
            outbound,
            priority,
            enabled,
        ))
    }

    /// Creates an IPv4 CIDR rule.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an invalid CIDR or an IPv6 network.
    pub fn ip_cidr(
        value: &str,
        outbound: RouteOutbound,
        priority: i32,
        enabled: bool,
    ) -> Result<Self, RouteConfigError> {
        let network = validated_cidr(value)?;
        if matches!(network, IpNet::V6(_)) {
            return Err(RouteConfigError::ExpectedIpv4 {
                value: value.to_owned(),
            });
        }
        Ok(Self::new(
            RuleMatcher::IpCidr(network),
            outbound,
            priority,
            enabled,
        ))
    }

    /// Creates an IPv6 CIDR rule.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an invalid CIDR or an IPv4 network.
    pub fn ip_cidr6(
        value: &str,
        outbound: RouteOutbound,
        priority: i32,
        enabled: bool,
    ) -> Result<Self, RouteConfigError> {
        let network = validated_cidr(value)?;
        if matches!(network, IpNet::V4(_)) {
            return Err(RouteConfigError::ExpectedIpv6 {
                value: value.to_owned(),
            });
        }
        Ok(Self::new(
            RuleMatcher::IpCidr(network),
            outbound,
            priority,
            enabled,
        ))
    }

    /// Creates a destination-port rule.
    ///
    /// # Errors
    ///
    /// Returns a typed error unless `port` is in `1..=65535`.
    pub fn port(
        port: u32,
        outbound: RouteOutbound,
        priority: i32,
        enabled: bool,
    ) -> Result<Self, RouteConfigError> {
        let port = u16::try_from(port)
            .ok()
            .and_then(NonZeroU16::new)
            .ok_or(RouteConfigError::InvalidPort { port })?;
        Ok(Self::new(
            RuleMatcher::Port(port),
            outbound,
            priority,
            enabled,
        ))
    }

    #[must_use]
    pub const fn network(
        network: Network,
        outbound: RouteOutbound,
        priority: i32,
        enabled: bool,
    ) -> Self {
        Self::new(RuleMatcher::Network(network), outbound, priority, enabled)
    }

    /// Creates a process-name rule (sing-box `process_name`, Xray `process`).
    ///
    /// # Errors
    ///
    /// Returns a typed error when `value` is empty.
    pub fn process_name(
        value: &str,
        outbound: RouteOutbound,
        priority: i32,
        enabled: bool,
    ) -> Result<Self, RouteConfigError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(RouteConfigError::EmptyValue);
        }
        Ok(Self::new(
            RuleMatcher::ProcessName(value.to_owned()),
            outbound,
            priority,
            enabled,
        ))
    }

    /// Creates a process-path rule (sing-box `process_path`).
    ///
    /// # Errors
    ///
    /// Returns a typed error when `value` is empty.
    pub fn process_path(
        value: &str,
        outbound: RouteOutbound,
        priority: i32,
        enabled: bool,
    ) -> Result<Self, RouteConfigError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(RouteConfigError::EmptyValue);
        }
        Ok(Self::new(
            RuleMatcher::ProcessPath(value.to_owned()),
            outbound,
            priority,
            enabled,
        ))
    }

    /// Creates a `GeoIP` rule backed by the official binary rule set.
    ///
    /// # Errors
    ///
    /// Returns a typed error when `code` is not a safe rule-set identifier.
    pub fn geo_ip(
        code: &str,
        outbound: RouteOutbound,
        priority: i32,
        enabled: bool,
    ) -> Result<Self, RouteConfigError> {
        Self::geo(GeoKind::Ip, code, outbound, priority, enabled)
    }

    /// Creates a `GeoSite` rule backed by the official binary rule set.
    ///
    /// # Errors
    ///
    /// Returns a typed error when `code` is not a safe rule-set identifier.
    pub fn geo_site(
        code: &str,
        outbound: RouteOutbound,
        priority: i32,
        enabled: bool,
    ) -> Result<Self, RouteConfigError> {
        Self::geo(GeoKind::Site, code, outbound, priority, enabled)
    }

    fn geo(
        kind: GeoKind,
        code: &str,
        outbound: RouteOutbound,
        priority: i32,
        enabled: bool,
    ) -> Result<Self, RouteConfigError> {
        let code = validated_geo_code(code)?;
        Ok(Self::new(
            RuleMatcher::Geo { kind, code },
            outbound,
            priority,
            enabled,
        ))
    }

    const fn new(
        matcher: RuleMatcher,
        outbound: RouteOutbound,
        priority: i32,
        enabled: bool,
    ) -> Self {
        Self {
            matcher,
            outbound,
            priority,
            enabled,
        }
    }

    fn json(&self) -> Value {
        let mut value = match &self.matcher {
            RuleMatcher::Domain(domain) => json!({ "domain": [domain] }),
            RuleMatcher::DomainSuffix(suffix) => json!({ "domain_suffix": [suffix] }),
            RuleMatcher::DomainKeyword(keyword) => json!({ "domain_keyword": [keyword] }),
            RuleMatcher::IpCidr(network) => json!({ "ip_cidr": [network.to_string()] }),
            RuleMatcher::Port(port) => json!({ "port": [port.get()] }),
            RuleMatcher::Network(network) => json!({ "network": network.as_str() }),
            RuleMatcher::ProcessName(name) => json!({ "process_name": [name] }),
            RuleMatcher::ProcessPath(path) => json!({ "process_path": [path] }),
            RuleMatcher::Geo { kind, code } => {
                json!({ "rule_set": [format!("{}-{code}", kind.prefix())] })
            }
        };
        value["action"] = Value::String("route".to_owned());
        value["outbound"] = Value::String(self.outbound.as_str().to_owned());
        value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteProfile {
    mode: RoutingMode,
    rules: Vec<RoutingRule>,
    final_outbound: RouteOutbound,
}

impl RouteProfile {
    /// Creates a validated routing profile.
    ///
    /// # Errors
    ///
    /// Returns a typed error for rules outside Rule mode or a final outbound
    /// incompatible with Global or Direct mode.
    pub fn new(
        mode: RoutingMode,
        rules: Vec<RoutingRule>,
        final_outbound: RouteOutbound,
    ) -> Result<Self, RouteConfigError> {
        if mode != RoutingMode::Rule && !rules.is_empty() {
            return Err(RouteConfigError::RulesRequireRuleMode);
        }
        if mode == RoutingMode::Global && final_outbound != RouteOutbound::Proxy {
            return Err(RouteConfigError::GlobalModeRequiresProxyFinal);
        }
        if mode == RoutingMode::Direct && final_outbound != RouteOutbound::Direct {
            return Err(RouteConfigError::DirectModeRequiresDirectFinal);
        }
        Ok(Self {
            mode,
            rules,
            final_outbound,
        })
    }

    #[must_use]
    pub const fn mode(&self) -> RoutingMode {
        self.mode
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedRouteConfig {
    json: Value,
}

impl GeneratedRouteConfig {
    #[must_use]
    pub const fn json(&self) -> &Value {
        &self.json
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SingBoxRouteConfigGenerator;

impl SingBoxRouteConfigGenerator {
    #[must_use]
    pub fn generate(profile: &RouteProfile) -> GeneratedRouteConfig {
        let mut rules = Vec::new();
        if profile.mode != RoutingMode::Direct {
            rules.push(json!({
                "ip_is_private": true,
                "action": "route",
                "outbound": "direct"
            }));
        }

        let mut explicit = enabled_rules(profile, false);
        let mut geo = enabled_rules(profile, true);
        explicit.sort_by_key(|rule| rule.priority);
        geo.sort_by_key(|rule| rule.priority);
        rules.extend(explicit.into_iter().map(RoutingRule::json));
        rules.extend(geo.iter().map(|rule| rule.json()));

        let rule_sets = generated_rule_sets(&geo);
        let mut route = json!({
            "rules": rules,
            "final": profile.final_outbound.as_str()
        });
        if !rule_sets.is_empty() {
            route["rule_set"] = Value::Array(rule_sets);
        }
        GeneratedRouteConfig { json: route }
    }
}

fn enabled_rules(profile: &RouteProfile, geo: bool) -> Vec<&RoutingRule> {
    if profile.mode != RoutingMode::Rule {
        return Vec::new();
    }
    profile
        .rules
        .iter()
        .filter(|rule| rule.enabled && rule.matcher.is_geo() == geo)
        .collect()
}

fn generated_rule_sets(rules: &[&RoutingRule]) -> Vec<Value> {
    let mut seen = HashSet::new();
    rules
        .iter()
        .filter_map(|rule| {
            let RuleMatcher::Geo { kind, code } = &rule.matcher else {
                return None;
            };
            let tag = format!("{}-{code}", kind.prefix());
            seen.insert(tag.clone()).then(|| {
                json!({
                    "type": "remote",
                    "tag": tag,
                    "format": "binary",
                    "url": format!("{}/{}-{code}.srs", kind.base_url(), kind.prefix()),
                    "download_detour": "direct"
                })
            })
        })
        .collect()
}

fn validated_domain(value: &str) -> Result<String, RouteConfigError> {
    let value = value.trim();
    match Host::parse(value) {
        Ok(Host::Domain(domain)) => Ok(domain),
        _ => Err(RouteConfigError::InvalidDomain {
            value: value.to_owned(),
        }),
    }
}

fn validated_cidr(value: &str) -> Result<IpNet, RouteConfigError> {
    value.parse().map_err(|_| RouteConfigError::InvalidCidr {
        value: value.to_owned(),
    })
}

fn validated_geo_code(value: &str) -> Result<String, RouteConfigError> {
    let normalized = value.trim().to_ascii_lowercase();
    let valid = !normalized.is_empty()
        && normalized
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if !valid {
        return Err(RouteConfigError::InvalidGeoCode {
            value: value.to_owned(),
        });
    }
    Ok(normalized)
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RouteConfigError {
    #[error("invalid route domain: {value}")]
    InvalidDomain { value: String },
    #[error("route matcher value cannot be empty")]
    EmptyValue,
    #[error("invalid route CIDR: {value}")]
    InvalidCidr { value: String },
    #[error("expected an IPv4 route CIDR: {value}")]
    ExpectedIpv4 { value: String },
    #[error("expected an IPv6 route CIDR: {value}")]
    ExpectedIpv6 { value: String },
    #[error("route port must be between 1 and 65535, got {port}")]
    InvalidPort { port: u32 },
    #[error("invalid GeoIP/GeoSite code: {value}")]
    InvalidGeoCode { value: String },
    #[error("user routing rules require Rule mode")]
    RulesRequireRuleMode,
    #[error("Global mode must use the proxy final outbound")]
    GlobalModeRequiresProxyFinal,
    #[error("Direct mode must use the direct final outbound")]
    DirectModeRequiresDirectFinal,
}
