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

use std::collections::HashMap;
use std::num::NonZeroU16;

use magies_domain::{CoreType, ProxyNode};
use magies_routing::{RouteProfile, XrayRouteConfigGenerator};
use serde_json::{Value, json};

use crate::NodeGroupStrategy;
use crate::xray_outbound::{
    XrayOutboundConfigGenerator, XrayOutboundError, apply_xray_final_fragment, apply_xray_fragment,
    apply_xray_mux, normalize_xray_finalmask_tcp, xray_fragment_outbound_with_options,
};
use crate::{
    DnsProfile, GeneratedCoreConfig, LocalHttpConfigGenerator, LocalHttpProfile,
    LocalSocksConfigGenerator, LocalSocksProfile, NodeCredential, XrayDnsConfigGenerator,
    node_outbound_tag,
};

/// The pool Xray hands out fake addresses from, matching sing-box's range so a
/// user switching Cores sees the same addresses.
const FAKE_DNS_V4_POOL: &str = "198.18.0.0/15";
const FAKE_DNS_POOL_SIZE: u32 = 65_535;
const DEFAULT_URLTEST_PROBE: &str = "https://www.gstatic.com/generate_204";
const URLTEST_INTERVAL: &str = "3m";

#[expect(
    clippy::struct_excessive_bools,
    reason = "independent Core toggles the UI exposes one by one"
)]
pub struct XrayRuntimeProfile<'a> {
    selected: Option<SelectedNode<'a>>,
    group_outbound: Option<GroupOutbound<'a>>,
    dns: &'a DnsProfile,
    route: &'a RouteProfile,
    socks: LocalSocksProfile,
    http: LocalHttpProfile,
    api_port: Option<NonZeroU16>,
    mux_enabled: bool,
    fragment_enabled: bool,
    final_fragment_enabled: bool,
    udp_noise_enabled: bool,
}

#[derive(Clone, Copy)]
struct SelectedNode<'a> {
    node: &'a ProxyNode,
    credential: NodeCredential<'a>,
}

struct GroupOutbound<'a> {
    strategy: NodeGroupStrategy,
    members: Vec<SelectedNode<'a>>,
    probe_url: &'a str,
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
            group_outbound: None,
            dns,
            route,
            socks: LocalSocksProfile::default(),
            http: LocalHttpProfile::default(),
            api_port: None,
            mux_enabled: false,
            fragment_enabled: false,
            final_fragment_enabled: false,
            udp_noise_enabled: false,
        }
    }

    #[must_use]
    pub fn without_selected_node(dns: &'a DnsProfile, route: &'a RouteProfile) -> Self {
        Self {
            selected: None,
            group_outbound: None,
            dns,
            route,
            socks: LocalSocksProfile::default(),
            http: LocalHttpProfile::default(),
            api_port: None,
            mux_enabled: false,
            fragment_enabled: false,
            final_fragment_enabled: false,
            udp_noise_enabled: false,
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

    /// Wraps proxy outbounds with a freedom detour that applies `finalmask`
    /// TCP fragmentation (v2rayN's Final tail fragmentation).
    #[must_use]
    pub const fn with_final_fragment(mut self, enabled: bool) -> Self {
        self.final_fragment_enabled = enabled;
        self
    }

    /// Turns on Xray UDP noise (v2rayN-style freedom `noises`) for the next
    /// generated config.
    #[must_use]
    pub const fn with_udp_noise(mut self, enabled: bool) -> Self {
        self.udp_noise_enabled = enabled;
        self
    }

    /// Emits a multi-node balancer tagged `proxy` when `members` has at least
    /// two nodes.
    #[must_use]
    pub fn with_group_outbound(
        mut self,
        strategy: NodeGroupStrategy,
        members: Vec<(&'a ProxyNode, NodeCredential<'a>)>,
        probe_url: &'a str,
    ) -> Self {
        self.group_outbound = Some(GroupOutbound {
            strategy,
            members: members
                .into_iter()
                .map(|(node, credential)| SelectedNode { node, credential })
                .collect(),
            probe_url,
        });
        self
    }

    /// Emits an observatory + leastPing balancer tagged `proxy` when `members`
    /// has at least two nodes.
    #[must_use]
    pub fn with_urltest(
        self,
        members: Vec<(&'a ProxyNode, NodeCredential<'a>)>,
        probe_url: &'a str,
    ) -> Self {
        self.with_group_outbound(NodeGroupStrategy::UrlTest, members, probe_url)
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
        let mut routing = XrayRouteConfigGenerator::generate(profile.route)
            .json()
            .clone();
        let mut outbounds = Vec::new();
        let group_outbound = profile
            .group_outbound
            .as_ref()
            .filter(|group| group.members.len() >= 2);
        let mut used_group = None;
        if route_requires_proxy(&routing) {
            if let Some(group) = group_outbound {
                let mut tags = Vec::with_capacity(group.members.len());
                for member in &group.members {
                    let tag = node_outbound_tag(member.node);
                    tags.push(tag.clone());
                    outbounds.push(xray_member_outbound(profile, member, &tag)?);
                }
                apply_proxy_balancer(&mut routing, &tags, group.strategy);
                used_group = Some(group);
            } else {
                let selected = profile
                    .selected
                    .ok_or(XrayRuntimeConfigError::MissingSelectedNode)?;
                outbounds.push(xray_member_outbound(profile, &selected, "proxy")?);
            }
            if profile.fragment_enabled || profile.udp_noise_enabled {
                outbounds.push(xray_fragment_outbound_with_options(
                    profile.fragment_enabled,
                    profile.udp_noise_enabled,
                ));
            }
            if profile.final_fragment_enabled {
                let overrides = finalmask_overrides(profile)?;
                apply_xray_final_fragment(&mut outbounds, &overrides);
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
        if let Some(group) = used_group
            && group.strategy.uses_observatory()
        {
            let probe = if group.probe_url.trim().is_empty() {
                DEFAULT_URLTEST_PROBE
            } else {
                group.probe_url
            };
            config["observatory"] = json!({
                "subjectSelector": ["node-"],
                "probeUrl": probe,
                "probeInterval": URLTEST_INTERVAL,
            });
        }

        Ok(GeneratedCoreConfig::from_json(CoreType::Xray, config))
    }
}

fn xray_member_outbound(
    profile: &XrayRuntimeProfile<'_>,
    member: &SelectedNode<'_>,
    tag: &str,
) -> Result<Value, XrayRuntimeConfigError> {
    let mut outbound = XrayOutboundConfigGenerator::generate(member.node, member.credential)?
        .json()
        .clone();
    outbound["tag"] = Value::String(tag.to_owned());
    if profile.mux_enabled {
        apply_xray_mux(&mut outbound, member.credential);
    }
    if profile.fragment_enabled || profile.udp_noise_enabled {
        apply_xray_fragment(&mut outbound);
    }
    Ok(outbound)
}

fn apply_proxy_balancer(routing: &mut Value, member_tags: &[String], strategy: NodeGroupStrategy) {
    if let Some(rules) = routing["rules"].as_array_mut() {
        for rule in rules {
            if rule.get("outboundTag").and_then(Value::as_str) == Some("proxy") {
                if let Some(object) = rule.as_object_mut() {
                    object.remove("outboundTag");
                    object.insert("balancerTag".to_owned(), json!("proxy"));
                }
            }
        }
    }
    let strategy_type = match strategy {
        NodeGroupStrategy::UrlTest => "leastPing",
        NodeGroupStrategy::Fallback => "roundRobin",
        NodeGroupStrategy::LoadBalance => "random",
        NodeGroupStrategy::Select => unreachable!("select groups do not emit a group outbound"),
    };
    let mut balancer = json!({
        "tag": "proxy",
        "selector": member_tags,
        "strategy": { "type": strategy_type }
    });
    if strategy == NodeGroupStrategy::Fallback
        && let Some(first) = member_tags.first()
    {
        balancer["fallbackTag"] = json!(first);
    }
    routing["balancers"] = json!([balancer]);
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
fn finalmask_overrides(
    profile: &XrayRuntimeProfile<'_>,
) -> Result<HashMap<String, Value>, XrayRuntimeConfigError> {
    let mut overrides = HashMap::new();
    if let Some(group) = profile
        .group_outbound
        .as_ref()
        .filter(|group| group.members.len() >= 2)
    {
        for member in &group.members {
            if let Some(json) = member.node.xray_finalmask_json.as_ref() {
                let value: Value = serde_json::from_str(json).map_err(|_| {
                    XrayRuntimeConfigError::Finalmask(
                        crate::xray_outbound::XrayFinalmaskError::InvalidShape,
                    )
                })?;
                overrides.insert(
                    node_outbound_tag(member.node),
                    normalize_xray_finalmask_tcp(&value)?,
                );
            }
        }
        return Ok(overrides);
    }
    if let Some(selected) = profile.selected
        && let Some(json) = selected.node.xray_finalmask_json.as_ref()
    {
        let value: Value = serde_json::from_str(json).map_err(|_| {
            XrayRuntimeConfigError::Finalmask(
                crate::xray_outbound::XrayFinalmaskError::InvalidShape,
            )
        })?;
        overrides.insert("proxy".to_owned(), normalize_xray_finalmask_tcp(&value)?);
    }
    Ok(overrides)
}

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
    #[error(transparent)]
    Finalmask(#[from] crate::xray_outbound::XrayFinalmaskError),
}
