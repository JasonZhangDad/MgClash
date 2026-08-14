use std::num::NonZeroU16;

use magies_domain::{CoreType, ProxyNode};

use crate::NodeGroupStrategy;
use magies_routing::{RouteProfile, SingBoxRouteConfigGenerator};
use serde_json::{Value, json};
use url::Url;

use crate::apply_config_template;
use crate::{
    DnsProfile, GeneratedCoreConfig, LocalHttpConfigGenerator, LocalHttpProfile,
    LocalSocksConfigGenerator, LocalSocksProfile, NodeCredential, OutboundConfigError,
    SingBoxDnsConfigGenerator, SingBoxOutboundConfigGenerator, SingBoxTunConfigGenerator,
    TunProfile, apply_sing_box_fragment, apply_sing_box_multiplex, node_outbound_tag,
};

const DEFAULT_URLTEST_PROBE: &str = "https://www.gstatic.com/generate_204";
/// The Core polls this often at the fastest; anything shorter is a self-inflicted
/// denial of service against the probe endpoint.
const MIN_PROBE_INTERVAL_SECONDS: u32 = 10;
const MAX_PROBE_INTERVAL_SECONDS: u32 = 24 * 60 * 60;
/// A tolerance above this makes the group ignore every real latency difference.
const MAX_PROBE_TOLERANCE_MS: u32 = 5_000;

/// How a policy group measures its members: what to fetch, how often, and how
/// much better a member has to be before the group switches to it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupProbe {
    url: String,
    interval_seconds: u32,
    tolerance_ms: u32,
}

impl GroupProbe {
    /// Validates one probe configuration.
    ///
    /// An empty URL keeps the built-in probe; everything else has to be an
    /// HTTP(S) URL the Core can fetch.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a non-HTTP URL, an interval outside
    /// 10s..=24h, or a tolerance above five seconds.
    pub fn new(
        url: &str,
        interval_seconds: u32,
        tolerance_ms: u32,
    ) -> Result<Self, GroupProbeError> {
        let trimmed = url.trim();
        let url = if trimmed.is_empty() {
            DEFAULT_URLTEST_PROBE.to_owned()
        } else {
            let parsed = Url::parse(trimmed).map_err(|_| GroupProbeError::InvalidUrl {
                value: trimmed.to_owned(),
            })?;
            if !matches!(parsed.scheme(), "http" | "https") {
                return Err(GroupProbeError::InvalidUrl {
                    value: trimmed.to_owned(),
                });
            }
            trimmed.to_owned()
        };
        if !(MIN_PROBE_INTERVAL_SECONDS..=MAX_PROBE_INTERVAL_SECONDS).contains(&interval_seconds) {
            return Err(GroupProbeError::IntervalOutOfRange {
                seconds: interval_seconds,
            });
        }
        if tolerance_ms > MAX_PROBE_TOLERANCE_MS {
            return Err(GroupProbeError::ToleranceOutOfRange {
                milliseconds: tolerance_ms,
            });
        }
        Ok(Self {
            url,
            interval_seconds,
            tolerance_ms,
        })
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub const fn tolerance_ms(&self) -> u32 {
        self.tolerance_ms
    }

    /// The interval in the duration spelling both Cores accept.
    #[must_use]
    pub fn interval(&self) -> String {
        if self.interval_seconds % 60 == 0 {
            format!("{}m", self.interval_seconds / 60)
        } else {
            format!("{}s", self.interval_seconds)
        }
    }
}

impl Default for GroupProbe {
    fn default() -> Self {
        Self {
            url: DEFAULT_URLTEST_PROBE.to_owned(),
            interval_seconds: 180,
            tolerance_ms: 50,
        }
    }
}

/// Why a probe configuration was refused.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum GroupProbeError {
    #[error("the group probe must be an HTTP(S) URL: {value}")]
    InvalidUrl { value: String },
    #[error("the group probe interval must be 10s..=24h, got {seconds}s")]
    IntervalOutOfRange { seconds: u32 },
    #[error("the group probe tolerance must be at most 5000ms, got {milliseconds}ms")]
    ToleranceOutOfRange { milliseconds: u32 },
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "independent Core toggles the UI exposes one by one"
)]
pub struct SingBoxRuntimeProfile<'a> {
    selected: Option<SelectedNode<'a>>,
    front: Option<SelectedNode<'a>>,
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
    template: Option<&'a Value>,
}

struct GroupOutbound<'a> {
    strategy: NodeGroupStrategy,
    members: Vec<SelectedNode<'a>>,
    probe: &'a GroupProbe,
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
            front: None,
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
            template: None,
        }
    }

    #[must_use]
    pub fn without_selected_node(dns: &'a DnsProfile, route: &'a RouteProfile) -> Self {
        Self {
            selected: None,
            front: None,
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
            template: None,
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

    /// Applies a user-supplied template to the generated document.
    ///
    /// A JSON Merge Patch, not a replacement: see [`apply_config_template`]
    /// and ADR 0005. Applied last, so what it produces is what gets validated
    /// and written — a template cannot smuggle a document past `sing-box
    /// check`.
    #[must_use]
    pub const fn with_template(mut self, template: &'a Value) -> Self {
        self.template = Some(template);
        self
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

    /// Dials every proxy outbound through `node` first, the way v2rayN's front
    /// proxy chains one server in front of another.
    #[must_use]
    pub const fn with_front_node(
        mut self,
        node: &'a ProxyNode,
        credential: NodeCredential<'a>,
    ) -> Self {
        self.front = Some(SelectedNode { node, credential });
        self
    }

    /// Emits a multi-node group outbound tagged `proxy` when `members` has at
    /// least two nodes. Fewer members fall back to the selected single outbound.
    #[must_use]
    pub fn with_group_outbound(
        mut self,
        strategy: NodeGroupStrategy,
        members: Vec<(&'a ProxyNode, NodeCredential<'a>)>,
        probe: &'a GroupProbe,
    ) -> Self {
        self.group_outbound = Some(GroupOutbound {
            strategy,
            members: members
                .into_iter()
                .map(|(node, credential)| SelectedNode { node, credential })
                .collect(),
            probe,
        });
        self
    }

    /// Emits a URL-TEST group tagged `proxy` when `members` has at least two
    /// nodes. Fewer members fall back to the selected single outbound.
    #[must_use]
    pub fn with_urltest(
        self,
        members: Vec<(&'a ProxyNode, NodeCredential<'a>)>,
        probe: &'a GroupProbe,
    ) -> Self {
        self.with_group_outbound(NodeGroupStrategy::UrlTest, members, probe)
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
            // The front comes first so the outbound it fronts can name it.
            if let Some((_, front)) = front_outbound(profile)? {
                outbounds.push(front);
            }
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
        } else if profile.route.requires_sniffing() {
            // A TUN session already sniffs. Everywhere else it stays off until
            // a rule asks what the traffic is, because it costs work on every
            // connection — and without it that rule matches nothing.
            prepend_sniff_action(&mut route);
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

        if let Some(template) = profile.template {
            // Last, so the Core validates what the user will actually run.
            apply_config_template(&mut config, template);
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
        let probe = group.probe.url();
        let interval = group.probe.interval();
        let group_outbound = match group.strategy {
            NodeGroupStrategy::UrlTest => json!({
                "type": "urltest",
                "tag": "proxy",
                "outbounds": tags,
                "url": probe,
                "interval": interval,
                "tolerance": group.probe.tolerance_ms(),
            }),
            NodeGroupStrategy::Fallback => json!({
                "type": "fallback",
                "tag": "proxy",
                "outbounds": tags,
                "url": probe,
                "interval": interval,
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

/// The front node's own outbound, dialled directly, plus its tag.
fn front_outbound(
    profile: &SingBoxRuntimeProfile<'_>,
) -> Result<Option<(String, Value)>, RuntimeConfigError> {
    let Some(front) = profile.front else {
        return Ok(None);
    };
    let tag = front_outbound_tag(front.node);
    // Built without the front applied to itself: the chain has to end somewhere.
    let mut outbound = SingBoxOutboundConfigGenerator::generate(front.node, front.credential)?
        .json()
        .clone();
    outbound["tag"] = Value::String(tag.clone());
    Ok(Some((tag, outbound)))
}

fn front_outbound_tag(node: &ProxyNode) -> String {
    format!("front-{}", node.id)
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
    if let Some(front) = profile.front {
        outbound["detour"] = Value::String(front_outbound_tag(front.node));
    }
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

fn prepend_sniff_action(route: &mut Value) {
    route["rules"]
        .as_array_mut()
        .expect("route generator always emits an array of rules")
        .insert(0, json!({ "action": "sniff" }));
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
