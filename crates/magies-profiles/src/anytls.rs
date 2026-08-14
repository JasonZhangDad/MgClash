//! `AnyTLS` sharing links.
//!
//! `AnyTLS` is a sing-box-only outbound (added in sing-box 1.12.0, carried by the
//! pinned 1.13.18) with no stream transport of its own: the connection is TLS
//! from the first byte, so this parser always produces a TLS layer and never a
//! [`TransportConfig`]. The sharing URI mirrors the shape `anytls-go` and
//! v2rayN emit: the password lives in the URI's user information, and the rest
//! of the query string is the same TLS/Reality vocabulary VLESS already uses.

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
    parse_server, parse_tls_with_default, validate_percent_encoding,
};

/// Not IANA-assigned; matches the port every `anytls-go` example and v2rayN
/// itself default to when a link omits it.
const DEFAULT_PORT: u16 = 443;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AnyTlsParser;

impl AnyTlsParser {
    #[must_use]
    pub fn can_parse(&self, value: &str) -> bool {
        value.trim().starts_with("anytls://")
    }

    /// Parses a standard `AnyTLS` sharing URI without persisting its password.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the URI is malformed, omits its mandatory
    /// TLS layer, or uses an extension the shared node model cannot preserve.
    pub fn parse(&self, value: &str) -> Result<ParsedAnyTlsNode, AnyTlsParseError> {
        let value = value.trim();
        if !self.can_parse(value) {
            return Err(AnyTlsParseError::UnsupportedScheme);
        }
        validate_percent_encoding(value).map_err(invalid_uri)?;

        let url = Url::parse(value).map_err(|source| AnyTlsParseError::MalformedUrl { source })?;
        if !matches!(url.path(), "" | "/") {
            return Err(AnyTlsParseError::UnexpectedPath);
        }
        if url.password().is_some() {
            return Err(AnyTlsParseError::UnexpectedPasswordComponent);
        }

        let raw_password = url.username();
        if raw_password.is_empty() {
            return Err(AnyTlsParseError::MissingPassword);
        }
        let password = decode_component(raw_password).map_err(invalid_uri)?;
        let server = parse_server(&url).map_err(invalid_uri)?;
        let port = parse_port_with_default(&url, DEFAULT_PORT).map_err(invalid_uri)?;

        let mut parameters = QueryParameters::parse(&url).map_err(invalid_uri)?;
        let allow_insecure = parse_insecure_aliases(&mut parameters)?;
        // Official links default to TLS when `security` is omitted: unlike
        // VLESS/Trojan, AnyTLS has no plaintext mode to fall back to.
        let mut tls =
            parse_tls_with_default(&mut parameters, &server, "tls").map_err(invalid_uri)?;
        // sing-box 1.13.18 applies its own idle-session/padding-scheme
        // defaults; nothing in the shared model can carry a per-node override,
        // so a link asking for one is refused rather than silently ignored.
        parameters.finish().map_err(invalid_uri)?;
        apply_insecure(&mut tls, allow_insecure)?;
        let tls = tls.ok_or(AnyTlsParseError::MissingTls)?;

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
            NodeName::new(name).map_err(|source| AnyTlsParseError::InvalidNode { source })?;

        Ok(ParsedAnyTlsNode {
            name,
            server,
            port,
            credential: AnyTlsCredential { password },
            tls,
        })
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct AnyTlsCredential {
    pub(crate) password: String,
}

impl AnyTlsCredential {
    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }
}

impl Debug for AnyTlsCredential {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AnyTlsCredential([REDACTED])")
    }
}

#[derive(Debug)]
pub struct ParsedAnyTlsNode {
    name: NodeName,
    server: ServerAddress,
    port: NonZeroU16,
    credential: AnyTlsCredential,
    tls: TlsConfig,
}

impl ParsedAnyTlsNode {
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
    pub const fn credential(&self) -> &AnyTlsCredential {
        &self.credential
    }

    /// `AnyTLS` is TLS from the first byte; it never carries a stream transport.
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
            ProxyProtocol::AnyTls,
            self.server.as_str(),
            u32::from(self.port.get()),
            Some(credential_ref),
        )?;
        node.tls = Some(self.tls);
        Ok(node)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AnyTlsParseError {
    #[error("URI scheme must be exactly anytls")]
    UnsupportedScheme,
    #[error("malformed AnyTLS URI")]
    MalformedUrl { source: url::ParseError },
    #[error("AnyTLS URI path must be empty or /")]
    UnexpectedPath,
    #[error("AnyTLS URI password must be encoded as URI user information")]
    UnexpectedPasswordComponent,
    #[error("AnyTLS password is required")]
    MissingPassword,
    #[error("invalid AnyTLS URI field")]
    InvalidUri { source: VlessParseError },
    #[error("AnyTLS parameter {name} must be 0 or 1, got {value}")]
    InvalidBoolean { name: &'static str, value: String },
    #[error("AnyTLS insecure parameter aliases conflict")]
    ConflictingInsecureAliases,
    #[error("AnyTLS allowInsecure cannot be used with {security}")]
    UnsupportedInsecureSecurity { security: &'static str },
    #[error("AnyTLS requires TLS")]
    MissingTls,
    #[error("invalid parsed AnyTLS node")]
    InvalidNode { source: NodeModelError },
}

fn invalid_uri(source: VlessParseError) -> AnyTlsParseError {
    AnyTlsParseError::InvalidUri { source }
}

/// Reads `insecure`/`allowInsecure`, the same pair VLESS and Trojan accept.
///
/// A link carrying both spellings must agree, because keeping one and
/// discarding the other would silently pick a value the user did not choose.
fn parse_insecure_aliases(
    parameters: &mut QueryParameters,
) -> Result<Option<bool>, AnyTlsParseError> {
    let allow_insecure =
        parse_boolean(parameters.take("allowInsecure").as_deref(), "allowInsecure")?;
    let insecure = parse_boolean(parameters.take("insecure").as_deref(), "insecure")?;
    match (allow_insecure, insecure) {
        (Some(first), Some(second)) if first != second => {
            Err(AnyTlsParseError::ConflictingInsecureAliases)
        }
        (Some(value), _) | (_, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
}

fn parse_boolean(
    value: Option<&str>,
    name: &'static str,
) -> Result<Option<bool>, AnyTlsParseError> {
    match value {
        None => Ok(None),
        Some("0") => Ok(Some(false)),
        Some("1") => Ok(Some(true)),
        Some(value) => Err(AnyTlsParseError::InvalidBoolean {
            name,
            value: value.to_owned(),
        }),
    }
}

/// Applies an explicit `insecure`/`allowInsecure` override onto the TLS layer
/// [`parse_tls_with_default`] already produced.
///
/// [`parse_tls_with_default`] hard-codes `allow_insecure: false`, so the flag
/// has to be threaded through afterwards, the same way `trojan.rs` does it.
fn apply_insecure(
    tls: &mut Option<TlsConfig>,
    allow_insecure: Option<bool>,
) -> Result<(), AnyTlsParseError> {
    let Some(allow_insecure) = allow_insecure else {
        return Ok(());
    };
    match tls {
        Some(TlsConfig::Tls {
            allow_insecure: value,
            ..
        }) => {
            *value = allow_insecure;
            Ok(())
        }
        Some(TlsConfig::Reality { .. }) if allow_insecure => {
            Err(AnyTlsParseError::UnsupportedInsecureSecurity {
                security: "reality",
            })
        }
        None if allow_insecure => {
            Err(AnyTlsParseError::UnsupportedInsecureSecurity { security: "none" })
        }
        Some(TlsConfig::Reality { .. }) | None => Ok(()),
    }
}
