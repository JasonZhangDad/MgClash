//! SOCKS5 sharing links.
//!
//! There is no widely used SOCKS sharing-link standard the way there is for
//! VLESS or Shadowsocks; this follows the informal `socks://user:pass@host:port`
//! shape that v2rayN-family clients already emit, with `socks5://` and
//! `socks5h://` accepted as the same thing sing-box and Xray both expose as one
//! `socks` outbound. `socks5h` only signals remote DNS resolution at the SOCKS
//! layer, which this model does not represent separately, so it parses
//! identically to `socks5`.

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

/// sing-box's and Xray's shared default when a SOCKS link omits the port.
const DEFAULT_PORT: u16 = 1080;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SocksParser;

impl SocksParser {
    /// Whether `value` is a SOCKS sharing URI this parser can read.
    ///
    /// Checks the path up front so a subscription URL that merely happens to
    /// use one of these schemes (unlikely, but the pattern matches
    /// [`crate::HttpProxyParser`]) is never mistaken for a node link.
    #[must_use]
    pub fn can_parse(&self, value: &str) -> bool {
        let trimmed = value.trim();
        let has_scheme = trimmed.starts_with("socks://")
            || trimmed.starts_with("socks5://")
            || trimmed.starts_with("socks5h://");
        if !has_scheme {
            return false;
        }
        Url::parse(trimmed).is_ok_and(|url| matches!(url.path(), "" | "/"))
    }

    /// Parses a SOCKS5 sharing URI without persisting its credential.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the URI is malformed, contains unsupported
    /// fields, or cannot be represented by the shared node model.
    pub fn parse(&self, value: &str) -> Result<ParsedSocksNode, SocksParseError> {
        let value = value.trim();
        if !self.can_parse(value) {
            return Err(SocksParseError::UnsupportedScheme);
        }
        validate_percent_encoding(value).map_err(invalid_uri)?;

        let url = Url::parse(value).map_err(|source| SocksParseError::MalformedUrl { source })?;
        if !matches!(url.path(), "" | "/") {
            return Err(SocksParseError::UnexpectedPath);
        }
        let server = parse_server(&url).map_err(invalid_uri)?;
        let port = parse_port_with_default(&url, DEFAULT_PORT).map_err(invalid_uri)?;

        let username = if url.username().is_empty() {
            None
        } else {
            Some(decode_component(url.username()).map_err(invalid_uri)?)
        };
        let password = url
            .password()
            .map(decode_component)
            .transpose()
            .map_err(invalid_uri)?;
        if username.is_none() && password.is_some() {
            return Err(SocksParseError::PasswordRequiresUsername);
        }

        let parameters = QueryParameters::parse(&url).map_err(invalid_uri)?;
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
        let name = NodeName::new(name).map_err(|source| SocksParseError::InvalidNode { source })?;

        Ok(ParsedSocksNode {
            name,
            server,
            port,
            credential: SocksCredential { username, password },
        })
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct SocksCredential {
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
}

impl SocksCredential {
    #[must_use]
    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    #[must_use]
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }
}

impl Debug for SocksCredential {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SocksCredential([REDACTED])")
    }
}

#[derive(Debug)]
pub struct ParsedSocksNode {
    name: NodeName,
    server: ServerAddress,
    port: NonZeroU16,
    credential: SocksCredential,
}

impl ParsedSocksNode {
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
    pub const fn credential(&self) -> &SocksCredential {
        &self.credential
    }

    #[must_use]
    pub const fn transport(&self) -> &TransportConfig {
        &TransportConfig::Tcp
    }

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
        let mut node = ProxyNode::new(
            id,
            self.name.as_str(),
            ProxyProtocol::Socks,
            self.server.as_str(),
            u32::from(self.port.get()),
            Some(credential_ref),
        )?;
        node.transport = Some(TransportConfig::Tcp);
        Ok(node)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SocksParseError {
    #[error("URI scheme must be socks, socks5, or socks5h")]
    UnsupportedScheme,
    #[error("malformed SOCKS URI")]
    MalformedUrl { source: url::ParseError },
    #[error("SOCKS URI path must be empty or /")]
    UnexpectedPath,
    #[error("invalid SOCKS URI field")]
    InvalidUri { source: VlessParseError },
    #[error("SOCKS password requires a username")]
    PasswordRequiresUsername,
    #[error("invalid parsed SOCKS node")]
    InvalidNode { source: NodeModelError },
}

fn invalid_uri(source: VlessParseError) -> SocksParseError {
    SocksParseError::InvalidUri { source }
}
