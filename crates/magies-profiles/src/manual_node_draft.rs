//! Typed form input for creating a node without a share link.
//!
//! [`ManualNodeDraft`] is the counterpart to [`crate::ShareLinkParser`]: both
//! produce the same `(ProxyNode, StoredNodeCredential)` pair, one from a URI and
//! one from user-entered fields, so a manually created node is indistinguishable
//! from an imported one downstream.
//!
//! The draft rejects up front every combination that
//! [`crate::SingBoxOutboundConfigGenerator`] would later refuse, so a node the
//! form accepts is a node that can actually connect. Fields the generator can
//! never honour — Trojan `flow`, VLESS `encryption`, Hysteria2 packet sizes —
//! are not offered at all.

use std::fmt::{Debug, Formatter};

use magies_domain::{CredentialRef, NodeModelError, ProxyNode, TlsConfig, TransportConfig};
use serde::Deserialize;
use thiserror::Error;
use uuid::Uuid;

use crate::hysteria2::{Hysteria2Credential, Hysteria2Obfuscation, Hysteria2ObfuscationMethod};
use crate::shadowsocks::{SUPPORTED_METHODS, ShadowsocksCredential};
use crate::trojan::TrojanCredential;
use crate::vmess::{VmessCredential, VmessSecurity};
use crate::{StoredNodeCredential, VlessCredential};

/// VLESS negotiates encryption at the TLS layer; the outbound generator accepts
/// no other value.
const VLESS_ENCRYPTION: &str = "none";

/// One node as entered in the manual creation form.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualNodeDraft {
    pub name: String,
    pub server: String,
    pub port: u32,
    #[serde(default = "enabled_by_default")]
    pub udp_enabled: bool,
    /// Defaults to [`TransportConfig::Tcp`] for the stream protocols and must be
    /// absent for Hysteria2, which carries its own QUIC transport.
    #[serde(default)]
    pub transport: Option<TransportConfig>,
    #[serde(default)]
    pub tls: Option<TlsConfig>,
    pub credential: ManualCredentialDraft,
}

impl ManualNodeDraft {
    /// Validates the draft and splits it into a node and its credential.
    ///
    /// The protocol is taken from the credential variant rather than a separate
    /// field, so the two can never disagree.
    ///
    /// # Errors
    ///
    /// Returns a typed error when a shared field fails [`ProxyNode`] validation,
    /// a protocol-specific field is missing or unsupported, or the transport and
    /// TLS combination is one the Core would reject.
    pub fn build(
        self,
        id: Uuid,
        credential_ref: CredentialRef,
    ) -> Result<(ProxyNode, StoredNodeCredential), ManualNodeDraftError> {
        let credential = self.credential.build()?;
        let transport = resolve_transport(&credential, self.transport)?;
        let tls = resolve_tls(&credential, self.tls)?;
        let mut node = ProxyNode::new(
            id,
            self.name,
            credential.protocol(),
            self.server,
            self.port,
            Some(credential_ref),
        )?;
        node.transport = transport;
        node.tls = tls;
        node.udp_enabled = self.udp_enabled;
        Ok((node, credential))
    }
}

impl Debug for ManualNodeDraft {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ManualNodeDraft([REDACTED])")
    }
}

/// Hysteria2 supplies its own QUIC transport; every other protocol runs over a
/// stream transport that defaults to plain TCP.
fn resolve_transport(
    credential: &StoredNodeCredential,
    transport: Option<TransportConfig>,
) -> Result<Option<TransportConfig>, ManualNodeDraftError> {
    match credential {
        StoredNodeCredential::Hysteria2(_) => {
            if transport.is_some() {
                return Err(ManualNodeDraftError::Hysteria2RejectsTransport);
            }
            Ok(None)
        }
        StoredNodeCredential::Shadowsocks(_) => match transport {
            None | Some(TransportConfig::Tcp) => Ok(Some(TransportConfig::Tcp)),
            Some(_) => Err(ManualNodeDraftError::ShadowsocksRequiresTcpTransport),
        },
        _ => Ok(Some(transport.unwrap_or(TransportConfig::Tcp))),
    }
}

/// Hysteria2 always runs over TLS, and only standard TLS — Reality is a
/// stream-protocol feature.
fn resolve_tls(
    credential: &StoredNodeCredential,
    tls: Option<TlsConfig>,
) -> Result<Option<TlsConfig>, ManualNodeDraftError> {
    if matches!(credential, StoredNodeCredential::Hysteria2(_))
        && !matches!(tls, Some(TlsConfig::Tls { .. }))
    {
        return Err(ManualNodeDraftError::Hysteria2RequiresTls);
    }
    Ok(tls)
}

/// Protocol-specific secret fields entered in the manual creation form.
#[derive(Clone, Deserialize)]
#[serde(tag = "protocol", rename_all = "lowercase")]
pub enum ManualCredentialDraft {
    #[serde(rename_all = "camelCase")]
    Vless {
        user_id: Uuid,
        #[serde(default)]
        flow: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Vmess {
        user_id: Uuid,
        security: VmessSecurity,
        #[serde(default)]
        alter_id: u32,
    },
    #[serde(rename_all = "camelCase")]
    Trojan { password: String },
    #[serde(rename_all = "camelCase")]
    Shadowsocks { method: String, password: String },
    #[serde(rename_all = "camelCase")]
    Hysteria2 {
        #[serde(default)]
        authentication: Option<String>,
        #[serde(default)]
        obfuscation: Option<ManualObfuscationDraft>,
    },
}

impl ManualCredentialDraft {
    fn build(self) -> Result<StoredNodeCredential, ManualNodeDraftError> {
        match self {
            Self::Vless { user_id, flow } => Ok(StoredNodeCredential::Vless(VlessCredential {
                user_id,
                encryption: VLESS_ENCRYPTION.to_owned(),
                flow: optional(flow),
            })),
            Self::Vmess {
                user_id,
                security,
                alter_id,
            } => Ok(StoredNodeCredential::Vmess(VmessCredential {
                user_id,
                security,
                alter_id,
            })),
            Self::Trojan { password } => {
                let password = required(&password, ManualNodeDraftError::MissingTrojanPassword)?;
                Ok(StoredNodeCredential::Trojan(TrojanCredential {
                    password,
                    // The outbound generator rejects any Trojan flow, so the
                    // form never offers one.
                    flow: None,
                }))
            }
            Self::Shadowsocks { method, password } => {
                let method = required(&method, ManualNodeDraftError::MissingShadowsocksMethod)?;
                let password =
                    required(&password, ManualNodeDraftError::MissingShadowsocksPassword)?;
                if !SUPPORTED_METHODS.contains(&method.as_str()) {
                    return Err(ManualNodeDraftError::UnsupportedShadowsocksMethod {
                        value: method,
                    });
                }
                Ok(StoredNodeCredential::Shadowsocks(ShadowsocksCredential {
                    method,
                    password,
                }))
            }
            Self::Hysteria2 {
                authentication,
                obfuscation,
            } => Ok(StoredNodeCredential::Hysteria2(Hysteria2Credential {
                authentication: optional(authentication),
                obfuscation: obfuscation.map(ManualObfuscationDraft::build).transpose()?,
            })),
        }
    }
}

/// Optional Hysteria2 traffic obfuscation entered in the manual creation form.
///
/// Packet-size shaping is deliberately absent: the outbound generator refuses
/// it, so a form that collected it could only produce unusable nodes.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualObfuscationDraft {
    pub method: Hysteria2ObfuscationMethod,
    pub password: String,
}

impl ManualObfuscationDraft {
    fn build(self) -> Result<Hysteria2Obfuscation, ManualNodeDraftError> {
        let password = required(
            &self.password,
            ManualNodeDraftError::MissingObfuscationPassword,
        )?;
        Ok(Hysteria2Obfuscation {
            method: self.method,
            password,
            min_packet_size: None,
            max_packet_size: None,
        })
    }
}

impl Debug for ManualObfuscationDraft {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ManualObfuscationDraft([REDACTED])")
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum ManualNodeDraftError {
    #[error("manual node fields are invalid")]
    Node(#[from] NodeModelError),
    #[error("Trojan password is required")]
    MissingTrojanPassword,
    #[error("Shadowsocks cipher method is required")]
    MissingShadowsocksMethod,
    #[error("Shadowsocks password is required")]
    MissingShadowsocksPassword,
    #[error("unsupported Shadowsocks cipher method: {value}")]
    UnsupportedShadowsocksMethod { value: String },
    #[error("Shadowsocks only supports the TCP transport")]
    ShadowsocksRequiresTcpTransport,
    #[error("Hysteria2 obfuscation password is required")]
    MissingObfuscationPassword,
    #[error("Hysteria2 carries its own transport and accepts no transport setting")]
    Hysteria2RejectsTransport,
    #[error("Hysteria2 requires standard TLS")]
    Hysteria2RequiresTls,
}

const fn enabled_by_default() -> bool {
    true
}

/// Treats a blank optional field as absent so empty form inputs do not become
/// meaningless empty strings in the stored credential.
fn optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn required(value: &str, error: ManualNodeDraftError) -> Result<String, ManualNodeDraftError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        Err(error)
    } else {
        Ok(value)
    }
}
