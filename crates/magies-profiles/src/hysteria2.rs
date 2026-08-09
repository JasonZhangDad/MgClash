use std::fmt::{Debug, Formatter};
use std::num::NonZeroU16;

use magies_domain::{
    CredentialRef, NodeModelError, NodeName, ProxyNode, ProxyProtocol, ServerAddress, TlsConfig,
    TransportConfig,
};
use url::Url;
use uuid::Uuid;

use super::{
    QueryParameters, VlessParseError, decode_component, default_name, parse_alpn, parse_server,
    validate_percent_encoding,
};

const DEFAULT_PORT: u16 = 443;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Hysteria2Parser;

impl Hysteria2Parser {
    #[must_use]
    pub fn can_parse(&self, value: &str) -> bool {
        let value = value.trim();
        value.starts_with("hysteria2://") || value.starts_with("hy2://")
    }

    /// Parses a standard Hysteria 2 sharing URI without persisting secrets.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the URI is malformed, contains an invalid
    /// field, or uses an extension the shared node model cannot preserve.
    pub fn parse(&self, value: &str) -> Result<ParsedHysteria2Node, Hysteria2ParseError> {
        let value = value.trim();
        if !self.can_parse(value) {
            return Err(Hysteria2ParseError::UnsupportedScheme);
        }
        validate_percent_encoding(value).map_err(invalid_uri)?;
        if uses_authority_port_hopping(value) {
            return Err(Hysteria2ParseError::UnsupportedPortHopping);
        }

        let url =
            Url::parse(value).map_err(|source| Hysteria2ParseError::MalformedUrl { source })?;
        if !matches!(url.path(), "" | "/") {
            return Err(Hysteria2ParseError::UnexpectedPath);
        }
        let server = parse_server(&url).map_err(invalid_uri)?;
        let port = parse_port(&url)?;
        let authentication = parse_authentication(&url)?;

        let mut parameters = QueryParameters::parse(&url).map_err(invalid_uri)?;
        reject_unrepresentable_parameters(&mut parameters)?;
        let server_name = parameters.take_non_empty("sni").map_err(invalid_uri)?;
        let allow_insecure = parse_insecure(&mut parameters)?;
        let alpn = parse_alpn(&mut parameters).map_err(invalid_uri)?;
        let fingerprint = parameters.take_non_empty("fp").map_err(invalid_uri)?;
        let obfuscation = parse_obfuscation(&mut parameters)?;
        parameters.finish().map_err(invalid_uri)?;

        let fallback_name = default_name(&server, port);
        let name = match url.fragment() {
            Some(fragment) => {
                let decoded = decode_component(fragment).map_err(invalid_uri)?;
                if decoded.is_empty() {
                    fallback_name
                } else {
                    decoded
                }
            }
            None => fallback_name,
        };
        let name =
            NodeName::new(name).map_err(|source| Hysteria2ParseError::InvalidNode { source })?;

        Ok(ParsedHysteria2Node {
            name,
            server,
            port,
            credential: Hysteria2Credential {
                authentication,
                obfuscation,
            },
            tls: TlsConfig::Tls {
                server_name,
                allow_insecure,
                alpn,
                fingerprint,
            },
        })
    }
}

pub struct Hysteria2Credential {
    authentication: Option<String>,
    obfuscation: Option<Hysteria2Obfuscation>,
}

impl Hysteria2Credential {
    #[must_use]
    pub fn authentication(&self) -> Option<&str> {
        self.authentication.as_deref()
    }

    #[must_use]
    pub const fn obfuscation(&self) -> Option<&Hysteria2Obfuscation> {
        self.obfuscation.as_ref()
    }
}

impl Debug for Hysteria2Credential {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Hysteria2Credential([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Hysteria2ObfuscationMethod {
    Salamander,
    Gecko,
}

#[derive(Eq, PartialEq)]
pub struct Hysteria2Obfuscation {
    method: Hysteria2ObfuscationMethod,
    password: String,
    min_packet_size: Option<NonZeroU16>,
    max_packet_size: Option<NonZeroU16>,
}

impl Hysteria2Obfuscation {
    #[must_use]
    pub const fn method(&self) -> Hysteria2ObfuscationMethod {
        self.method
    }

    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }

    #[must_use]
    pub fn min_packet_size(&self) -> Option<u16> {
        self.min_packet_size.map(NonZeroU16::get)
    }

    #[must_use]
    pub fn max_packet_size(&self) -> Option<u16> {
        self.max_packet_size.map(NonZeroU16::get)
    }
}

impl Debug for Hysteria2Obfuscation {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Hysteria2Obfuscation")
            .field("method", &self.method)
            .field("password", &"[REDACTED]")
            .field("min_packet_size", &self.min_packet_size)
            .field("max_packet_size", &self.max_packet_size)
            .finish()
    }
}

#[derive(Debug)]
pub struct ParsedHysteria2Node {
    name: NodeName,
    server: ServerAddress,
    port: NonZeroU16,
    credential: Hysteria2Credential,
    tls: TlsConfig,
}

impl ParsedHysteria2Node {
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
    pub const fn credential(&self) -> &Hysteria2Credential {
        &self.credential
    }

    #[must_use]
    pub const fn transport(&self) -> Option<&TransportConfig> {
        None
    }

    #[must_use]
    pub const fn tls(&self) -> Option<&TlsConfig> {
        Some(&self.tls)
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
            ProxyProtocol::Hysteria2,
            self.server.as_str(),
            u32::from(self.port.get()),
            Some(credential_ref),
        )?;
        node.tls = Some(self.tls);
        Ok(node)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Hysteria2ParseError {
    #[error("URI scheme must be exactly hysteria2 or hy2")]
    UnsupportedScheme,
    #[error("malformed Hysteria2 URI")]
    MalformedUrl { source: url::ParseError },
    #[error("Hysteria2 URI path must be empty or /")]
    UnexpectedPath,
    #[error("Hysteria2 port must be between 1 and 65535, got {port}")]
    InvalidPort { port: u16 },
    #[error("invalid Hysteria2 URI field")]
    InvalidUri { source: VlessParseError },
    #[error("Hysteria2 parameter {name} must be 0 or 1, got {value}")]
    InvalidBoolean { name: &'static str, value: String },
    #[error("unsupported Hysteria2 obfuscation: {value}")]
    UnsupportedObfuscation { value: String },
    #[error("Hysteria2 obfuscation requires obfs-password")]
    MissingObfuscationPassword,
    #[error("Hysteria2 obfs-password requires obfs")]
    UnexpectedObfuscationPassword,
    #[error("Hysteria2 packet sizes are only supported for Gecko obfuscation")]
    UnexpectedGeckoPacketSize,
    #[error("Hysteria2 packet size {name} must be between 1 and 65535, got {value}")]
    InvalidPacketSize { name: &'static str, value: String },
    #[error("Hysteria2 minimum packet size must not exceed its maximum")]
    InvalidPacketSizeRange,
    #[error("Hysteria2 port hopping is not supported by the shared node model")]
    UnsupportedPortHopping,
    #[error("Hysteria2 certificate pinning is not supported by the shared TLS model")]
    UnsupportedCertificatePin,
    #[error("Hysteria2 ECH is not supported by the shared TLS model")]
    UnsupportedEch,
    #[error("invalid parsed Hysteria2 node")]
    InvalidNode { source: NodeModelError },
}

fn parse_port(url: &Url) -> Result<NonZeroU16, Hysteria2ParseError> {
    let port = url.port().unwrap_or(DEFAULT_PORT);
    NonZeroU16::new(port).ok_or(Hysteria2ParseError::InvalidPort { port })
}

fn parse_authentication(url: &Url) -> Result<Option<String>, Hysteria2ParseError> {
    let username = decode_component(url.username()).map_err(invalid_uri)?;
    match url.password() {
        Some(raw_password) => {
            let password = decode_component(raw_password).map_err(invalid_uri)?;
            Ok(Some(format!("{username}:{password}")))
        }
        None if username.is_empty() => Ok(None),
        None => Ok(Some(username)),
    }
}

fn parse_insecure(parameters: &mut QueryParameters) -> Result<bool, Hysteria2ParseError> {
    match parameters.take_non_empty("insecure").map_err(invalid_uri)? {
        None => Ok(false),
        Some(value) if value == "0" => Ok(false),
        Some(value) if value == "1" => Ok(true),
        Some(value) => Err(Hysteria2ParseError::InvalidBoolean {
            name: "insecure",
            value,
        }),
    }
}

fn parse_obfuscation(
    parameters: &mut QueryParameters,
) -> Result<Option<Hysteria2Obfuscation>, Hysteria2ParseError> {
    let method = parameters.take_non_empty("obfs").map_err(invalid_uri)?;
    let password = parameters
        .take_non_empty("obfs-password")
        .map_err(invalid_uri)?;
    let min_packet_size = parameters
        .take_non_empty("minPacketSize")
        .map_err(invalid_uri)?;
    let max_packet_size = parameters
        .take_non_empty("maxPacketSize")
        .map_err(invalid_uri)?;

    let Some(method) = method else {
        if password.is_some() {
            return Err(Hysteria2ParseError::UnexpectedObfuscationPassword);
        }
        if min_packet_size.is_some() || max_packet_size.is_some() {
            return Err(Hysteria2ParseError::UnexpectedGeckoPacketSize);
        }
        return Ok(None);
    };
    let password = password.ok_or(Hysteria2ParseError::MissingObfuscationPassword)?;
    let method = match method.as_str() {
        "salamander" => Hysteria2ObfuscationMethod::Salamander,
        "gecko" => Hysteria2ObfuscationMethod::Gecko,
        value => {
            return Err(Hysteria2ParseError::UnsupportedObfuscation {
                value: value.to_owned(),
            });
        }
    };
    if method == Hysteria2ObfuscationMethod::Salamander
        && (min_packet_size.is_some() || max_packet_size.is_some())
    {
        return Err(Hysteria2ParseError::UnexpectedGeckoPacketSize);
    }

    let min_packet_size = parse_packet_size("minPacketSize", min_packet_size)?;
    let max_packet_size = parse_packet_size("maxPacketSize", max_packet_size)?;
    if min_packet_size
        .zip(max_packet_size)
        .is_some_and(|(min, max)| min > max)
    {
        return Err(Hysteria2ParseError::InvalidPacketSizeRange);
    }
    Ok(Some(Hysteria2Obfuscation {
        method,
        password,
        min_packet_size,
        max_packet_size,
    }))
}

fn parse_packet_size(
    name: &'static str,
    value: Option<String>,
) -> Result<Option<NonZeroU16>, Hysteria2ParseError> {
    let Some(value) = value else {
        return Ok(None);
    };
    value
        .parse::<u16>()
        .ok()
        .and_then(NonZeroU16::new)
        .map(Some)
        .ok_or(Hysteria2ParseError::InvalidPacketSize { name, value })
}

fn reject_unrepresentable_parameters(
    parameters: &mut QueryParameters,
) -> Result<(), Hysteria2ParseError> {
    if parameters.take("mport").is_some() {
        return Err(Hysteria2ParseError::UnsupportedPortHopping);
    }
    if parameters.take("pinSHA256").is_some() || parameters.take("pcs").is_some() {
        return Err(Hysteria2ParseError::UnsupportedCertificatePin);
    }
    if parameters.take("ech").is_some() {
        return Err(Hysteria2ParseError::UnsupportedEch);
    }
    Ok(())
}

fn uses_authority_port_hopping(value: &str) -> bool {
    let Some((_, payload)) = value.split_once("://") else {
        return false;
    };
    let authority_end = payload.find(['/', '?', '#']).unwrap_or(payload.len());
    let authority = &payload[..authority_end];
    let endpoint = authority
        .rsplit_once('@')
        .map_or(authority, |(_, endpoint)| endpoint);
    let port = if let Some(bracket_end) = endpoint.find(']') {
        endpoint
            .get(bracket_end + 1..)
            .and_then(|suffix| suffix.strip_prefix(':'))
    } else {
        endpoint.rsplit_once(':').map(|(_, port)| port)
    };
    port.is_some_and(|port| port.contains(',') || port.contains('-'))
}

fn invalid_uri(source: VlessParseError) -> Hysteria2ParseError {
    Hysteria2ParseError::InvalidUri { source }
}
