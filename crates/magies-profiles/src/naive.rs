//! `NaiveProxy` sharing links and credentials.
//!
//! Naive has been available in sing-box since 1.13.0 (the pinned Core is
//! 1.13.18). Xray ships no `naive` outbound, so this module stays
//! sing-box-only. Official sing-box builds need platform libcronet on some
//! targets; `MgClash` still generates the outbound the same way v2rayN does.
//!
//! Sharing schemes follow the informal shape v2rayN-family clients emit:
//! `naive://`, `naive+https://` (HTTP/2), and `naive+quic://` (QUIC).

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

/// Naive's well-known default when a sharing link omits the port.
const DEFAULT_PORT: u16 = 443;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NaiveParser;

impl NaiveParser {
    /// Whether `value` is a Naive sharing URI this parser can read.
    #[must_use]
    pub fn can_parse(&self, value: &str) -> bool {
        let trimmed = value.trim();
        trimmed.starts_with("naive://")
            || trimmed.starts_with("naive+https://")
            || trimmed.starts_with("naive+quic://")
    }

    /// Parses a Naive sharing URI without persisting its credential.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the URI is malformed, carries an extension
    /// the shared node model cannot preserve, or uses TLS options sing-box's
    /// Naive outbound rejects (everything beyond `server_name`).
    pub fn parse(&self, value: &str) -> Result<ParsedNaiveNode, NaiveParseError> {
        let value = value.trim();
        if !self.can_parse(value) {
            return Err(NaiveParseError::UnsupportedScheme);
        }
        validate_percent_encoding(value).map_err(invalid_uri)?;

        let (normalized, quic) = normalize_scheme(value);
        // `url` does not accept `naive+https` / `naive+quic` as schemes, so the
        // link is rewritten to plain `naive://` before parsing; `quic` carries
        // the meaning the compound scheme encoded.
        let url = Url::parse(&normalized).map_err(|source| NaiveParseError::MalformedUrl { source })?;
        if !matches!(url.path(), "" | "/") {
            return Err(NaiveParseError::UnexpectedPath);
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
            return Err(NaiveParseError::PasswordRequiresUsername);
        }

        let mut parameters = QueryParameters::parse(&url).map_err(invalid_uri)?;
        reject_unrepresentable_parameters(&mut parameters)?;
        let server_name = parameters.take_non_empty("sni").map_err(invalid_uri)?;
        let quic_congestion_control = parse_congestion_control(&mut parameters)?;
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
        let name = NodeName::new(name).map_err(|source| NaiveParseError::InvalidNode { source })?;

        Ok(ParsedNaiveNode {
            name,
            server,
            port,
            credential: NaiveCredential {
                username,
                password,
                quic,
                quic_congestion_control,
            },
            // sing-box Naive TLS accepts only `server_name` (plus certificate /
            // ECH paths this model does not expose). Everything else is refused
            // above rather than silently dropped.
            tls: TlsConfig::Tls {
                server_name,
                allow_insecure: false,
                alpn: Vec::new(),
                fingerprint: None,
                pinned_sha256: None,
            },
        })
    }
}

/// Rewrites compound schemes the `url` crate cannot parse into plain `naive://`,
/// returning whether the original scheme selected QUIC.
fn normalize_scheme(value: &str) -> (String, bool) {
    if let Some(rest) = value.strip_prefix("naive+quic://") {
        (format!("naive://{rest}"), true)
    } else if let Some(rest) = value.strip_prefix("naive+https://") {
        (format!("naive://{rest}"), false)
    } else {
        (value.to_owned(), false)
    }
}

/// Parameters that would change traffic shape or that sing-box Naive cannot
/// represent in the shared TLS model — refuse rather than drop.
fn reject_unrepresentable_parameters(
    parameters: &mut QueryParameters,
) -> Result<(), NaiveParseError> {
    const REJECTED: &[&str] = &[
        "security",
        "s",
        "fp",
        "alpn",
        "insecure",
        "allowInsecure",
        "allow_insecure",
        "pinSHA256",
        "pbk",
        "sid",
        "spx",
        "insecure_concurrency",
        "udp_over_tcp",
        "extra_headers",
    ];
    for name in REJECTED {
        if parameters.take(name).is_some() {
            return Err(NaiveParseError::UnsupportedParameter { name });
        }
    }
    Ok(())
}

fn parse_congestion_control(
    parameters: &mut QueryParameters,
) -> Result<Option<NaiveCongestionControl>, NaiveParseError> {
    let Some(value) = take_aliased(
        parameters,
        &["congestion_control", "quic_congestion_control", "cc"],
    )?
    else {
        return Ok(None);
    };
    match value.as_str() {
        "bbr" => Ok(Some(NaiveCongestionControl::Bbr)),
        "bbr2" => Ok(Some(NaiveCongestionControl::Bbr2)),
        "cubic" => Ok(Some(NaiveCongestionControl::Cubic)),
        "reno" => Ok(Some(NaiveCongestionControl::Reno)),
        _ => Err(NaiveParseError::UnsupportedCongestionControl { value }),
    }
}

/// Reads one setting written under any of its accepted spellings.
///
/// A link carrying two spellings must agree, because keeping one and
/// discarding the other would silently pick a value the user did not choose.
fn take_aliased(
    parameters: &mut QueryParameters,
    names: &[&'static str],
) -> Result<Option<String>, NaiveParseError> {
    let mut result: Option<String> = None;
    for name in names {
        let Some(value) = parameters.take_non_empty(name).map_err(invalid_uri)? else {
            continue;
        };
        match &result {
            Some(existing) if existing != &value => {
                return Err(NaiveParseError::ConflictingParameter { name: names[0] });
            }
            Some(_) => {}
            None => result = Some(value),
        }
    }
    Ok(result)
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct NaiveCredential {
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
    #[serde(default)]
    pub(crate) quic: bool,
    #[serde(default)]
    pub(crate) quic_congestion_control: Option<NaiveCongestionControl>,
}

impl NaiveCredential {
    #[must_use]
    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    #[must_use]
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    #[must_use]
    pub const fn quic(&self) -> bool {
        self.quic
    }

    #[must_use]
    pub const fn quic_congestion_control(&self) -> Option<NaiveCongestionControl> {
        self.quic_congestion_control
    }
}

impl Debug for NaiveCredential {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("NaiveCredential([REDACTED])")
    }
}

/// Matches sing-box's own spelling for `quic_congestion_control`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NaiveCongestionControl {
    Bbr,
    Bbr2,
    Cubic,
    Reno,
}

impl NaiveCongestionControl {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bbr => "bbr",
            Self::Bbr2 => "bbr2",
            Self::Cubic => "cubic",
            Self::Reno => "reno",
        }
    }
}

#[derive(Debug)]
pub struct ParsedNaiveNode {
    name: NodeName,
    server: ServerAddress,
    port: NonZeroU16,
    credential: NaiveCredential,
    tls: TlsConfig,
}

impl ParsedNaiveNode {
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
    pub const fn credential(&self) -> &NaiveCredential {
        &self.credential
    }

    /// `Naive` tunnels over HTTP/2 or QUIC; it never carries a stream transport.
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
            ProxyProtocol::Naive,
            self.server.as_str(),
            u32::from(self.port.get()),
            Some(credential_ref),
        )?;
        node.tls = Some(self.tls);
        Ok(node)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NaiveParseError {
    #[error("URI scheme must be naive, naive+https, or naive+quic")]
    UnsupportedScheme,
    #[error("malformed Naive URI")]
    MalformedUrl { source: url::ParseError },
    #[error("Naive URI path must be empty or /")]
    UnexpectedPath,
    #[error("Naive password requires a username")]
    PasswordRequiresUsername,
    #[error("invalid Naive URI field")]
    InvalidUri { source: VlessParseError },
    #[error("Naive parameter {name} is unsupported")]
    UnsupportedParameter { name: &'static str },
    #[error("Naive congestion control {value} is unsupported")]
    UnsupportedCongestionControl { value: String },
    #[error("Naive parameter {name} aliases conflict")]
    ConflictingParameter { name: &'static str },
    #[error("invalid parsed Naive node")]
    InvalidNode { source: NodeModelError },
}

fn invalid_uri(source: VlessParseError) -> NaiveParseError {
    NaiveParseError::InvalidUri { source }
}
