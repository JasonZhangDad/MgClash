use std::fmt::{Debug, Formatter};

use magies_domain::ProxyProtocol;
use magies_storage::{SecretStoreError, SecretValue};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Hysteria2Credential, NodeCredential, ShadowsocksCredential, TrojanCredential, TuicCredential,
    VlessCredential, VmessCredential,
};

const CREDENTIAL_PAYLOAD_VERSION: u8 = 1;

/// Owned node credential stored as one versioned secret payload.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "protocol", content = "value", rename_all = "lowercase")]
pub enum StoredNodeCredential {
    Vless(VlessCredential),
    Vmess(VmessCredential),
    Trojan(TrojanCredential),
    Shadowsocks(ShadowsocksCredential),
    Hysteria2(Hysteria2Credential),
    Tuic(TuicCredential),
}

impl StoredNodeCredential {
    #[must_use]
    pub const fn protocol(&self) -> ProxyProtocol {
        match self {
            Self::Vless(_) => ProxyProtocol::Vless,
            Self::Vmess(_) => ProxyProtocol::Vmess,
            Self::Trojan(_) => ProxyProtocol::Trojan,
            Self::Shadowsocks(_) => ProxyProtocol::Shadowsocks,
            Self::Hysteria2(_) => ProxyProtocol::Hysteria2,
            Self::Tuic(_) => ProxyProtocol::Tuic,
        }
    }

    #[must_use]
    pub const fn as_node_credential(&self) -> NodeCredential<'_> {
        match self {
            Self::Vless(value) => NodeCredential::Vless(value),
            Self::Vmess(value) => NodeCredential::Vmess(value),
            Self::Trojan(value) => NodeCredential::Trojan(value),
            Self::Shadowsocks(value) => NodeCredential::Shadowsocks(value),
            Self::Hysteria2(value) => NodeCredential::Hysteria2(value),
            Self::Tuic(value) => NodeCredential::Tuic(value),
        }
    }
}

impl Debug for StoredNodeCredential {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("StoredNodeCredential([REDACTED])")
    }
}

macro_rules! impl_from_credential {
    ($credential:ty, $variant:ident) => {
        impl From<&$credential> for StoredNodeCredential {
            fn from(value: &$credential) -> Self {
                Self::$variant(value.clone())
            }
        }
    };
}

impl_from_credential!(VlessCredential, Vless);
impl_from_credential!(VmessCredential, Vmess);
impl_from_credential!(TrojanCredential, Trojan);
impl_from_credential!(ShadowsocksCredential, Shadowsocks);
impl_from_credential!(Hysteria2Credential, Hysteria2);
impl_from_credential!(TuicCredential, Tuic);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CredentialCodec;

impl CredentialCodec {
    /// Encodes an owned credential into a zeroing secret payload.
    ///
    /// # Errors
    ///
    /// Returns a typed serialization or empty-secret error.
    pub fn encode(credential: &StoredNodeCredential) -> Result<SecretValue, CredentialCodecError> {
        let envelope = CredentialEnvelopeRef {
            version: CREDENTIAL_PAYLOAD_VERSION,
            credential,
        };
        let payload = serde_json::to_vec(&envelope)
            .map_err(|source| CredentialCodecError::SerializationFailed { source })?;
        SecretValue::new(payload).map_err(CredentialCodecError::SecretValue)
    }

    /// Decodes a versioned secret payload into an owned credential.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed data or unsupported versions.
    pub fn decode(payload: &SecretValue) -> Result<StoredNodeCredential, CredentialCodecError> {
        let envelope: CredentialEnvelope = serde_json::from_slice(payload.expose_secret())
            .map_err(|source| CredentialCodecError::InvalidPayload { source })?;
        if envelope.version != CREDENTIAL_PAYLOAD_VERSION {
            return Err(CredentialCodecError::UnsupportedVersion {
                version: envelope.version,
            });
        }
        Ok(envelope.credential)
    }
}

#[derive(Serialize)]
struct CredentialEnvelopeRef<'a> {
    version: u8,
    credential: &'a StoredNodeCredential,
}

#[derive(Deserialize)]
struct CredentialEnvelope {
    version: u8,
    credential: StoredNodeCredential,
}

#[derive(Debug, Error)]
pub enum CredentialCodecError {
    #[error("node credential serialization failed")]
    SerializationFailed {
        #[source]
        source: serde_json::Error,
    },
    #[error("node credential payload is invalid")]
    InvalidPayload {
        #[source]
        source: serde_json::Error,
    },
    #[error("node credential payload version {version} is unsupported")]
    UnsupportedVersion { version: u8 },
    #[error("node credential secret value is invalid")]
    SecretValue(#[source] SecretStoreError),
}
