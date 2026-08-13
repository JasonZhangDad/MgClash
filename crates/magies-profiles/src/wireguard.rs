//! `WireGuard` sharing links.
//!
//! There is no protocol-level standard for a `WireGuard` sharing URI — the
//! `.conf` file is the only thing `WireGuard` itself defines — so this follows
//! the informal `wireguard://` shape v2rayN-family clients already emit:
//! the private key as userinfo, the peer's public key and tunnel addresses as
//! query parameters. `WireGuard` carries no stream transport and no TLS layer of
//! its own; it *is* the tunnel, so both are always absent on the shared model.

use std::fmt::{Debug, Formatter};
use std::num::NonZeroU16;

use magies_domain::{
    CredentialRef, NodeModelError, NodeName, ProxyNode, ProxyProtocol, ServerAddress,
    TransportConfig,
};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use super::{
    QueryParameters, VlessParseError, decode_component, default_name, parse_port_with_default,
    parse_server, validate_percent_encoding,
};

/// `WireGuard` has no IANA-assigned default; this matches the upstream
/// `wireguard-tools` example configs and every client that emits this link.
const DEFAULT_PORT: u16 = 51820;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WireGuardParser;

impl WireGuardParser {
    #[must_use]
    pub fn can_parse(&self, value: &str) -> bool {
        value.trim().starts_with("wireguard://")
    }

    /// Parses a `WireGuard` sharing URI without persisting its credential.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the URI is malformed, contains an invalid or
    /// unsupported field, or cannot be represented by the shared node model.
    pub fn parse(&self, value: &str) -> Result<ParsedWireGuardNode, WireGuardParseError> {
        let value = value.trim();
        if !self.can_parse(value) {
            return Err(WireGuardParseError::UnsupportedScheme);
        }
        validate_percent_encoding(value).map_err(invalid_uri)?;

        let url =
            Url::parse(value).map_err(|source| WireGuardParseError::MalformedUrl { source })?;
        if !matches!(url.path(), "" | "/") {
            return Err(WireGuardParseError::UnexpectedPath);
        }
        if url.password().is_some() {
            return Err(WireGuardParseError::UnexpectedPassword);
        }
        let server = parse_server(&url).map_err(invalid_uri)?;
        let port = parse_port_with_default(&url, DEFAULT_PORT).map_err(invalid_uri)?;

        let raw_private_key = url.username();
        if raw_private_key.is_empty() {
            return Err(WireGuardParseError::MissingPrivateKey);
        }
        let private_key = decode_component(raw_private_key).map_err(invalid_uri)?;

        let mut parameters = QueryParameters::parse(&url).map_err(invalid_uri)?;
        let peer_public_key = parameters
            .take_required_non_empty("publickey")
            .map_err(invalid_uri)?;
        let local_address =
            parse_local_address(parameters.take_required_non_empty("address").map_err(invalid_uri)?)?;
        let mtu = parse_mtu(&mut parameters)?;
        let pre_shared_key = parse_pre_shared_key(&mut parameters)?;
        let reserved = parse_reserved(&mut parameters)?;
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
            NodeName::new(name).map_err(|source| WireGuardParseError::InvalidNode { source })?;

        Ok(ParsedWireGuardNode {
            name,
            server,
            port,
            credential: WireGuardCredential {
                private_key,
                peer_public_key,
                pre_shared_key,
                local_address,
                mtu,
                reserved,
            },
        })
    }
}

/// The private key lives in the OS keyring, alongside every other secret this
/// model stores; nothing about it is safe to print.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct WireGuardCredential {
    pub(crate) private_key: String,
    pub(crate) peer_public_key: String,
    pub(crate) pre_shared_key: Option<String>,
    pub(crate) local_address: Vec<String>,
    pub(crate) mtu: Option<u32>,
    pub(crate) reserved: Option<[u8; 3]>,
}

impl WireGuardCredential {
    #[must_use]
    pub fn private_key(&self) -> &str {
        &self.private_key
    }

    #[must_use]
    pub fn peer_public_key(&self) -> &str {
        &self.peer_public_key
    }

    #[must_use]
    pub fn pre_shared_key(&self) -> Option<&str> {
        self.pre_shared_key.as_deref()
    }

    #[must_use]
    pub fn local_address(&self) -> &[String] {
        &self.local_address
    }

    #[must_use]
    pub const fn mtu(&self) -> Option<u32> {
        self.mtu
    }

    #[must_use]
    pub const fn reserved(&self) -> Option<[u8; 3]> {
        self.reserved
    }
}

impl Debug for WireGuardCredential {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("WireGuardCredential([REDACTED])")
    }
}

#[derive(Debug)]
pub struct ParsedWireGuardNode {
    name: NodeName,
    server: ServerAddress,
    port: NonZeroU16,
    credential: WireGuardCredential,
}

impl ParsedWireGuardNode {
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
    pub const fn credential(&self) -> &WireGuardCredential {
        &self.credential
    }

    /// `WireGuard` is its own tunnel; it never carries a stream transport.
    #[must_use]
    pub const fn transport(&self) -> Option<&TransportConfig> {
        None
    }

    /// `WireGuard` authenticates peers by key, not by certificate; it never
    /// carries a TLS layer.
    #[must_use]
    pub const fn tls(&self) -> Option<&magies_domain::TlsConfig> {
        None
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
        ProxyNode::new(
            id,
            self.name.as_str(),
            ProxyProtocol::WireGuard,
            self.server.as_str(),
            u32::from(self.port.get()),
            Some(credential_ref),
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WireGuardParseError {
    #[error("URI scheme must be exactly wireguard")]
    UnsupportedScheme,
    #[error("malformed WireGuard URI")]
    MalformedUrl { source: url::ParseError },
    #[error("WireGuard URI path must be empty or /")]
    UnexpectedPath,
    #[error("WireGuard URI private key must not carry a separate password")]
    UnexpectedPassword,
    #[error("invalid WireGuard URI field")]
    InvalidUri { source: VlessParseError },
    #[error("WireGuard private key is required")]
    MissingPrivateKey,
    #[error("WireGuard local address {value:?} contains an empty entry")]
    InvalidLocalAddress { value: String },
    #[error("WireGuard MTU {value:?} is not a valid number")]
    InvalidMtu { value: String },
    #[error("WireGuard reserved field {value:?} must be exactly 3 comma-separated bytes")]
    InvalidReserved { value: String },
    #[error("WireGuard link disagrees about its pre-shared key")]
    ConflictingPreSharedKey,
    #[error("invalid parsed WireGuard node")]
    InvalidNode { source: NodeModelError },
}

fn parse_local_address(value: String) -> Result<Vec<String>, WireGuardParseError> {
    let addresses: Vec<String> = value
        .split(',')
        .map(|entry| entry.trim().to_owned())
        .collect();
    if addresses.iter().any(String::is_empty) {
        return Err(WireGuardParseError::InvalidLocalAddress { value });
    }
    Ok(addresses)
}

fn parse_mtu(parameters: &mut QueryParameters) -> Result<Option<u32>, WireGuardParseError> {
    let Some(value) = parameters.take_non_empty("mtu").map_err(invalid_uri)? else {
        return Ok(None);
    };
    value
        .parse::<u32>()
        .map(Some)
        .map_err(|_| WireGuardParseError::InvalidMtu { value })
}

/// Reads the pre-shared key from either spelling the sharing links use.
///
/// A link carrying both spellings must agree, because keeping one and
/// discarding the other would silently pick a key the user did not choose.
fn parse_pre_shared_key(
    parameters: &mut QueryParameters,
) -> Result<Option<String>, WireGuardParseError> {
    let mut result: Option<String> = None;
    for name in ["presharedkey", "psk"] {
        let Some(value) = parameters.take_non_empty(name).map_err(invalid_uri)? else {
            continue;
        };
        match &result {
            Some(existing) if existing != &value => {
                return Err(WireGuardParseError::ConflictingPreSharedKey);
            }
            Some(_) => {}
            None => result = Some(value),
        }
    }
    Ok(result)
}

fn parse_reserved(
    parameters: &mut QueryParameters,
) -> Result<Option<[u8; 3]>, WireGuardParseError> {
    let Some(value) = parameters.take_non_empty("reserved").map_err(invalid_uri)? else {
        return Ok(None);
    };
    let parts: Vec<&str> = value.split(',').collect();
    let [first, second, third] = parts.as_slice() else {
        return Err(WireGuardParseError::InvalidReserved { value });
    };
    let parse_byte = |part: &str| part.trim().parse::<u8>();
    match (parse_byte(first), parse_byte(second), parse_byte(third)) {
        (Ok(first), Ok(second), Ok(third)) => Ok(Some([first, second, third])),
        _ => Err(WireGuardParseError::InvalidReserved { value }),
    }
}

fn invalid_uri(source: VlessParseError) -> WireGuardParseError {
    WireGuardParseError::InvalidUri { source }
}
