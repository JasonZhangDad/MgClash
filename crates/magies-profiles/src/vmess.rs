use std::fmt::{Debug, Formatter};
use std::num::NonZeroU16;

use base64::{Engine as _, engine::general_purpose};
use magies_domain::{
    CredentialRef, GrpcMode, NodeModelError, NodeName, ProxyNode, ProxyProtocol, ServerAddress,
    TlsConfig, TransportConfig,
};
use serde::Deserialize;
use uuid::Uuid;

use super::{ParsedVlessNode, VlessCredential, VlessParseError, VlessParser, default_name};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VmessParser;

impl VmessParser {
    #[must_use]
    pub fn can_parse(&self, value: &str) -> bool {
        value.trim().starts_with("vmess://")
    }

    /// Parses either a `VMess` AEAD URL or a v2rayN Base64-JSON sharing URI.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the sharing URI is malformed, unsupported,
    /// or cannot be represented by the shared node model.
    pub fn parse(&self, value: &str) -> Result<ParsedVmessNode, VmessParseError> {
        let value = value.trim();
        let payload = value
            .strip_prefix("vmess://")
            .ok_or(VmessParseError::UnsupportedScheme)?;
        if payload.is_empty() {
            return Err(VmessParseError::EmptyPayload);
        }

        if payload.contains('@') {
            parse_aead(payload)
        } else {
            parse_legacy(payload)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmessSecurity {
    Auto,
    Aes128Gcm,
    Chacha20Poly1305,
    None,
    Zero,
}

impl VmessSecurity {
    fn parse(value: &str, allow_legacy_zero: bool) -> Result<Self, VmessParseError> {
        match value {
            "auto" => Ok(Self::Auto),
            "aes-128-gcm" => Ok(Self::Aes128Gcm),
            "chacha20-poly1305" => Ok(Self::Chacha20Poly1305),
            "none" => Ok(Self::None),
            "zero" if allow_legacy_zero => Ok(Self::Zero),
            value => Err(VmessParseError::UnsupportedEncryption {
                value: value.to_owned(),
            }),
        }
    }
}

pub struct VmessCredential {
    user_id: Uuid,
    security: VmessSecurity,
    alter_id: u32,
}

impl VmessCredential {
    #[must_use]
    pub const fn user_id(&self) -> Uuid {
        self.user_id
    }

    #[must_use]
    pub const fn security(&self) -> VmessSecurity {
        self.security
    }

    #[must_use]
    pub const fn alter_id(&self) -> u32 {
        self.alter_id
    }
}

impl Debug for VmessCredential {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("VmessCredential([REDACTED])")
    }
}

#[derive(Debug)]
pub struct ParsedVmessNode {
    name: NodeName,
    server: ServerAddress,
    port: NonZeroU16,
    credential: VmessCredential,
    transport: TransportConfig,
    tls: Option<TlsConfig>,
}

impl ParsedVmessNode {
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
    pub const fn credential(&self) -> &VmessCredential {
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
            ProxyProtocol::Vmess,
            self.server.as_str(),
            u32::from(self.port.get()),
            Some(credential_ref),
        )?;
        node.transport = Some(self.transport);
        node.tls = self.tls;
        Ok(node)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VmessParseError {
    #[error("URI scheme must be exactly vmess")]
    UnsupportedScheme,
    #[error("VMess URI payload is required")]
    EmptyPayload,
    #[error("invalid VMess AEAD URI")]
    InvalidAeadUri { source: VlessParseError },
    #[error("invalid VMess Base64 payload")]
    InvalidBase64 { source: base64::DecodeError },
    #[error("invalid VMess JSON payload")]
    InvalidJson { source: serde_json::Error },
    #[error("VMess field {name} is required")]
    MissingField { name: &'static str },
    #[error("invalid VMess user ID")]
    InvalidUserId { source: uuid::Error },
    #[error("VMess field {name} must be an unsigned integer")]
    InvalidNumber { name: &'static str },
    #[error("VMess port must be between 1 and 65535, got {port}")]
    InvalidPort { port: u64 },
    #[error("unsupported legacy VMess version: {value}")]
    UnsupportedLegacyVersion { value: String },
    #[error("unsupported VMess encryption: {value}")]
    UnsupportedEncryption { value: String },
    #[error("unsupported VMess transport: {value}")]
    UnsupportedTransport { value: String },
    #[error("unsupported VMess TCP header: {value}")]
    UnsupportedTcpHeader { value: String },
    #[error("unsupported VMess transport security: {value}")]
    UnsupportedSecurity { value: String },
    #[error("invalid VMess ALPN list")]
    InvalidAlpn,
    #[error("VMess field {name} cannot be represented by the node model")]
    UnsupportedField { name: &'static str },
    #[error("unsupported VMess parameter: {name}")]
    UnsupportedParameter { name: String },
    #[error("invalid parsed VMess node")]
    InvalidNode { source: NodeModelError },
}

fn parse_aead(payload: &str) -> Result<ParsedVmessNode, VmessParseError> {
    let uri = format!("vless://{payload}");
    let parsed = VlessParser
        .parse_with_default_encryption(&uri, "auto")
        .map_err(|source| VmessParseError::InvalidAeadUri { source })?;
    let ParsedVlessNode {
        name,
        server,
        port,
        credential,
        transport,
        tls,
    } = parsed;
    let VlessCredential {
        user_id,
        encryption,
        flow,
    } = credential;
    if flow.is_some() {
        return Err(VmessParseError::UnsupportedParameter {
            name: "flow".to_owned(),
        });
    }

    Ok(ParsedVmessNode {
        name,
        server,
        port,
        credential: VmessCredential {
            user_id,
            security: VmessSecurity::parse(&encryption, false)?,
            alter_id: 0,
        },
        transport,
        tls,
    })
}

fn parse_legacy(payload: &str) -> Result<ParsedVmessNode, VmessParseError> {
    let decoded = decode_base64(payload)?;
    let value: LegacyVmess = serde_json::from_slice(&decoded)
        .map_err(|source| VmessParseError::InvalidJson { source })?;
    value.try_into()
}

fn decode_base64(payload: &str) -> Result<Vec<u8>, VmessParseError> {
    let mut padded = payload.to_owned();
    let missing_padding = (4 - padded.len() % 4) % 4;
    padded.extend(std::iter::repeat_n('=', missing_padding));

    match general_purpose::STANDARD.decode(&padded) {
        Ok(decoded) => Ok(decoded),
        Err(_) => general_purpose::URL_SAFE
            .decode(&padded)
            .map_err(|source| VmessParseError::InvalidBase64 { source }),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyVmess {
    v: Option<TextOrNumber>,
    ps: Option<String>,
    add: Option<String>,
    port: Option<TextOrNumber>,
    id: Option<String>,
    aid: Option<TextOrNumber>,
    scy: Option<String>,
    net: Option<String>,
    #[serde(rename = "type")]
    header_type: Option<String>,
    host: Option<String>,
    path: Option<String>,
    tls: Option<String>,
    sni: Option<String>,
    alpn: Option<String>,
    fp: Option<String>,
    insecure: Option<TextOrNumber>,
    vcn: Option<String>,
    pcs: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TextOrNumber {
    Text(String),
    Number(u64),
}

impl TextOrNumber {
    fn as_text(&self) -> String {
        match self {
            Self::Text(value) => value.clone(),
            Self::Number(value) => value.to_string(),
        }
    }

    fn parse_u64(&self, name: &'static str) -> Result<u64, VmessParseError> {
        match self {
            Self::Text(value) => value
                .parse()
                .map_err(|_| VmessParseError::InvalidNumber { name }),
            Self::Number(value) => Ok(*value),
        }
    }
}

impl TryFrom<LegacyVmess> for ParsedVmessNode {
    type Error = VmessParseError;

    fn try_from(value: LegacyVmess) -> Result<Self, Self::Error> {
        validate_version(value.v.as_ref())?;
        reject_non_empty(value.vcn.as_deref(), "vcn")?;
        reject_non_empty(value.pcs.as_deref(), "pcs")?;

        let server = required_non_empty(value.add, "add")?;
        let server =
            ServerAddress::new(server).map_err(|source| VmessParseError::InvalidNode { source })?;
        let port = value
            .port
            .as_ref()
            .ok_or(VmessParseError::MissingField { name: "port" })?
            .parse_u64("port")?;
        let port = u16::try_from(port)
            .ok()
            .and_then(NonZeroU16::new)
            .ok_or(VmessParseError::InvalidPort { port })?;

        let raw_user_id = required_non_empty(value.id, "id")?;
        let user_id = Uuid::parse_str(&raw_user_id)
            .map_err(|source| VmessParseError::InvalidUserId { source })?;
        let alter_id = match value.aid.as_ref() {
            Some(value) => u32::try_from(value.parse_u64("aid")?)
                .map_err(|_| VmessParseError::InvalidNumber { name: "aid" })?,
            None => 0,
        };
        let security = VmessSecurity::parse(value.scy.as_deref().unwrap_or("auto"), true)?;
        let transport = parse_legacy_transport(
            value.net.as_deref(),
            value.header_type.as_deref(),
            value.host,
            value.path,
        )?;
        let tls = parse_legacy_tls(
            value.tls.as_deref(),
            value.sni,
            value.alpn.as_deref(),
            value.fp,
            value.insecure.as_ref(),
        )?;

        let fallback_name = default_name(&server, port);
        let name = value
            .ps
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(fallback_name);
        let name = NodeName::new(name).map_err(|source| VmessParseError::InvalidNode { source })?;

        Ok(Self {
            name,
            server,
            port,
            credential: VmessCredential {
                user_id,
                security,
                alter_id,
            },
            transport,
            tls,
        })
    }
}

fn validate_version(version: Option<&TextOrNumber>) -> Result<(), VmessParseError> {
    let Some(version) = version else {
        return Ok(());
    };
    let value = version.as_text();
    if value == "2" {
        Ok(())
    } else {
        Err(VmessParseError::UnsupportedLegacyVersion { value })
    }
}

fn required_non_empty(
    value: Option<String>,
    name: &'static str,
) -> Result<String, VmessParseError> {
    match value {
        Some(value) if !value.trim().is_empty() => Ok(value),
        _ => Err(VmessParseError::MissingField { name }),
    }
}

fn reject_non_empty(value: Option<&str>, name: &'static str) -> Result<(), VmessParseError> {
    if value.is_some_and(|value| !value.is_empty()) {
        Err(VmessParseError::UnsupportedField { name })
    } else {
        Ok(())
    }
}

fn parse_legacy_transport(
    network: Option<&str>,
    header_type: Option<&str>,
    host: Option<String>,
    path: Option<String>,
) -> Result<TransportConfig, VmessParseError> {
    let network = network.filter(|value| !value.is_empty()).unwrap_or("tcp");
    let header_type = header_type
        .filter(|value| !value.is_empty())
        .unwrap_or("none");
    if header_type != "none" {
        return Err(VmessParseError::UnsupportedTcpHeader {
            value: header_type.to_owned(),
        });
    }

    match network {
        "tcp" => Ok(TransportConfig::Tcp),
        "ws" => Ok(TransportConfig::WebSocket {
            path: path
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "/".to_owned()),
            host: host.filter(|value| !value.is_empty()),
        }),
        "grpc" => Ok(TransportConfig::Grpc {
            service_name: required_non_empty(path, "path")?,
            mode: GrpcMode::Gun,
            authority: None,
        }),
        value => Err(VmessParseError::UnsupportedTransport {
            value: value.to_owned(),
        }),
    }
}

fn parse_legacy_tls(
    security: Option<&str>,
    server_name: Option<String>,
    alpn: Option<&str>,
    fingerprint: Option<String>,
    insecure: Option<&TextOrNumber>,
) -> Result<Option<TlsConfig>, VmessParseError> {
    match security.filter(|value| !value.is_empty()).unwrap_or("none") {
        "none" => Ok(None),
        "tls" => Ok(Some(TlsConfig::Tls {
            server_name: server_name.filter(|value| !value.is_empty()),
            allow_insecure: parse_legacy_boolean(insecure, "insecure")?,
            alpn: parse_legacy_alpn(alpn)?,
            fingerprint: fingerprint.filter(|value| !value.is_empty()),
        })),
        value => Err(VmessParseError::UnsupportedSecurity {
            value: value.to_owned(),
        }),
    }
}

fn parse_legacy_boolean(
    value: Option<&TextOrNumber>,
    name: &'static str,
) -> Result<bool, VmessParseError> {
    match value.map(|value| value.parse_u64(name)).transpose()? {
        None | Some(0) => Ok(false),
        Some(1) => Ok(true),
        Some(_) => Err(VmessParseError::InvalidNumber { name }),
    }
}

fn parse_legacy_alpn(value: Option<&str>) -> Result<Vec<String>, VmessParseError> {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return Ok(Vec::new());
    };
    let values: Vec<_> = value.split(',').map(str::to_owned).collect();
    if values
        .iter()
        .any(|value| value.is_empty() || value.trim() != value)
    {
        Err(VmessParseError::InvalidAlpn)
    } else {
        Ok(values)
    }
}
