//! Shared domain models for `MgClash`.

use std::fmt::{Debug, Formatter};
use std::num::{NonZeroU16, NonZeroU32};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoreType {
    Xray,
    SingBox,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyProtocol {
    Vless,
    Vmess,
    Trojan,
    Shadowsocks,
    Hysteria2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct NodeName(String);

impl NodeName {
    /// Creates a non-empty node name and removes surrounding whitespace.
    ///
    /// # Errors
    ///
    /// Returns [`NodeModelError::EmptyName`] when the value contains no
    /// non-whitespace characters.
    pub fn new(value: impl Into<String>) -> Result<Self, NodeModelError> {
        let value = value.into();
        normalized_non_empty(&value, NodeModelError::EmptyName).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for NodeName {
    type Error = NodeModelError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<NodeName> for String {
    fn from(value: NodeName) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct ServerAddress(String);

impl ServerAddress {
    /// Creates a non-empty server address and removes surrounding whitespace.
    ///
    /// # Errors
    ///
    /// Returns [`NodeModelError::EmptyServer`] when the value contains no
    /// non-whitespace characters.
    pub fn new(value: impl Into<String>) -> Result<Self, NodeModelError> {
        let value = value.into();
        normalized_non_empty(&value, NodeModelError::EmptyServer).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ServerAddress {
    type Error = NodeModelError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ServerAddress> for String {
    fn from(value: ServerAddress) -> Self {
        value.0
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct CredentialRef(String);

impl CredentialRef {
    /// Creates a non-empty reference to credentials stored outside the model.
    ///
    /// # Errors
    ///
    /// Returns [`NodeModelError::EmptyCredentialReference`] when the value
    /// contains no non-whitespace characters.
    pub fn new(value: impl Into<String>) -> Result<Self, NodeModelError> {
        let value = value.into();
        normalized_non_empty(&value, NodeModelError::EmptyCredentialReference).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Debug for CredentialRef {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CredentialRef([REDACTED])")
    }
}

impl TryFrom<String> for CredentialRef {
    type Error = NodeModelError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<CredentialRef> for String {
    fn from(value: CredentialRef) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct SubscriptionName(String);

impl SubscriptionName {
    /// Creates a non-empty subscription name and removes surrounding whitespace.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriptionModelError::EmptyName`] when the value contains
    /// no non-whitespace characters.
    pub fn new(value: impl Into<String>) -> Result<Self, SubscriptionModelError> {
        let value = value.into();
        let normalized = value.trim();
        if normalized.is_empty() {
            Err(SubscriptionModelError::EmptyName)
        } else {
            Ok(Self(normalized.to_owned()))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SubscriptionName {
    type Error = SubscriptionModelError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SubscriptionName> for String {
    fn from(value: SubscriptionName) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: Uuid,
    pub name: SubscriptionName,
    pub url_secret_ref: CredentialRef,
    pub update_interval_minutes: NonZeroU32,
    pub auto_update: bool,
    pub last_updated_at: Option<TimestampMillis>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub enabled: bool,
}

impl Subscription {
    /// Creates a subscription with disabled automatic updates and no fetch metadata.
    ///
    /// # Errors
    ///
    /// Returns a typed validation error for an empty name or zero update interval.
    pub fn new(
        id: Uuid,
        name: impl Into<String>,
        url_secret_ref: CredentialRef,
        update_interval_minutes: u32,
    ) -> Result<Self, SubscriptionModelError> {
        let name = SubscriptionName::new(name)?;
        let update_interval_minutes = NonZeroU32::new(update_interval_minutes).ok_or(
            SubscriptionModelError::InvalidUpdateInterval {
                minutes: update_interval_minutes,
            },
        )?;
        Ok(Self {
            id,
            name,
            url_secret_ref,
            update_interval_minutes,
            auto_update: false,
            last_updated_at: None,
            etag: None,
            last_modified: None,
            enabled: true,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SubscriptionModelError {
    #[error("subscription name must not be empty")]
    EmptyName,
    #[error("subscription update interval must be greater than zero, got {minutes}")]
    InvalidUpdateInterval { minutes: u32 },
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum TransportConfig {
    #[serde(rename = "tcp")]
    Tcp,
    #[serde(rename = "websocket")]
    WebSocket { path: String, host: Option<String> },
    #[serde(rename = "grpc")]
    Grpc {
        #[serde(rename = "serviceName")]
        service_name: String,
        mode: GrpcMode,
        authority: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GrpcMode {
    Gun,
    Multi,
    Guna,
}

/// A server certificate's SHA-256 digest, as `pinSHA256` carries it.
///
/// Held as lower-case hex without separators: share links print the digest with
/// colons and in either case, and two spellings of one digest must compare
/// equal so node de-duplication is not fooled by formatting.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct CertificatePin(String);

impl CertificatePin {
    /// Parses a digest of exactly 32 bytes written as hex, colons optional.
    ///
    /// # Errors
    ///
    /// Returns [`NodeModelError::InvalidCertificatePin`] for anything that is
    /// not 32 hex-encoded bytes, including the Base64 spelling other tools use.
    pub fn new(value: &str) -> Result<Self, NodeModelError> {
        let digits: String = value.chars().filter(|value| *value != ':').collect();
        let is_hex = digits.len() == 64 && digits.chars().all(|value| value.is_ascii_hexdigit());
        if is_hex {
            Ok(Self(digits.to_ascii_lowercase()))
        } else {
            Err(NodeModelError::InvalidCertificatePin)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CertificatePin {
    type Error = NodeModelError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl From<CertificatePin> for String {
    fn from(value: CertificatePin) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum TlsConfig {
    #[serde(rename = "tls")]
    Tls {
        #[serde(rename = "serverName")]
        server_name: Option<String>,
        #[serde(rename = "allowInsecure")]
        allow_insecure: bool,
        alpn: Vec<String>,
        fingerprint: Option<String>,
        /// The server certificate's SHA-256, when the node pins one.
        ///
        /// Defaulted because nodes persisted before pinning existed carry no
        /// such field, and failing to read them would lose the user's list.
        #[serde(rename = "pinnedSha256", default)]
        pinned_sha256: Option<CertificatePin>,
    },
    #[serde(rename = "reality")]
    Reality {
        #[serde(rename = "serverName")]
        server_name: String,
        #[serde(rename = "publicKey")]
        public_key: String,
        #[serde(rename = "shortId")]
        short_id: Option<String>,
        fingerprint: Option<String>,
        alpn: Vec<String>,
        #[serde(rename = "spiderX")]
        spider_x: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct TimestampMillis(i64);

impl TimestampMillis {
    #[must_use]
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyNode {
    pub id: Uuid,
    pub name: NodeName,
    pub protocol_type: ProxyProtocol,
    pub server: ServerAddress,
    pub port: NonZeroU16,
    pub credential_ref: CredentialRef,
    pub transport: Option<TransportConfig>,
    pub tls: Option<TlsConfig>,
    pub udp_enabled: bool,
    pub subscription_id: Option<Uuid>,
    pub group_id: Option<Uuid>,
    pub latency_ms: Option<u32>,
    pub last_tested_at: Option<TimestampMillis>,
    pub enabled: bool,
}

impl ProxyNode {
    /// Creates a node with the PRD defaults for optional and runtime fields.
    ///
    /// # Errors
    ///
    /// Returns a typed validation error when the name or server is empty, the
    /// port is outside `1..=65535`, or the credential reference is missing.
    pub fn new(
        id: Uuid,
        name: impl Into<String>,
        protocol_type: ProxyProtocol,
        server: impl Into<String>,
        port: u32,
        credential_ref: Option<CredentialRef>,
    ) -> Result<Self, NodeModelError> {
        let name = NodeName::new(name)?;
        let server = ServerAddress::new(server)?;
        let port = u16::try_from(port)
            .ok()
            .and_then(NonZeroU16::new)
            .ok_or(NodeModelError::InvalidPort { port })?;
        let credential_ref = credential_ref.ok_or(NodeModelError::MissingCredentialReference)?;

        Ok(Self {
            id,
            name,
            protocol_type,
            server,
            port,
            credential_ref,
            transport: None,
            tls: None,
            udp_enabled: true,
            subscription_id: None,
            group_id: None,
            latency_ms: None,
            last_tested_at: None,
            enabled: true,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum NodeModelError {
    #[error("node name must not be empty")]
    EmptyName,
    #[error("node server must not be empty")]
    EmptyServer,
    #[error("node port must be between 1 and 65535, got {port}")]
    InvalidPort { port: u32 },
    #[error("node credential reference is required")]
    MissingCredentialReference,
    #[error("node credential reference must not be empty")]
    EmptyCredentialReference,
    #[error("certificate pin must be a SHA-256 digest written as 32 hex bytes")]
    InvalidCertificatePin,
}

fn normalized_non_empty(value: &str, error: NodeModelError) -> Result<String, NodeModelError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        Err(error)
    } else {
        Ok(normalized.to_owned())
    }
}
