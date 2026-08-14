use std::collections::HashSet;

use ipnet::IpNet;
use magies_domain::CoreType;
use magies_platform::OperatingSystem;
use serde_json::{Value, json};

use crate::{DnsProfile, GeneratedCoreConfig, SingBoxDnsConfigGenerator};

const IPV4_ADDRESS: &str = "172.19.0.1/30";
const IPV6_ADDRESS: &str = "fdfe:dcba:9876::1/126";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TunProfile {
    platform: OperatingSystem,
    ipv6_enabled: bool,
    mtu: u16,
    auto_route: bool,
    strict_route: bool,
    routes: TunRouteSettings,
    dns: Option<TunDnsSettings>,
}

impl TunProfile {
    /// Creates a validated Windows or Linux TUN profile.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an MTU outside `1280..=9000`, or strict
    /// routing without automatic routes.
    pub fn new(
        platform: OperatingSystem,
        ipv6_enabled: bool,
        mtu: u16,
        auto_route: bool,
        strict_route: bool,
    ) -> Result<Self, TunProfileError> {
        if !(1280..=9000).contains(&mtu) {
            return Err(TunProfileError::InvalidMtu { mtu });
        }
        if strict_route && !auto_route {
            return Err(TunProfileError::StrictRouteRequiresAutoRoute);
        }
        Ok(Self {
            platform,
            ipv6_enabled,
            mtu,
            auto_route,
            strict_route,
            routes: TunRouteSettings::default(),
            dns: None,
        })
    }

    /// Adds validated routes to a profile using automatic routing.
    ///
    /// # Errors
    ///
    /// Returns a typed error when automatic routing is disabled or an IPv6
    /// route is used by an IPv4-only profile.
    pub fn with_routes(mut self, routes: TunRouteSettings) -> Result<Self, TunProfileError> {
        if !routes.is_empty() && !self.auto_route {
            return Err(TunProfileError::RoutesRequireAutoRoute);
        }
        if !self.ipv6_enabled {
            if let Some(route) = routes.all().find(|route| matches!(route, IpNet::V6(_))) {
                return Err(TunProfileError::Ipv6RouteRequiresIpv6 {
                    cidr: route.to_string(),
                });
            }
        }
        self.routes = routes;
        Ok(self)
    }

    #[must_use]
    pub fn with_dns(mut self, profile: DnsProfile, hijack: bool) -> Self {
        self.dns = Some(TunDnsSettings { profile, hijack });
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TunDnsSettings {
    profile: DnsProfile,
    hijack: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TunRouteSettings {
    included: Vec<IpNet>,
    excluded: Vec<IpNet>,
}

impl TunRouteSettings {
    /// Creates route settings from CIDR values.
    ///
    /// # Errors
    ///
    /// Returns a typed error for invalid CIDR values or a route present in
    /// both the include and exclude lists.
    pub fn new(included: Vec<String>, excluded: Vec<String>) -> Result<Self, TunProfileError> {
        let included = parse_routes(included)?;
        let excluded = parse_routes(excluded)?;
        let included_set: HashSet<_> = included.iter().collect();
        if let Some(route) = excluded.iter().find(|route| included_set.contains(route)) {
            return Err(TunProfileError::ConflictingRoute {
                cidr: route.to_string(),
            });
        }
        Ok(Self { included, excluded })
    }

    fn all(&self) -> impl Iterator<Item = &IpNet> {
        self.included.iter().chain(&self.excluded)
    }

    fn is_empty(&self) -> bool {
        self.included.is_empty() && self.excluded.is_empty()
    }
}

fn parse_routes(values: Vec<String>) -> Result<Vec<IpNet>, TunProfileError> {
    values
        .into_iter()
        .map(|value| {
            value
                .parse()
                .map_err(|_| TunProfileError::InvalidRoute { value })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SingBoxTunConfigGenerator;

impl SingBoxTunConfigGenerator {
    #[must_use]
    pub fn generate(profile: &TunProfile) -> GeneratedCoreConfig {
        let addresses = if profile.ipv6_enabled {
            vec![IPV4_ADDRESS, IPV6_ADDRESS]
        } else {
            vec![IPV4_ADDRESS]
        };
        let mut inbound = json!({
            "type": "tun",
            "tag": "tun-in",
            "address": addresses,
            "mtu": profile.mtu,
            "auto_route": profile.auto_route,
            "strict_route": profile.strict_route,
            "stack": "gvisor"
        });
        // macOS only accepts `utunN` and hands out the next free one itself, so
        // naming the device is left to it rather than guessing a number.
        if let Some(name) = match profile.platform {
            OperatingSystem::Windows => Some("MgClash"),
            OperatingSystem::Linux => Some("mgclash0"),
            OperatingSystem::MacOs => None,
        } {
            inbound["interface_name"] = Value::String(name.to_owned());
        }
        if profile.platform == OperatingSystem::Linux && profile.auto_route {
            inbound["auto_redirect"] = Value::Bool(true);
        }
        if !profile.routes.included.is_empty() {
            inbound["route_address"] = json!(
                profile
                    .routes
                    .included
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            );
        }
        if !profile.routes.excluded.is_empty() {
            inbound["route_exclude_address"] = json!(
                profile
                    .routes
                    .excluded
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            );
        }
        let mut route = json!({ "auto_detect_interface": true, "final": "direct" });
        if let Some(dns) = &profile.dns {
            route["default_domain_resolver"] = Value::String(dns.profile.final_server().to_owned());
            if dns.hijack {
                route["rules"] = json!([
                    { "action": "sniff" },
                    { "protocol": "dns", "action": "hijack-dns" }
                ]);
            }
        }
        let mut config = json!({
            "log": { "level": "warn" },
            "inbounds": [inbound],
            "outbounds": [{ "type": "direct", "tag": "direct" }],
            "route": route
        });
        if let Some(dns) = &profile.dns {
            config["dns"] = SingBoxDnsConfigGenerator::generate(&dns.profile)
                .json()
                .clone();
        }
        GeneratedCoreConfig::from_json(CoreType::SingBox, config)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TunProfileError {
    #[error("TUN is unavailable for unsigned {0:?} builds")]
    UnsupportedPlatform(OperatingSystem),
    #[error("TUN MTU must be between 1280 and 9000, got {mtu}")]
    InvalidMtu { mtu: u16 },
    #[error("strict TUN routing requires automatic routes")]
    StrictRouteRequiresAutoRoute,
    #[error("invalid TUN route CIDR: {value}")]
    InvalidRoute { value: String },
    #[error("TUN route is included and excluded: {cidr}")]
    ConflictingRoute { cidr: String },
    #[error("IPv6 TUN route requires IPv6 to be enabled: {cidr}")]
    Ipv6RouteRequiresIpv6 { cidr: String },
    #[error("TUN routes require automatic routing")]
    RoutesRequireAutoRoute,
}
