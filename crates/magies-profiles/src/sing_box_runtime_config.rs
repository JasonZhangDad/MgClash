use magies_domain::{CoreType, ProxyNode};
use magies_routing::{RouteProfile, SingBoxRouteConfigGenerator};
use serde_json::{Value, json};

use crate::{
    DnsProfile, GeneratedCoreConfig, LocalHttpConfigGenerator, LocalHttpProfile,
    LocalSocksConfigGenerator, LocalSocksProfile, NodeCredential, OutboundConfigError,
    SingBoxDnsConfigGenerator, SingBoxOutboundConfigGenerator, SingBoxTunConfigGenerator,
    TunProfile,
};

pub struct SingBoxRuntimeProfile<'a> {
    selected: Option<SelectedNode<'a>>,
    dns: &'a DnsProfile,
    route: &'a RouteProfile,
    socks: LocalSocksProfile,
    http: LocalHttpProfile,
    tun: Option<&'a TunProfile>,
    dns_hijack: bool,
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
            dns,
            route,
            socks: LocalSocksProfile::default(),
            http: LocalHttpProfile::default(),
            tun: None,
            dns_hijack: false,
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
            tun: None,
            dns_hijack: false,
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
        self.socks = socks;
        self.http = http;
        Ok(self)
    }

    #[must_use]
    pub const fn with_tun(mut self, tun: &'a TunProfile, dns_hijack: bool) -> Self {
        self.tun = Some(tun);
        self.dns_hijack = dns_hijack;
        self
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
            let selected = profile
                .selected
                .ok_or(RuntimeConfigError::MissingSelectedNode)?;
            outbounds.push(
                SingBoxOutboundConfigGenerator::generate(selected.node, selected.credential)?
                    .json()
                    .clone(),
            );
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
        route["auto_detect_interface"] = Value::Bool(true);
        route["default_domain_resolver"] = Value::String(profile.dns.final_server().to_owned());

        Ok(GeneratedCoreConfig::from_json(
            CoreType::SingBox,
            json!({
                "log": { "level": "warn" },
                "dns": SingBoxDnsConfigGenerator::generate(profile.dns).json(),
                "inbounds": inbounds,
                "outbounds": outbounds,
                "route": route
            }),
        ))
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeConfigError {
    #[error("local SOCKS and HTTP listeners cannot share port {port}")]
    DuplicateLocalPort { port: u16 },
    #[error("routing rules require a selected proxy node")]
    MissingSelectedNode,
    #[error(transparent)]
    Outbound(#[from] OutboundConfigError),
}
