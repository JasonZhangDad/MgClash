//! HTTP/HTTPS proxy sharing links.
//!
//! Named `http_proxy` rather than `http` so it cannot be confused with the
//! desktop's local HTTP inbound. The link shape mirrors [`crate::SocksParser`]:
//! `http://user:pass@host:port` for a plaintext proxy, `https://` for one that
//! wraps the connection to the proxy itself in TLS.
//!
//! `can_parse` checks the URI path before claiming the scheme: an `https://`
//! subscription URL almost always carries a path, so requiring an empty or `/`
//! path keeps this parser from stealing links [`crate::SubscriptionContentParser`]
//! and the other importers still need to see.

use std::fmt::{Debug, Formatter};
use std::num::NonZeroU16;

use magies_domain::{
    CredentialRef, NodeModelError, NodeName, ProxyNode, ProxyProtocol, ServerAddress, TlsConfig,
    TransportConfig,
};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use super::{
    QueryParameters, VlessParseError, decode_component, default_name, parse_port_with_default,
    parse_server, validate_percent_encoding,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HttpProxyParser;

impl HttpProxyParser {
    /// Whether `value` is an HTTP proxy sharing URI this parser can read.
    ///
    /// Only claims URIs whose path is empty or `/`, so a subscription link —
    /// which almost always carries a path — is left for the subscription
    /// fetcher instead of being misread as a proxy node.
    #[must_use]
    pub fn can_parse(&self, value: &str) -> bool {
        let trimmed = value.trim();
        let has_scheme = trimmed.starts_with("http://") || trimmed.starts_with("https://");
        if !has_scheme {
            return false;
        }
        Url::parse(trimmed).is_ok_and(|url| matches!(url.path(), "" | "/"))
    }

    /// Parses an HTTP/HTTPS proxy sharing URI without persisting its credential.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the URI is malformed, contains unsupported
    /// fields, or cannot be represented by the shared node model.
    pub fn parse(&self, value: &str) -> Result<ParsedHttpProxyNode, HttpProxyParseError> {
        let value = value.trim();
        if !self.can_parse(value) {
            return Err(HttpProxyParseError::UnsupportedScheme);
        }
        validate_percent_encoding(value).map_err(invalid_uri)?;

        let url =
            Url::parse(value).map_err(|source| HttpProxyParseError::MalformedUrl { source })?;
        if !matches!(url.path(), "" | "/") {
            return Err(HttpProxyParseError::UnexpectedPath);
        }
        let is_tls = url.scheme() == "https";
        let server = parse_server(&url).map_err(invalid_uri)?;
        let default_port = if is_tls { 443 } else { 80 };
        let port = parse_port_with_default(&url, default_port).map_err(invalid_uri)?;

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
            return Err(HttpProxyParseError::PasswordRequiresUsername);
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
        let name =
            NodeName::new(name).map_err(|source| HttpProxyParseError::InvalidNode { source })?;

        let tls = is_tls.then(|| TlsConfig::Tls {
            server_name: Some(server.as_str().to_owned()),
            allow_insecure: false,
            alpn: Vec::new(),
            fingerprint: None,
            pinned_sha256: None,
        });

        Ok(ParsedHttpProxyNode {
            name,
            server,
            port,
            credential: HttpCredential { username, password },
            tls,
        })
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct HttpCredential {
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
}

impl HttpCredential {
    #[must_use]
    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    #[must_use]
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }
}

impl Debug for HttpCredential {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("HttpCredential([REDACTED])")
    }
}

#[derive(Debug)]
pub struct ParsedHttpProxyNode {
    name: NodeName,
    server: ServerAddress,
    port: NonZeroU16,
    credential: HttpCredential,
    tls: Option<TlsConfig>,
}

impl ParsedHttpProxyNode {
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
    pub const fn credential(&self) -> &HttpCredential {
        &self.credential
    }

    #[must_use]
    pub const fn transport(&self) -> &TransportConfig {
        &TransportConfig::Tcp
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
            ProxyProtocol::Http,
            self.server.as_str(),
            u32::from(self.port.get()),
            Some(credential_ref),
        )?;
        node.transport = Some(TransportConfig::Tcp);
        node.tls = self.tls;
        Ok(node)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HttpProxyParseError {
    #[error("URI scheme must be http or https")]
    UnsupportedScheme,
    #[error("malformed HTTP proxy URI")]
    MalformedUrl { source: url::ParseError },
    #[error("HTTP proxy URI path must be empty or /")]
    UnexpectedPath,
    #[error("invalid HTTP proxy URI field")]
    InvalidUri { source: VlessParseError },
    #[error("HTTP proxy password requires a username")]
    PasswordRequiresUsername,
    #[error("invalid parsed HTTP proxy node")]
    InvalidNode { source: NodeModelError },
}

fn invalid_uri(source: VlessParseError) -> HttpProxyParseError {
    HttpProxyParseError::InvalidUri { source }
}
