//! Assembles one complete Xray runtime configuration.
//!
//! The counterpart to [`crate::SingBoxRuntimeConfigGenerator`]. It differs
//! where Xray does:
//!
//! - the direct outbound is `freedom`, not a `direct` type
//! - there is no TUN inbound; the capability matrix already keeps TUN sessions
//!   away from Xray, and this generator refuses one as a second line of defence
//! - fake IP needs a top-level `fakedns` object plus inbound sniffing, not just
//!   a DNS server entry

use std::num::NonZeroU16;

use magies_domain::{CoreType, ProxyNode};
use magies_routing::{RouteProfile, XrayRouteConfigGenerator};
use serde_json::{Value, json};

use crate::{
    DnsProfile, GeneratedCoreConfig, LocalHttpConfigGenerator, LocalHttpProfile,
    LocalSocksConfigGenerator, LocalSocksProfile, NodeCredential, XrayDnsConfigGenerator,
    XrayOutboundConfigGenerator, XrayOutboundError, apply_xray_fragment, apply_xray_mux,
    xray_fragment_outbound,
};

/// The pool Xray hands out fake addresses from, matching sing-box's range so a
/// user switching Cores sees the same addresses.
const FAKE_DNS_V4_POOL: &str = "198.18.0.0/15";
const FAKE_DNS_POOL_SIZE: u32 = 65_535;

pub struct XrayRuntimeProfile<'a> {
    selected: Option<SelectedNode<'a>>,
    dns: &'a DnsProfile,
    route: &'a RouteProfile,
    socks: LocalSocksProfile,
    http: LocalHttpProfile,
    api_port: Option<NonZeroU16>,
    mux_enabled: bool,
    fragment_enabled: bool,
}

#[derive(Clone, Copy)]
struct SelectedNode<'a> {
    node: &'a ProxyNode,
    credential: NodeCredential<'a>,
}

impl<'a> XrayRuntimeProfile<'a> {
    #[must_use]
    pub fn new(
        node: &'a ProxyNode,
        credential: NodeCredential<'a>,
        dns: &'a DnsProfile,
        route: &'a RouteProfile,
    ) -> Self {
        Self {
            selected: Some(SelectedNode { node, credential }),
            dns,
            route,
            socks: LocalSocksProfile::default(),
            http: LocalHttpProfile::default(),
            api_port: None,
            mux_enabled: false,
            fragment_enabled: false,
        }
    }

    #[must_use]
    pub fn without_selected_node(dns: &'a DnsProfile, route: &'a RouteProfile) -> Self {
        Self {
            selected: None,
            dns,
            route,
            socks: LocalSocksProfile::default(),
            http: LocalHttpProfile::default(),
            api_port: None,
            mux_enabled: false,
            fragment_enabled: false,
        }
    }

    /// Replaces the default loopback listeners.
    ///
    /// # Errors
    ///
    /// Returns a typed error when SOCKS and HTTP use the same port.
    pub fn with_local_proxies(
        mut self,
        socks: LocalSocksProfile,
        http: LocalHttpProfile,
    ) -> Result<Self, XrayRuntimeConfigError> {
        if socks.port() == http.port() {
            return Err(XrayRuntimeConfigError::DuplicateLocalPort {
                port: socks.port().get(),
            });
        }
        self.socks = socks;
        self.http = http;
        Ok(self)
    }

    /// Exposes Xray's stats API on a loopback port.
    #[must_use]
    pub const fn with_api_port(mut self, port: NonZeroU16) -> Self {
        self.api_port = Some(port);
        self
    }

    /// Turns on outbound mux for the next generated config.
    #[must_use]
    pub const fn with_mux(mut self, enabled: bool) -> Self {
        self.mux_enabled = enabled;
        self
    }

    /// Turns on TLS `ClientHello` fragmentation (v2rayN's Fragment toggle) for
    /// the next generated config.
    #[must_use]
    pub const fn with_fragment(mut self, enabled: bool) -> Self {
        self.fragment_enabled = enabled;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XrayRuntimeConfigGenerator;

impl XrayRuntimeConfigGenerator {
    /// Generates a complete Xray runtime configuration.
    ///
    /// # Errors
    ///
    /// Returns a typed error when a proxy route has no selected node or the
    /// selected node cannot be represented by Xray.
    pub fn generate(
        profile: &XrayRuntimeProfile<'_>,
    ) -> Result<GeneratedCoreConfig, XrayRuntimeConfigError> {
        let routing = XrayRouteConfigGenerator::generate(profile.route)
            .json()
            .clone();
        let mut outbounds = Vec::new();
        if route_requires_proxy(&routing) {
            let selected = profile
                .selected
                .ok_or(XrayRuntimeConfigError::MissingSelectedNode)?;
            outbounds.push({
                let mut outbound =
                    XrayOutboundConfigGenerator::generate(selected.node, selected.credential)?
                        .json()
                        .clone();
                if profile.mux_enabled {
                    apply_xray_mux(&mut outbound, selected.credential);
                }
                if profile.fragment_enabled {
                    apply_xray_fragment(&mut outbound);
                }
                outbound
            });
            if profile.fragment_enabled {
                outbounds.push(xray_fragment_outbound());
            }
        }
        outbounds.push(json!({ "protocol": "freedom", "tag": "direct" }));

        let fake_dns = profile.dns.fake_ip_enabled();
        let inbounds = vec![
            sniffing(local_socks_inbound(profile.socks), fake_dns),
            sniffing(local_http_inbound(profile.http), fake_dns),
        ];

        let mut config = json!({
            "log": { "loglevel": "warning" },
            "dns": XrayDnsConfigGenerator::generate(profile.dns).json(),
            "inbounds": inbounds,
            "outbounds": outbounds,
            "routing": routing,
        });
        if fake_dns {
            // Without this object the `fakedns` DNS server has no pool to draw
            // from, so the sniffing above would never resolve anything.
            config["fakedns"] = json!({
                "ipPool": FAKE_DNS_V4_POOL,
                "poolSize": FAKE_DNS_POOL_SIZE,
            });
        }
        if let Some(port) = profile.api_port {
            config["stats"] = json!({});
            config["api"] = json!({
                "tag": "api",
                "services": ["StatsService"],
            });
            config["policy"] = json!({
                "system": { "statsOutboundUplink": true, "statsOutboundDownlink": true }
            });
            add_api_inbound(&mut config, port);
        }

        Ok(GeneratedCoreConfig::from_json(CoreType::Xray, config))
    }
}

/// Turns on the sniffing an inbound needs, adding fake-DNS resolution when the
/// profile asked for it.
fn sniffing(mut inbound: Value, fake_dns: bool) -> Value {
    let mut overrides = vec!["http", "tls"];
    if fake_dns {
        overrides.push("fakedns");
    }
    inbound["sniffing"] = json!({
        "enabled": true,
        "destOverride": overrides,
    });
    inbound
}

/// The stats API is reachable through a dedicated loopback inbound, which is
/// also why it needs its own routing rule.
fn add_api_inbound(config: &mut Value, port: NonZeroU16) {
    let inbounds = config["inbounds"]
        .as_array_mut()
        .expect("the generator always builds an array of inbounds");
    inbounds.push(json!({
        "tag": "api-in",
        "listen": "127.0.0.1",
        "port": port.get(),
        "protocol": "dokodemo-door",
        "settings": { "address": "127.0.0.1" },
    }));

    let rules = config["routing"]["rules"]
        .as_array_mut()
        .expect("the route generator always emits an array of rules");
    // Ahead of everything else: the catch-all would otherwise swallow it.
    rules.insert(
        0,
        json!({
            "type": "field",
            "inboundTag": ["api-in"],
            "outboundTag": "api",
        }),
    );
}

fn local_socks_inbound(profile: LocalSocksProfile) -> Value {
    LocalSocksConfigGenerator::generate(CoreType::Xray, &profile).json()["inbounds"][0].clone()
}

fn local_http_inbound(profile: LocalHttpProfile) -> Value {
    LocalHttpConfigGenerator::generate(CoreType::Xray, &profile).json()["inbounds"][0].clone()
}

/// Whether any rule sends traffic to the proxy, which decides if a node is
/// needed at all.
fn route_requires_proxy(routing: &Value) -> bool {
    routing["rules"].as_array().is_some_and(|rules| {
        rules
            .iter()
            .any(|rule| rule["outboundTag"].as_str() == Some("proxy"))
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum XrayRuntimeConfigError {
    #[error("local SOCKS and HTTP listeners cannot share port {port}")]
    DuplicateLocalPort { port: u16 },
    #[error("routing rules require a selected proxy node")]
    MissingSelectedNode,
    #[error(transparent)]
    Outbound(#[from] XrayOutboundError),
}
