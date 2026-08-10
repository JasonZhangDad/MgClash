use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::num::NonZeroU16;

use serde_json::{Value, json};
use url::Host;

const FAKE_IP_TAG: &str = "fakeip";
const FAKE_IP_V4_RANGE: &str = "198.18.0.0/15";
const FAKE_IP_V6_RANGE: &str = "fc00::/18";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DnsStrategy {
    PreferIpv4,
    PreferIpv6,
    Ipv4Only,
    Ipv6Only,
}

impl DnsStrategy {
    const fn as_str(self) -> &'static str {
        match self {
            Self::PreferIpv4 => "prefer_ipv4",
            Self::PreferIpv6 => "prefer_ipv6",
            Self::Ipv4Only => "ipv4_only",
            Self::Ipv6Only => "ipv6_only",
        }
    }

    const fn requires_ipv6(self) -> bool {
        matches!(self, Self::PreferIpv6 | Self::Ipv6Only)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlainDnsTransport {
    Udp,
    Tcp,
}

impl PlainDnsTransport {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Udp => "udp",
            Self::Tcp => "tcp",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DnsServer {
    tag: String,
    kind: DnsServerKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DnsServerKind {
    System,
    Plain {
        transport: PlainDnsTransport,
        server: String,
        port: NonZeroU16,
        bootstrap: Option<String>,
    },
    Doh {
        server: String,
        port: NonZeroU16,
        path: String,
        bootstrap: String,
    },
    Dot {
        server: String,
        port: NonZeroU16,
        bootstrap: String,
    },
}

impl DnsServer {
    /// Creates a system DNS server.
    ///
    /// # Errors
    ///
    /// Returns a typed error when `tag` is empty.
    pub fn system(tag: &str) -> Result<Self, DnsConfigError> {
        Ok(Self {
            tag: validated_tag(tag)?,
            kind: DnsServerKind::System,
        })
    }

    /// Creates a plain UDP or TCP DNS server.
    ///
    /// # Errors
    ///
    /// Returns a typed error for invalid fields or a domain server without a
    /// bootstrap resolver.
    pub fn plain(
        tag: &str,
        transport: PlainDnsTransport,
        server: &str,
        port: u32,
        bootstrap: Option<&str>,
    ) -> Result<Self, DnsConfigError> {
        let tag = validated_tag(tag)?;
        let (server, is_domain) = validated_server(server)?;
        let bootstrap = bootstrap.map(validated_tag).transpose()?;
        if is_domain && bootstrap.is_none() {
            return Err(DnsConfigError::BootstrapRequired { tag });
        }
        Ok(Self {
            tag,
            kind: DnsServerKind::Plain {
                transport,
                server,
                port: validated_port(port)?,
                bootstrap,
            },
        })
    }

    /// Creates a DNS-over-HTTPS server.
    ///
    /// # Errors
    ///
    /// Returns a typed error for invalid fields or a path without a leading
    /// slash.
    pub fn doh(
        tag: &str,
        server: &str,
        port: u32,
        path: &str,
        bootstrap: &str,
    ) -> Result<Self, DnsConfigError> {
        let path = path.trim();
        if !path.starts_with('/') {
            return Err(DnsConfigError::InvalidDohPath);
        }
        Ok(Self {
            tag: validated_tag(tag)?,
            kind: DnsServerKind::Doh {
                server: validated_server(server)?.0,
                port: validated_port(port)?,
                path: path.to_owned(),
                bootstrap: validated_tag(bootstrap)?,
            },
        })
    }

    /// Creates a DNS-over-TLS server.
    ///
    /// # Errors
    ///
    /// Returns a typed error for invalid fields.
    pub fn dot(
        tag: &str,
        server: &str,
        port: u32,
        bootstrap: &str,
    ) -> Result<Self, DnsConfigError> {
        Ok(Self {
            tag: validated_tag(tag)?,
            kind: DnsServerKind::Dot {
                server: validated_server(server)?.0,
                port: validated_port(port)?,
                bootstrap: validated_tag(bootstrap)?,
            },
        })
    }

    fn bootstrap(&self) -> Option<&str> {
        match &self.kind {
            DnsServerKind::System => None,
            DnsServerKind::Plain { bootstrap, .. } => bootstrap.as_deref(),
            DnsServerKind::Doh { bootstrap, .. } | DnsServerKind::Dot { bootstrap, .. } => {
                Some(bootstrap)
            }
        }
    }

    fn json(&self) -> Value {
        match &self.kind {
            DnsServerKind::System => json!({
                "type": "local",
                "tag": self.tag
            }),
            DnsServerKind::Plain {
                transport,
                server,
                port,
                bootstrap,
            } => {
                let mut value = json!({
                    "type": transport.as_str(),
                    "tag": self.tag,
                    "server": server,
                    "server_port": port.get()
                });
                if let Some(bootstrap) = bootstrap {
                    value["domain_resolver"] = Value::String(bootstrap.clone());
                }
                value
            }
            DnsServerKind::Doh {
                server,
                port,
                path,
                bootstrap,
            } => json!({
                "type": "https",
                "tag": self.tag,
                "server": server,
                "server_port": port.get(),
                "path": path,
                "tls": { "enabled": true, "server_name": server },
                "domain_resolver": bootstrap
            }),
            DnsServerKind::Dot {
                server,
                port,
                bootstrap,
            } => json!({
                "type": "tls",
                "tag": self.tag,
                "server": server,
                "server_port": port.get(),
                "tls": { "enabled": true, "server_name": server },
                "domain_resolver": bootstrap
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DnsRule {
    kind: DnsRuleKind,
    values: Vec<String>,
    server: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DnsRuleKind {
    Domain,
    DomainSuffix,
    DomainKeyword,
}

impl DnsRuleKind {
    const fn field(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::DomainSuffix => "domain_suffix",
            Self::DomainKeyword => "domain_keyword",
        }
    }
}

impl DnsRule {
    /// Creates an exact-domain DNS rule.
    ///
    /// # Errors
    ///
    /// Returns a typed error for empty values or target tags.
    pub fn domain(values: Vec<String>, server: &str) -> Result<Self, DnsConfigError> {
        Self::new(DnsRuleKind::Domain, values, server)
    }

    /// Creates a domain-suffix DNS rule.
    ///
    /// # Errors
    ///
    /// Returns a typed error for empty values or target tags.
    pub fn domain_suffix(values: Vec<String>, server: &str) -> Result<Self, DnsConfigError> {
        Self::new(DnsRuleKind::DomainSuffix, values, server)
    }

    /// Creates a domain-keyword DNS rule.
    ///
    /// # Errors
    ///
    /// Returns a typed error for empty values or target tags.
    pub fn domain_keyword(values: Vec<String>, server: &str) -> Result<Self, DnsConfigError> {
        Self::new(DnsRuleKind::DomainKeyword, values, server)
    }

    fn new(kind: DnsRuleKind, values: Vec<String>, server: &str) -> Result<Self, DnsConfigError> {
        if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
            return Err(DnsConfigError::EmptyRuleValues);
        }
        Ok(Self {
            kind,
            values: values
                .into_iter()
                .map(|value| value.trim().to_owned())
                .collect(),
            server: validated_tag(server)?,
        })
    }

    fn json(&self) -> Value {
        let mut value = json!({
            "action": "route",
            "server": self.server
        });
        value[self.kind.field()] = json!(self.values);
        value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DnsProfile {
    servers: Vec<DnsServer>,
    rules: Vec<DnsRule>,
    final_server: String,
    strategy: DnsStrategy,
    fake_ip_enabled: bool,
    ipv6_enabled: bool,
}

impl DnsProfile {
    /// Creates a validated DNS profile.
    ///
    /// # Errors
    ///
    /// Returns a typed error for missing or duplicate servers, invalid
    /// references, recursive bootstrap resolution, or an unavailable strategy.
    pub fn new(
        servers: Vec<DnsServer>,
        rules: Vec<DnsRule>,
        final_server: &str,
        strategy: DnsStrategy,
        fake_ip_enabled: bool,
        ipv6_enabled: bool,
    ) -> Result<Self, DnsConfigError> {
        if servers.is_empty() {
            return Err(DnsConfigError::NoServers);
        }
        if !ipv6_enabled && strategy.requires_ipv6() {
            return Err(DnsConfigError::Ipv6StrategyRequiresIpv6);
        }

        let mut by_tag = HashMap::new();
        for server in &servers {
            if by_tag.insert(server.tag.as_str(), server).is_some() {
                return Err(DnsConfigError::DuplicateTag {
                    tag: server.tag.clone(),
                });
            }
        }
        if fake_ip_enabled && by_tag.contains_key(FAKE_IP_TAG) {
            return Err(DnsConfigError::DuplicateTag {
                tag: FAKE_IP_TAG.to_owned(),
            });
        }

        let final_server = validated_tag(final_server)?;
        if !by_tag.contains_key(final_server.as_str()) {
            return Err(DnsConfigError::UnknownServer { tag: final_server });
        }
        for rule in &rules {
            if !(by_tag.contains_key(rule.server.as_str())
                || fake_ip_enabled && rule.server == FAKE_IP_TAG)
            {
                return Err(DnsConfigError::UnknownServer {
                    tag: rule.server.clone(),
                });
            }
        }
        for server in &servers {
            if let Some(bootstrap) = server.bootstrap() {
                if !by_tag.contains_key(bootstrap) {
                    return Err(DnsConfigError::UnknownBootstrap {
                        tag: bootstrap.to_owned(),
                    });
                }
            }
        }
        reject_bootstrap_cycles(&servers, &by_tag)?;

        Ok(Self {
            servers,
            rules,
            final_server,
            strategy,
            fake_ip_enabled,
            ipv6_enabled,
        })
    }

    pub(crate) fn final_server(&self) -> &str {
        &self.final_server
    }
}

fn reject_bootstrap_cycles(
    servers: &[DnsServer],
    by_tag: &HashMap<&str, &DnsServer>,
) -> Result<(), DnsConfigError> {
    for server in servers {
        let mut visited = HashSet::new();
        let mut current = server;
        while let Some(bootstrap) = current.bootstrap() {
            if !visited.insert(current.tag.as_str()) {
                return Err(DnsConfigError::RecursiveBootstrap {
                    tag: server.tag.clone(),
                });
            }
            current = by_tag[bootstrap];
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedDnsConfig {
    json: Value,
}

impl GeneratedDnsConfig {
    #[must_use]
    pub const fn json(&self) -> &Value {
        &self.json
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SingBoxDnsConfigGenerator;

impl SingBoxDnsConfigGenerator {
    #[must_use]
    pub fn generate(profile: &DnsProfile) -> GeneratedDnsConfig {
        let mut servers: Vec<_> = profile.servers.iter().map(DnsServer::json).collect();
        let mut rules: Vec<_> = profile.rules.iter().map(DnsRule::json).collect();
        if profile.fake_ip_enabled {
            let mut fake_ip = json!({
                "type": "fakeip",
                "tag": FAKE_IP_TAG,
                "inet4_range": FAKE_IP_V4_RANGE
            });
            let query_types = if profile.ipv6_enabled {
                vec!["A", "AAAA"]
            } else {
                vec!["A"]
            };
            if profile.ipv6_enabled {
                fake_ip["inet6_range"] = Value::String(FAKE_IP_V6_RANGE.to_owned());
            }
            servers.push(fake_ip);
            rules.push(json!({
                "query_type": query_types,
                "action": "route",
                "server": FAKE_IP_TAG
            }));
        }
        GeneratedDnsConfig {
            json: json!({
                "servers": servers,
                "rules": rules,
                "final": profile.final_server,
                "strategy": profile.strategy.as_str()
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DnsConfigError {
    #[error("DNS server tags must not be empty")]
    InvalidTag,
    #[error("DNS server address is invalid")]
    InvalidServerAddress,
    #[error("DNS server port must be between 1 and 65535, got {port}")]
    InvalidPort { port: u32 },
    #[error("DNS server {tag} uses a domain and requires a bootstrap resolver")]
    BootstrapRequired { tag: String },
    #[error("DNS-over-HTTPS path must start with a slash")]
    InvalidDohPath,
    #[error("DNS rules require at least one non-empty value")]
    EmptyRuleValues,
    #[error("DNS profile requires at least one server")]
    NoServers,
    #[error("duplicate DNS server tag: {tag}")]
    DuplicateTag { tag: String },
    #[error("unknown DNS server tag: {tag}")]
    UnknownServer { tag: String },
    #[error("unknown DNS bootstrap server tag: {tag}")]
    UnknownBootstrap { tag: String },
    #[error("recursive DNS bootstrap chain starting at {tag}")]
    RecursiveBootstrap { tag: String },
    #[error("the selected DNS strategy requires IPv6")]
    Ipv6StrategyRequiresIpv6,
}

fn validated_tag(value: &str) -> Result<String, DnsConfigError> {
    let value = value.trim();
    if value.is_empty() {
        Err(DnsConfigError::InvalidTag)
    } else {
        Ok(value.to_owned())
    }
}

fn validated_port(port: u32) -> Result<NonZeroU16, DnsConfigError> {
    u16::try_from(port)
        .ok()
        .and_then(NonZeroU16::new)
        .ok_or(DnsConfigError::InvalidPort { port })
}

fn validated_server(value: &str) -> Result<(String, bool), DnsConfigError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(DnsConfigError::InvalidServerAddress);
    }
    if value.parse::<IpAddr>().is_ok() {
        return Ok((value.to_owned(), false));
    }
    match Host::parse(value) {
        Ok(Host::Domain(_)) => Ok((value.to_owned(), true)),
        _ => Err(DnsConfigError::InvalidServerAddress),
    }
}
