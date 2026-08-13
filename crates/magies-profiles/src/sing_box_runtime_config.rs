use std::num::NonZeroU16;

use magies_domain::{CoreType, ProxyNode};

use crate::NodeGroupStrategy;
use magies_routing::{RouteProfile, SingBoxRouteConfigGenerator};
use serde_json::{Value, json};

use crate::{
    DnsProfile, GeneratedCoreConfig, LocalHttpConfigGenerator, LocalHttpProfile,
    LocalSocksConfigGenerator, LocalSocksProfile, NodeCredential, OutboundConfigError,
    SingBoxDnsConfigGenerator, SingBoxOutboundConfigGenerator, SingBoxTunConfigGenerator,
    TunProfile, apply_sing_box_fragment, apply_sing_box_multiplex, node_outbound_tag,
};

const DEFAULT_URLTEST_PROBE: &str = "https://www.gstatic.com/generate_204";
const URLTEST_INTERVAL: &str = "3m";
const URLTEST_TOLERANCE_MS: u32 = 50;

#[expect(
    clippy::struct_excessive_bools,
    reason = "independent Core toggles the UI exposes one by one"
)]
pub struct SingBoxRuntimeProfile<'a> {
    selected: Option<SelectedNode<'a>>,
    group_outbound: Option<GroupOutbound<'a>>,
    dns: &'a DnsProfile,
    route: &'a RouteProfile,
    socks: LocalSocksProfile,
    http: LocalHttpProfile,
    clash_api_port: Option<NonZeroU16>,
    tun: Option<&'a TunProfile>,
    dns_hijack: bool,
    mux_enabled: bool,
    fragment_enabled: bool,
    final_fragment_enabled: bool,
}

struct GroupOutbound<'a> {
    strategy: NodeGroupStrategy,
    members: Vec<SelectedNode<'a>>,
    probe_url: &'a str,
}

#[derive(Clone, Copy)]
struct SelectedNode<'a> {
    node: &'a ProxyNode,
    credential: NodeCredential<'a>,
}

impl<'a> SingBoxRuntimeProfile<'a> {
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
            clash_api_port: None,
            tun: None,
            dns_hijack: false,
            mux_enabled: false,
            fragment_enabled: false,
            final_fragment_enabled: false,
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
            clash_api_port: None,
            tun: None,
            dns_hijack: false,
            mux_enabled: false,
            fragment_enabled: false,
            final_fragment_enabled: false,
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
    ) -> Result<Self, RuntimeConfigError> {
        if socks.port() == http.port() {
            return Err(RuntimeConfigError::DuplicateLocalPort {
                port: socks.port().get(),
            });
        }
        if let Some(port) = self
            .clash_api_port
            .filter(|port| *port == socks.port() || *port == http.port())
        {
            return Err(RuntimeConfigError::DuplicateLocalPort { port: port.get() });
        }
        self.socks = socks;
        self.http = http;
        Ok(self)
    }

    /// Enables the loopback-only Clash API used for live traffic samples.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the API would reuse a local proxy port.
    pub fn with_clash_api_port(mut self, port: NonZeroU16) -> Result<Self, RuntimeConfigError> {
        if port == self.socks.port() || port == self.http.port() {
            return Err(RuntimeConfigError::DuplicateLocalPort { port: port.get() });
        }
        self.clash_api_port = Some(port);
        Ok(self)
    }

    #[must_use]
    pub const fn with_tun(mut self, tun: &'a TunProfile, dns_hijack: bool) -> Self {
        self.tun = Some(tun);
        self.dns_hijack = dns_hijack;
        self
    }

    /// Turns on outbound multiplex for the next generated config.
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

    /// Turns on TLS record fragmentation at the landing stage (v2rayN's Final
    /// tail fragmentation) via a `route-options` rule.
    #[must_use]
    pub const fn with_final_fragment(mut self, enabled: bool) -> Self {
        self.final_fragment_enabled = enabled;
        self
    }

    /// Emits a multi-node group outbound tagged `proxy` when `members` has at
    /// least two nodes. Fewer members fall back to the selected single outbound.
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

    /// Emits a URL-TEST group tagged `proxy` when `members` has at least two
    /// nodes. Fewer members fall back to the selected single outbound.
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
pub struct SingBoxRuntimeConfigGenerator;

impl SingBoxRuntimeConfigGenerator {
    /// Generates a complete sing-box runtime configuration.
    ///
    /// # Errors
    ///
    /// Returns a typed error when a proxy route has no selected node or the
    /// selected node cannot be represented by sing-box.
    pub fn generate(
        profile: &SingBoxRuntimeProfile<'_>,
    ) -> Result<GeneratedCoreConfig, RuntimeConfigError> {
        let mut route = SingBoxRouteConfigGenerator::generate(profile.route)
            .json()
            .clone();
        let requires_proxy = route_requires_proxy(&route);
        let mut outbounds = Vec::new();
        if requires_proxy {
            outbounds.extend(proxy_outbounds(profile)?);
        }
        outbounds.push(json!({ "type": "direct", "tag": "direct" }));

        let mut inbounds = vec![
            local_socks_inbound(profile.socks),
            local_http_inbound(profile.http),
        ];
        if let Some(tun) = profile.tun {
            let generated = SingBoxTunConfigGenerator::generate(tun);
            inbounds.push(generated.json()["inbounds"][0].clone());
            prepend_tun_actions(&mut route, profile.dns_hijack);
        }
        if profile.final_fragment_enabled {
            prepend_final_fragment_rule(&mut route);
        }
        route["auto_detect_interface"] = Value::Bool(true);
        route["default_domain_resolver"] = Value::String(profile.dns.final_server().to_owned());

        let mut config = json!({
            "log": { "level": "warn" },
            "dns": SingBoxDnsConfigGenerator::generate(profile.dns).json(),
            "inbounds": inbounds,
            "outbounds": outbounds,
            "route": route
        });
        if let Some(port) = profile.clash_api_port {
            config["experimental"] = json!({
                "clash_api": {
                    "external_controller": format!("127.0.0.1:{}", port.get())
                }
            });
        }

        Ok(GeneratedCoreConfig::from_json(CoreType::SingBox, config))
    }
}

fn proxy_outbounds(profile: &SingBoxRuntimeProfile<'_>) -> Result<Vec<Value>, RuntimeConfigError> {
    if let Some(group) = profile
        .group_outbound
        .as_ref()
        .filter(|group| group.members.len() >= 2)
    {
        let mut outbounds = Vec::with_capacity(group.members.len() + 1);
        let mut tags = Vec::with_capacity(group.members.len());
        for member in &group.members {
            let tag = node_outbound_tag(member.node);
            tags.push(tag.clone());
            outbounds.push(member_outbound(profile, member, &tag)?);
        }
        let probe = if group.probe_url.trim().is_empty() {
            DEFAULT_URLTEST_PROBE
        } else {
            group.probe_url
        };
        let group_outbound = match group.strategy {
            NodeGroupStrategy::UrlTest => json!({
                "type": "urltest",
                "tag": "proxy",
                "outbounds": tags,
                "url": probe,
                "interval": URLTEST_INTERVAL,
                "tolerance": URLTEST_TOLERANCE_MS,
            }),
            NodeGroupStrategy::Fallback => json!({
                "type": "fallback",
                "tag": "proxy",
                "outbounds": tags,
                "url": probe,
                "interval": URLTEST_INTERVAL,
            }),
            NodeGroupStrategy::LoadBalance => json!({
                "type": "loadbalance",
                "tag": "proxy",
                "outbounds": tags,
                "strategy": "round-robin",
            }),
            NodeGroupStrategy::Select => unreachable!("select groups do not emit a group outbound"),
        };
        outbounds.push(group_outbound);
        return Ok(outbounds);
    }

    let selected = profile
        .selected
        .ok_or(RuntimeConfigError::MissingSelectedNode)?;
    Ok(vec![member_outbound(profile, &selected, "proxy")?])
}

fn member_outbound(
    profile: &SingBoxRuntimeProfile<'_>,
    member: &SelectedNode<'_>,
    tag: &str,
) -> Result<Value, RuntimeConfigError> {
    let mut outbound = SingBoxOutboundConfigGenerator::generate(member.node, member.credential)?
        .json()
        .clone();
    outbound["tag"] = Value::String(tag.to_owned());
    if profile.mux_enabled {
        apply_sing_box_multiplex(&mut outbound, member.node.protocol_type);
    }
    if profile.fragment_enabled {
        apply_sing_box_fragment(&mut outbound);
    }
    Ok(outbound)
}

fn local_socks_inbound(profile: LocalSocksProfile) -> Value {
    LocalSocksConfigGenerator::generate(CoreType::SingBox, &profile).json()["inbounds"][0].clone()
}

fn local_http_inbound(profile: LocalHttpProfile) -> Value {
    LocalHttpConfigGenerator::generate(CoreType::SingBox, &profile).json()["inbounds"][0].clone()
}

fn route_requires_proxy(route: &Value) -> bool {
    route["final"] == "proxy"
        || route["rules"].as_array().is_some_and(|rules| {
            rules
                .iter()
                .any(|rule| rule["outbound"].as_str() == Some("proxy"))
        })
}

fn prepend_tun_actions(route: &mut Value, dns_hijack: bool) {
    let rules = route["rules"]
        .as_array_mut()
        .expect("route generator always emits an array of rules");
    let mut combined = vec![json!({ "action": "sniff" })];
    if dns_hijack {
        combined.push(json!({ "protocol": "dns", "action": "hijack-dns" }));
    }
    combined.append(rules);
    *rules = combined;
}

fn prepend_final_fragment_rule(route: &mut Value) {
    let rules = route["rules"]
        .as_array_mut()
        .expect("route generator always emits an array of rules");
    rules.insert(
        0,
        json!({
            "action": "route-options",
            "tls_record_fragment": true,
        }),
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeConfigError {
    #[error("local SOCKS and HTTP listeners cannot share port {port}")]
    DuplicateLocalPort { port: u16 },
    #[error("routing rules require a selected proxy node")]
    MissingSelectedNode,
    #[error(transparent)]
    Outbound(#[from] OutboundConfigError),
}
