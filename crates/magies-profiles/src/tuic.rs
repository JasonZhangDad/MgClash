use std::fmt::{Debug, Formatter};
use std::num::NonZeroU16;

use magies_domain::{
    CertificatePin, CredentialRef, NodeModelError, NodeName, ProxyNode, ProxyProtocol,
    ServerAddress, TlsConfig, TransportConfig,
};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use super::{
    QueryParameters, VlessParseError, decode_component, default_name, parse_alpn, parse_server,
    validate_percent_encoding,
};

const DEFAULT_PORT: u16 = 443;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TuicParser;

impl TuicParser {
    #[must_use]
    pub fn can_parse(&self, value: &str) -> bool {
        value.trim().starts_with("tuic://")
    }

    /// Parses a standard TUIC sharing URI without persisting secrets.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the URI is malformed, contains an invalid
    /// field, or uses an extension the shared node model cannot preserve.
    pub fn parse(&self, value: &str) -> Result<ParsedTuicNode, TuicParseError> {
        let value = value.trim();
        if !self.can_parse(value) {
            return Err(TuicParseError::UnsupportedScheme);
        }
        validate_percent_encoding(value).map_err(invalid_uri)?;

        let url = Url::parse(value).map_err(|source| TuicParseError::MalformedUrl { source })?;
        if !matches!(url.path(), "" | "/") {
            return Err(TuicParseError::UnexpectedPath);
        }
        let server = parse_server(&url).map_err(invalid_uri)?;
        let port = parse_port(&url)?;
        let (uuid, password) = parse_authentication(&url)?;

        let mut parameters = QueryParameters::parse(&url).map_err(invalid_uri)?;
        reject_unrepresentable_parameters(&mut parameters)?;
        let server_name = parameters.take_non_empty("sni").map_err(invalid_uri)?;
        let allow_insecure = parse_insecure(&mut parameters)?;
        let alpn = parse_alpn(&mut parameters).map_err(invalid_uri)?;
        let fingerprint = parameters.take_non_empty("fp").map_err(invalid_uri)?;
        let pinned_sha256 = parse_certificate_pin(&mut parameters)?;
        let congestion_control = parse_congestion_control(&mut parameters)?;
        let udp_relay_mode = parse_udp_relay_mode(&mut parameters)?;
        let udp_over_stream = parse_udp_over_stream(&mut parameters)?;
        let zero_rtt_handshake = parse_zero_rtt_handshake(&mut parameters)?;
        parameters.finish().map_err(invalid_uri)?;

        if udp_relay_mode.is_some() && udp_over_stream {
            return Err(TuicParseError::ConflictingUdpRelaySettings);
        }

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
        let name = NodeName::new(name).map_err(|source| TuicParseError::InvalidNode { source })?;

        Ok(ParsedTuicNode {
            name,
            server,
            port,
            credential: TuicCredential {
                uuid,
                password,
                congestion_control,
                udp_relay_mode,
                udp_over_stream,
                zero_rtt_handshake,
            },
            tls: TlsConfig::Tls {
                server_name,
                allow_insecure,
                alpn,
                fingerprint,
                pinned_sha256,
            },
        })
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct TuicCredential {
    pub(crate) uuid: Uuid,
    pub(crate) password: Option<String>,
    pub(crate) congestion_control: Option<TuicCongestionControl>,
    pub(crate) udp_relay_mode: Option<TuicUdpRelayMode>,
    pub(crate) udp_over_stream: bool,
    pub(crate) zero_rtt_handshake: bool,
}

impl TuicCredential {
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.uuid
    }

    #[must_use]
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    #[must_use]
    pub const fn congestion_control(&self) -> Option<TuicCongestionControl> {
        self.congestion_control
    }

    #[must_use]
    pub const fn udp_relay_mode(&self) -> Option<TuicUdpRelayMode> {
        self.udp_relay_mode
    }

    #[must_use]
    pub const fn udp_over_stream(&self) -> bool {
        self.udp_over_stream
    }

    #[must_use]
    pub const fn zero_rtt_handshake(&self) -> bool {
        self.zero_rtt_handshake
    }
}

impl Debug for TuicCredential {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TuicCredential([REDACTED])")
    }
}

/// Matches sing-box's own spelling, so the outbound generator and the sharing
/// URI never need a separate translation table.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TuicCongestionControl {
    Cubic,
    NewReno,
    Bbr,
}

/// Matches sing-box's own spelling, so the outbound generator and the sharing
/// URI never need a separate translation table.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TuicUdpRelayMode {
    Native,
    Quic,
}

#[derive(Debug)]
pub struct ParsedTuicNode {
    name: NodeName,
    server: ServerAddress,
    port: NonZeroU16,
    credential: TuicCredential,
    tls: TlsConfig,
}

impl ParsedTuicNode {
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
    pub const fn credential(&self) -> &TuicCredential {
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
            ProxyProtocol::Tuic,
            self.server.as_str(),
            u32::from(self.port.get()),
            Some(credential_ref),
        )?;
        node.tls = Some(self.tls);
        Ok(node)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TuicParseError {
    #[error("URI scheme must be exactly tuic")]
    UnsupportedScheme,
    #[error("malformed TUIC URI")]
    MalformedUrl { source: url::ParseError },
    #[error("TUIC URI path must be empty or /")]
    UnexpectedPath,
    #[error("TUIC port must be between 1 and 65535, got {port}")]
    InvalidPort { port: u16 },
    #[error("invalid TUIC URI field")]
    InvalidUri { source: VlessParseError },
    #[error("TUIC UUID is required")]
    MissingUuid,
    #[error("invalid TUIC UUID")]
    InvalidUuid { source: uuid::Error },
    #[error("TUIC parameter {name} must be 0 or 1, got {value}")]
    InvalidBoolean { name: &'static str, value: String },
    #[error("unsupported TUIC congestion control: {value}")]
    UnsupportedCongestionControl { value: String },
    #[error("unsupported TUIC UDP relay mode: {value}")]
    UnsupportedUdpRelayMode { value: String },
    #[error("TUIC link disagrees about its {name} parameter")]
    ConflictingParameter { name: &'static str },
    #[error("TUIC cannot combine udp-relay-mode with udp-over-stream")]
    ConflictingUdpRelaySettings,
    #[error("certificate pin {value:?} is not a SHA-256 digest")]
    InvalidCertificatePin { value: String },
    #[error("TUIC link pins two different certificate digests")]
    ConflictingCertificatePins,
    #[error("TUIC ECH is not supported by the shared TLS model")]
    UnsupportedEch,
    #[error("invalid parsed TUIC node")]
    InvalidNode { source: NodeModelError },
}

fn parse_port(url: &Url) -> Result<NonZeroU16, TuicParseError> {
    let port = url.port().unwrap_or(DEFAULT_PORT);
    NonZeroU16::new(port).ok_or(TuicParseError::InvalidPort { port })
}

fn parse_authentication(url: &Url) -> Result<(Uuid, Option<String>), TuicParseError> {
    let raw_username = url.username();
    if raw_username.is_empty() {
        return Err(TuicParseError::MissingUuid);
    }
    let username = decode_component(raw_username).map_err(invalid_uri)?;
    let uuid =
        Uuid::parse_str(&username).map_err(|source| TuicParseError::InvalidUuid { source })?;
    let password = url
        .password()
        .map(decode_component)
        .transpose()
        .map_err(invalid_uri)?;
    Ok((uuid, password))
}

/// Reads one setting written under either of its accepted spellings.
///
/// A link carrying both spellings must agree, because keeping one and
/// discarding the other would silently pick a value the user did not choose.
fn take_aliased(
    parameters: &mut QueryParameters,
    names: &[&'static str],
) -> Result<Option<String>, TuicParseError> {
    let mut result: Option<String> = None;
    for name in names {
        let Some(value) = parameters.take_non_empty(name).map_err(invalid_uri)? else {
            continue;
        };
        match &result {
            Some(existing) if existing != &value => {
                return Err(TuicParseError::ConflictingParameter { name: names[0] });
            }
            Some(_) => {}
            None => result = Some(value),
        }
    }
    Ok(result)
}

fn parse_boolean_alias(
    parameters: &mut QueryParameters,
    names: &[&'static str],
    error_name: &'static str,
) -> Result<bool, TuicParseError> {
    match take_aliased(parameters, names)? {
        None => Ok(false),
        Some(value) if value == "0" => Ok(false),
        Some(value) if value == "1" => Ok(true),
        Some(value) => Err(TuicParseError::InvalidBoolean {
            name: error_name,
            value,
        }),
    }
}

fn parse_insecure(parameters: &mut QueryParameters) -> Result<bool, TuicParseError> {
    parse_boolean_alias(parameters, &["insecure", "allow_insecure"], "insecure")
}

fn parse_congestion_control(
    parameters: &mut QueryParameters,
) -> Result<Option<TuicCongestionControl>, TuicParseError> {
    let Some(value) = take_aliased(parameters, &["congestion_control", "congestion-control"])?
    else {
        return Ok(None);
    };
    match value.as_str() {
        "cubic" => Ok(Some(TuicCongestionControl::Cubic)),
        "new_reno" => Ok(Some(TuicCongestionControl::NewReno)),
        "bbr" => Ok(Some(TuicCongestionControl::Bbr)),
        _ => Err(TuicParseError::UnsupportedCongestionControl { value }),
    }
}

fn parse_udp_relay_mode(
    parameters: &mut QueryParameters,
) -> Result<Option<TuicUdpRelayMode>, TuicParseError> {
    let Some(value) = take_aliased(parameters, &["udp_relay_mode", "udp-relay-mode"])? else {
        return Ok(None);
    };
    match value.as_str() {
        "native" => Ok(Some(TuicUdpRelayMode::Native)),
        "quic" => Ok(Some(TuicUdpRelayMode::Quic)),
        _ => Err(TuicParseError::UnsupportedUdpRelayMode { value }),
    }
}

fn parse_udp_over_stream(parameters: &mut QueryParameters) -> Result<bool, TuicParseError> {
    parse_boolean_alias(
        parameters,
        &["udp_over_stream", "udp-over-stream"],
        "udp_over_stream",
    )
}

fn parse_zero_rtt_handshake(parameters: &mut QueryParameters) -> Result<bool, TuicParseError> {
    parse_boolean_alias(
        parameters,
        &["zero_rtt_handshake", "zero-rtt-handshake", "reduce_rtt"],
        "zero_rtt_handshake",
    )
}

fn reject_unrepresentable_parameters(
    parameters: &mut QueryParameters,
) -> Result<(), TuicParseError> {
    if parameters.take("ech").is_some() {
        return Err(TuicParseError::UnsupportedEch);
    }
    Ok(())
}

/// Reads the digest from either spelling of the pin parameter.
///
/// `pinSHA256` is the Hysteria2 spelling TUIC links borrow; `pcs` is the
/// abbreviation v2rayN writes. A link carrying both must agree, because
/// keeping one and discarding the other would pin against a digest the user
/// did not choose.
fn parse_certificate_pin(
    parameters: &mut QueryParameters,
) -> Result<Option<CertificatePin>, TuicParseError> {
    let mut pin = None;
    for name in ["pinSHA256", "pcs"] {
        let Some(value) = parameters.take(name) else {
            continue;
        };
        let parsed = CertificatePin::new(&value)
            .map_err(|_| TuicParseError::InvalidCertificatePin { value })?;
        if pin.get_or_insert(parsed.clone()) != &parsed {
            return Err(TuicParseError::ConflictingCertificatePins);
        }
    }
    Ok(pin)
}

fn invalid_uri(source: VlessParseError) -> TuicParseError {
    TuicParseError::InvalidUri { source }
}
