//! Node import and profile services for `MgClash`.

mod credential_codec;
mod diagnostics;
mod dns_config;
mod hysteria2;
mod local_proxy_config;
mod manual_node_draft;
mod manual_node_store;
mod node_dedup;
mod node_group_store;
mod node_order_store;
mod shadowsocks;
mod share_link;
mod sing_box_outbound;
mod sing_box_runtime_config;
mod subscription;
mod subscription_content;
mod subscription_management;
mod subscription_service;
mod subscription_transaction;
mod trojan;
mod tun_config;
mod vmess;

pub use credential_codec::{CredentialCodec, CredentialCodecError, StoredNodeCredential};
pub use diagnostics::{DiagnosticRedactor, REDACTED};
pub use dns_config::{
    DnsConfigError, DnsProfile, DnsRule, DnsServer, DnsStrategy, GeneratedDnsConfig,
    PlainDnsTransport, SingBoxDnsConfigGenerator,
};
pub use hysteria2::{
    Hysteria2Credential, Hysteria2Obfuscation, Hysteria2ObfuscationMethod, Hysteria2ParseError,
    Hysteria2Parser, ParsedHysteria2Node,
};
pub use local_proxy_config::{
    DEFAULT_HTTP_PORT, DEFAULT_SOCKS_PORT, GeneratedCoreConfig, LocalHttpConfigGenerator,
    LocalHttpProfile, LocalProxyConfigError, LocalSocksConfigGenerator, LocalSocksProfile,
};
pub use manual_node_draft::{
    ManualCredentialDraft, ManualNodeDraft, ManualNodeDraftError, ManualObfuscationDraft,
};
pub use manual_node_store::{ManualNodeStoreError, SqliteManualNodeStore};
pub use node_dedup::{
    CredentialIdentity, NodeDedupCandidate, NodeDedupResult, NodeDedupSummary, NodeDeduplicator,
    NodeFingerprint,
};
pub use node_group_store::{NodeGroup, NodeGroupStoreError, SqliteNodeGroupStore};
pub use node_order_store::{NodeOrderStoreError, SqliteNodeOrderStore};
pub use shadowsocks::{
    ParsedShadowsocksNode, ShadowsocksCredential, ShadowsocksParseError, ShadowsocksParser,
};
pub use share_link::{ParsedShareLink, ShareLinkParseError, ShareLinkParser};
pub use sing_box_outbound::{
    GeneratedSingBoxOutbound, NodeCredential, OutboundConfigError, SingBoxOutboundConfigGenerator,
};
pub use sing_box_runtime_config::{
    RuntimeConfigError, SingBoxRuntimeConfigGenerator, SingBoxRuntimeProfile,
};
pub use subscription::{
    SubscriptionFetchError, SubscriptionFetchOptions, SubscriptionFetchResult, SubscriptionFetcher,
    SubscriptionValidators, ensure_rustls_crypto_provider,
};
pub use subscription_content::{
    ParsedSubscriptionNode, SubscriptionContentError, SubscriptionContentParser,
};
pub use subscription_management::{
    SubscriptionManagementError, SubscriptionManagementSecretOperation,
    SubscriptionManagementService,
};
pub use subscription_service::{
    SubscriptionRefreshError, SubscriptionRefreshOutcome, SubscriptionRefreshService,
    SubscriptionSecretOperation,
};
pub use subscription_transaction::{
    DeletedSubscription, SqliteSubscriptionStore, SubscriptionCommit, SubscriptionState,
    SubscriptionTransactionError, SubscriptionUpdate,
};
pub use trojan::{ParsedTrojanNode, TrojanCredential, TrojanParseError, TrojanParser};
pub use tun_config::{SingBoxTunConfigGenerator, TunProfile, TunProfileError, TunRouteSettings};
pub use vmess::{ParsedVmessNode, VmessCredential, VmessParseError, VmessParser, VmessSecurity};

use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::num::NonZeroU16;

use magies_domain::{
    CredentialRef, GrpcMode, NodeModelError, NodeName, ProxyNode, ProxyProtocol, ServerAddress,
    TlsConfig, TransportConfig,
};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use url::{Host, Url};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VlessParser;

impl VlessParser {
    #[must_use]
    pub fn can_parse(&self, value: &str) -> bool {
        value.trim().starts_with("vless://")
    }

    /// Parses a VLESS sharing URI without persisting its credential.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the URI is malformed, repeats or contains an
    /// unsupported parameter, or cannot be represented by the shared model.
    pub fn parse(&self, value: &str) -> Result<ParsedVlessNode, VlessParseError> {
        self.parse_with_default_encryption(value, "none")
    }

    pub(crate) fn parse_with_default_encryption(
        self,
        value: &str,
        default_encryption: &str,
    ) -> Result<ParsedVlessNode, VlessParseError> {
        let value = value.trim();
        if !self.can_parse(value) {
            return Err(VlessParseError::UnsupportedScheme);
        }
        validate_percent_encoding(value)?;

        let url = Url::parse(value).map_err(|source| VlessParseError::MalformedUrl { source })?;
        if !url.path().is_empty() {
            return Err(VlessParseError::UnexpectedPath);
        }
        if url.password().is_some() {
            return Err(VlessParseError::UnexpectedPassword);
        }

        let raw_user_id = url.username();
        if raw_user_id.is_empty() {
            return Err(VlessParseError::MissingUserId);
        }
        let user_id = decode_component(raw_user_id)?;
        let user_id = Uuid::parse_str(&user_id)
            .map_err(|source| VlessParseError::InvalidUserId { source })?;
        let server = parse_server(&url)?;
        let port = parse_port(&url)?;

        let mut parameters = QueryParameters::parse(&url)?;
        let encryption = parameters
            .take_non_empty("encryption")?
            .unwrap_or_else(|| default_encryption.to_owned());
        let flow = parameters.take("flow");
        let transport = parse_transport(&mut parameters)?;
        let tls = parse_tls(&mut parameters, &server)?;
        parameters.finish()?;

        let fallback_name = default_name(&server, port);
        let name = match url.fragment() {
            Some(fragment) => {
                let decoded = decode_component(fragment)?;
                if decoded.is_empty() {
                    fallback_name
                } else {
                    decoded
                }
            }
            None => fallback_name,
        };
        let name = NodeName::new(name).map_err(|source| VlessParseError::InvalidNode { source })?;

        Ok(ParsedVlessNode {
            name,
            server,
            port,
            credential: VlessCredential {
                user_id,
                encryption,
                flow,
            },
            transport,
            tls,
        })
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct VlessCredential {
    pub(crate) user_id: Uuid,
    pub(crate) encryption: String,
    pub(crate) flow: Option<String>,
}

impl VlessCredential {
    #[must_use]
    pub const fn user_id(&self) -> Uuid {
        self.user_id
    }

    #[must_use]
    pub fn encryption(&self) -> &str {
        &self.encryption
    }

    #[must_use]
    pub fn flow(&self) -> Option<&str> {
        self.flow.as_deref()
    }
}

impl Debug for VlessCredential {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("VlessCredential([REDACTED])")
    }
}

#[derive(Debug)]
pub struct ParsedVlessNode {
    name: NodeName,
    server: ServerAddress,
    port: NonZeroU16,
    credential: VlessCredential,
    transport: TransportConfig,
    tls: Option<TlsConfig>,
}

impl ParsedVlessNode {
    #[must_use]
    pub const fn name(&self) -> &NodeName {
        &self.name
    }

    #[must_use]
    pub const fn server(&self) -> &ServerAddress {
        &self.server
    }

    #[must_use]
    pub const fn port(&self) -> NonZeroU16 {
        self.port
    }

    #[must_use]
    pub const fn credential(&self) -> &VlessCredential {
        &self.credential
    }

    #[must_use]
    pub const fn transport(&self) -> &TransportConfig {
        &self.transport
    }

    #[must_use]
    pub const fn tls(&self) -> Option<&TlsConfig> {
        self.tls.as_ref()
    }

    /// Creates the unified node after the caller stores the parsed credential.
    ///
    /// # Errors
    ///
    /// Returns the shared model's validation error if the supplied credential
    /// reference is invalid.
    pub fn into_proxy_node(
        self,
        id: Uuid,
        credential_ref: CredentialRef,
    ) -> Result<ProxyNode, NodeModelError> {
        let mut node = ProxyNode::new(
            id,
            self.name.as_str(),
            ProxyProtocol::Vless,
            self.server.as_str(),
            u32::from(self.port.get()),
            Some(credential_ref),
        )?;
        node.transport = Some(self.transport);
        node.tls = self.tls;
        Ok(node)
    }
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum VlessParseError {
    #[error("URI scheme must be exactly vless")]
    UnsupportedScheme,
    #[error("malformed VLESS URI")]
    MalformedUrl { source: url::ParseError },
    #[error("VLESS URI path must be empty")]
    UnexpectedPath,
    #[error("VLESS URI user ID must not contain a password")]
    UnexpectedPassword,
    #[error("VLESS user ID is required")]
    MissingUserId,
    #[error("invalid VLESS user ID")]
    InvalidUserId { source: uuid::Error },
    #[error("VLESS server is required")]
    MissingServer,
    #[error("VLESS port is required")]
    MissingPort,
    #[error("VLESS port must be between 1 and 65535, got {port}")]
    InvalidPort { port: u16 },
    #[error("VLESS URI contains invalid percent encoding")]
    InvalidPercentEncoding,
    #[error("duplicate VLESS parameter: {name}")]
    DuplicateParameter { name: String },
    #[error("VLESS parameter {name} must not be empty")]
    EmptyParameter { name: &'static str },
    #[error("VLESS parameter {name} is required")]
    MissingParameter { name: &'static str },
    #[error("unsupported VLESS transport: {value}")]
    UnsupportedTransport { value: String },
    #[error("unsupported VLESS transport security: {value}")]
    UnsupportedSecurity { value: String },
    #[error("unsupported VLESS gRPC mode: {value}")]
    UnsupportedGrpcMode { value: String },
    #[error("invalid VLESS ALPN list")]
    InvalidAlpn,
    #[error("unsupported VLESS parameter: {name}")]
    UnsupportedParameter { name: String },
    #[error("invalid parsed VLESS node")]
    InvalidNode { source: NodeModelError },
}

#[derive(Default)]
struct QueryParameters {
    entries: BTreeMap<String, String>,
}

impl QueryParameters {
    fn parse(url: &Url) -> Result<Self, VlessParseError> {
        let mut parameters = Self::default();
        let Some(query) = url.query() else {
            return Ok(parameters);
        };
        if query.is_empty() {
            return Ok(parameters);
        }

        for pair in query.split('&') {
            let (raw_name, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
            let name = decode_component(raw_name)?;
            let value = decode_component(raw_value)?;
            if parameters.entries.insert(name.clone(), value).is_some() {
                return Err(VlessParseError::DuplicateParameter { name });
            }
        }
        Ok(parameters)
    }

    fn take(&mut self, name: &str) -> Option<String> {
        self.entries.remove(name)
    }

    fn take_non_empty(&mut self, name: &'static str) -> Result<Option<String>, VlessParseError> {
        match self.take(name) {
            Some(value) if value.is_empty() => Err(VlessParseError::EmptyParameter { name }),
            value => Ok(value),
        }
    }

    fn take_required_non_empty(&mut self, name: &'static str) -> Result<String, VlessParseError> {
        self.take_non_empty(name)?
            .ok_or(VlessParseError::MissingParameter { name })
    }

    fn finish(self) -> Result<(), VlessParseError> {
        if let Some(name) = self.entries.into_keys().next() {
            Err(VlessParseError::UnsupportedParameter { name })
        } else {
            Ok(())
        }
    }
}

fn parse_server(url: &Url) -> Result<ServerAddress, VlessParseError> {
    let server = match url.host() {
        Some(Host::Domain(domain)) => domain.to_owned(),
        Some(Host::Ipv4(address)) => address.to_string(),
        Some(Host::Ipv6(address)) => address.to_string(),
        None => return Err(VlessParseError::MissingServer),
    };
    ServerAddress::new(server).map_err(|source| VlessParseError::InvalidNode { source })
}

fn parse_port(url: &Url) -> Result<NonZeroU16, VlessParseError> {
    let port = url.port().ok_or(VlessParseError::MissingPort)?;
    NonZeroU16::new(port).ok_or(VlessParseError::InvalidPort { port })
}

fn parse_transport(parameters: &mut QueryParameters) -> Result<TransportConfig, VlessParseError> {
    let transport = parameters
        .take_non_empty("type")?
        .unwrap_or_else(|| "tcp".to_owned());
    match transport.as_str() {
        "tcp" => Ok(TransportConfig::Tcp),
        "ws" => Ok(TransportConfig::WebSocket {
            path: parameters
                .take_non_empty("path")?
                .unwrap_or_else(|| "/".to_owned()),
            host: parameters.take("host"),
        }),
        "grpc" => {
            let service_name = parameters.take_required_non_empty("serviceName")?;
            let mode = match parameters
                .take_non_empty("mode")?
                .as_deref()
                .unwrap_or("gun")
            {
                "gun" => GrpcMode::Gun,
                "multi" => GrpcMode::Multi,
                "guna" => GrpcMode::Guna,
                value => {
                    return Err(VlessParseError::UnsupportedGrpcMode {
                        value: value.to_owned(),
                    });
                }
            };
            Ok(TransportConfig::Grpc {
                service_name,
                mode,
                authority: parameters.take("authority"),
            })
        }
        value => Err(VlessParseError::UnsupportedTransport {
            value: value.to_owned(),
        }),
    }
}

fn parse_tls(
    parameters: &mut QueryParameters,
    server: &ServerAddress,
) -> Result<Option<TlsConfig>, VlessParseError> {
    parse_tls_with_default(parameters, server, "none")
}

fn parse_tls_with_default(
    parameters: &mut QueryParameters,
    server: &ServerAddress,
    default_security: &str,
) -> Result<Option<TlsConfig>, VlessParseError> {
    let security = parameters
        .take_non_empty("security")?
        .unwrap_or_else(|| default_security.to_owned());
    match security.as_str() {
        "none" => Ok(None),
        "tls" => Ok(Some(TlsConfig::Tls {
            server_name: parameters.take_non_empty("sni")?,
            allow_insecure: false,
            alpn: parse_alpn(parameters)?,
            fingerprint: parameters.take_non_empty("fp")?,
        })),
        "reality" => {
            let public_key = parameters.take_required_non_empty("pbk")?;
            let fingerprint = parameters.take_required_non_empty("fp")?;
            Ok(Some(TlsConfig::Reality {
                server_name: parameters
                    .take_non_empty("sni")?
                    .unwrap_or_else(|| server.as_str().to_owned()),
                public_key,
                short_id: parameters.take("sid"),
                fingerprint: Some(fingerprint),
                alpn: parse_alpn(parameters)?,
                spider_x: parameters.take("spx"),
            }))
        }
        value => Err(VlessParseError::UnsupportedSecurity {
            value: value.to_owned(),
        }),
    }
}

fn parse_alpn(parameters: &mut QueryParameters) -> Result<Vec<String>, VlessParseError> {
    let Some(alpn) = parameters.take_non_empty("alpn")? else {
        return Ok(Vec::new());
    };
    let values: Vec<_> = alpn.split(',').map(str::to_owned).collect();
    if values
        .iter()
        .any(|value| value.is_empty() || value.trim() != value)
    {
        Err(VlessParseError::InvalidAlpn)
    } else {
        Ok(values)
    }
}

fn default_name(server: &ServerAddress, port: NonZeroU16) -> String {
    if server.as_str().contains(':') {
        format!("[{}]:{port}", server.as_str())
    } else {
        format!("{}:{port}", server.as_str())
    }
}

fn decode_component(value: &str) -> Result<String, VlessParseError> {
    percent_decode_str(value)
        .decode_utf8()
        .map(Into::into)
        .map_err(|_| VlessParseError::InvalidPercentEncoding)
}

fn validate_percent_encoding(value: &str) -> Result<(), VlessParseError> {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let encoded = bytes.get(index + 1..index + 3);
            if !encoded.is_some_and(|digits| digits.iter().all(u8::is_ascii_hexdigit)) {
                return Err(VlessParseError::InvalidPercentEncoding);
            }
            index += 3;
        } else {
            index += 1;
        }
    }
    Ok(())
}
