//! Turns a [`DnsProfile`] into an Xray `dns` section.
//!
//! The two Cores model DNS differently enough that this is a translation, not a
//! renaming:
//!
//! - sing-box gives every server a tag and routes to it with separate rules;
//!   Xray has no tags, and each server carries the `domains` it answers for
//! - sing-box names a `final` server; in Xray a server with no `domains`
//!   answers everything, so the fallback is expressed by placing it last
//! - sing-box's fake-IP is a server type; Xray's is a separate `fakedns` object
//!   that also needs inbound sniffing, which the runtime assembler adds

use serde_json::{Value, json};

use crate::dns_config::{
    DnsProfile, DnsRuleKind, DnsServerKind, DnsStrategy, GeneratedDnsConfig, PlainDnsTransport,
};

/// The address Xray recognises as its fake-DNS pool.
pub const FAKE_DNS_SERVER: &str = "fakedns";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XrayDnsConfigGenerator;

impl XrayDnsConfigGenerator {
    /// Generates the `dns` section for one profile.
    ///
    /// When the profile asks for fake IP the returned config lists the
    /// `fakedns` server; the caller is responsible for the top-level `fakedns`
    /// object and the inbound sniffing that makes it effective.
    #[must_use]
    pub fn generate(profile: &DnsProfile) -> GeneratedDnsConfig {
        let mut servers = Vec::new();
        if profile.fake_ip_enabled() {
            servers.push(Value::String(FAKE_DNS_SERVER.to_owned()));
        }

        for server in profile.servers() {
            let domains = domains_for(profile, server.tag());
            let mut entry = server_entry(server.kind());
            // A server with no `domains` answers everything, which is what makes
            // the profile's final server the fallback; every other server is
            // restricted to the names its rules named.
            if !domains.is_empty() && server.tag() != profile.final_server() {
                entry = ensure_object(entry);
                entry["domains"] = json!(domains);
            }
            servers.push(entry);
        }

        // Xray tries servers in order, so the catch-all has to come last.
        let final_index = profile
            .servers()
            .iter()
            .position(|server| server.tag() == profile.final_server());
        if let Some(index) = final_index {
            let offset = usize::from(profile.fake_ip_enabled());
            let entry = servers.remove(index + offset);
            servers.push(entry);
        }

        GeneratedDnsConfig::from_json(json!({
            "servers": servers,
            "queryStrategy": query_strategy(profile.strategy()),
        }))
    }
}

/// Collects every domain expression routed to `tag`, using the same prefixes as
/// Xray routing.
fn domains_for(profile: &DnsProfile, tag: &str) -> Vec<String> {
    profile
        .rules()
        .iter()
        .filter(|rule| rule.server() == tag)
        .flat_map(|rule| {
            let kind = rule.kind();
            rule.values().iter().map(move |value| match kind {
                DnsRuleKind::Domain => format!("full:{value}"),
                DnsRuleKind::DomainSuffix => {
                    format!("domain:{}", value.trim_start_matches('.'))
                }
                DnsRuleKind::DomainKeyword => format!("keyword:{value}"),
            })
        })
        .collect()
}

/// Renders one server in the shortest form Xray accepts.
fn server_entry(kind: &DnsServerKind) -> Value {
    match kind {
        DnsServerKind::System => Value::String("localhost".to_owned()),
        DnsServerKind::Plain {
            transport,
            server,
            port,
            ..
        } => match transport {
            PlainDnsTransport::Udp => json!({ "address": server, "port": port.get() }),
            // Xray selects plain TCP with a scheme on the address itself.
            PlainDnsTransport::Tcp => {
                json!({ "address": format!("tcp://{server}"), "port": port.get() })
            }
        },
        DnsServerKind::Doh {
            server, port, path, ..
        } => json!({
            "address": format!("https://{server}{}", normalized_path(path)),
            "port": port.get(),
        }),
        DnsServerKind::Dot { server, port, .. } => json!({
            "address": format!("tls://{server}"),
            "port": port.get(),
        }),
    }
}

/// Promotes a bare address string to the object form so `domains` can be added.
fn ensure_object(entry: Value) -> Value {
    match entry {
        Value::String(address) => json!({ "address": address }),
        other => other,
    }
}

fn normalized_path(path: &str) -> String {
    if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    }
}

/// Xray has no preference strategies, only hard choices, so both `Prefer`
/// variants collapse to `UseIP` and the ordering hint is lost.
const fn query_strategy(strategy: DnsStrategy) -> &'static str {
    match strategy {
        DnsStrategy::Ipv4Only => "UseIPv4",
        DnsStrategy::Ipv6Only => "UseIPv6",
        DnsStrategy::PreferIpv4 | DnsStrategy::PreferIpv6 => "UseIP",
    }
}
