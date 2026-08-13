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
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::anytls::AnyTlsCredential;
use crate::http_proxy::HttpCredential;
use crate::hysteria2::{Hysteria2Credential, Hysteria2Obfuscation, Hysteria2ObfuscationMethod};
use crate::shadowsocks::{SUPPORTED_METHODS, ShadowsocksCredential};
use crate::socks::SocksCredential;
use crate::trojan::TrojanCredential;
use crate::tuic::{TuicCongestionControl, TuicCredential, TuicUdpRelayMode};
use crate::vmess::{VmessCredential, VmessSecurity};
use crate::wireguard::WireGuardCredential;
use crate::{StoredNodeCredential, VlessCredential};

/// VLESS negotiates encryption at the TLS layer; the outbound generator accepts
/// no other value.
const VLESS_ENCRYPTION: &str = "none";

/// One node as entered in the manual creation form.
#[derive(Clone, Deserialize, Serialize)]
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
    /// Rebuilds the form draft from a persisted node and its decoded secret.
    ///
    /// Used by the edit dialog so the user sees the same fields that were saved,
    /// including credentials that already live in the OS store.
    #[must_use]
    pub fn from_stored(node: &ProxyNode, credential: &StoredNodeCredential) -> Self {
        Self {
            name: node.name.as_str().to_owned(),
            server: node.server.as_str().to_owned(),
            port: u32::from(node.port.get()),
            udp_enabled: node.udp_enabled,
            transport: node.transport.clone(),
            tls: node.tls.clone(),
            credential: ManualCredentialDraft::from_stored(credential),
        }
    }

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
        StoredNodeCredential::Tuic(_) => {
            if transport.is_some() {
                return Err(ManualNodeDraftError::TuicRejectsTransport);
            }
            Ok(None)
        }
        StoredNodeCredential::WireGuard(_) => {
            if transport.is_some() {
                return Err(ManualNodeDraftError::WireGuardRejectsTransport);
            }
            Ok(None)
        }
        StoredNodeCredential::AnyTls(_) => {
            if transport.is_some() {
                return Err(ManualNodeDraftError::AnyTlsRejectsTransport);
            }
            Ok(None)
        }
        StoredNodeCredential::Shadowsocks(_) => match transport {
            None | Some(TransportConfig::Tcp) => Ok(Some(TransportConfig::Tcp)),
            Some(_) => Err(ManualNodeDraftError::ShadowsocksRequiresTcpTransport),
        },
        StoredNodeCredential::Socks(_) => match transport {
            None | Some(TransportConfig::Tcp) => Ok(Some(TransportConfig::Tcp)),
            Some(_) => Err(ManualNodeDraftError::SocksRequiresTcpTransport),
        },
        StoredNodeCredential::Http(_) => match transport {
            None | Some(TransportConfig::Tcp) => Ok(Some(TransportConfig::Tcp)),
            Some(_) => Err(ManualNodeDraftError::HttpRequiresTcpTransport),
        },
        _ => Ok(Some(transport.unwrap_or(TransportConfig::Tcp))),
    }
}

/// Hysteria2 / TUIC always run over TLS, and only standard TLS — Reality is a
/// stream-protocol feature.
fn resolve_tls(
    credential: &StoredNodeCredential,
    tls: Option<TlsConfig>,
) -> Result<Option<TlsConfig>, ManualNodeDraftError> {
    match credential {
        StoredNodeCredential::Hysteria2(_) if !matches!(tls, Some(TlsConfig::Tls { .. })) => {
            Err(ManualNodeDraftError::Hysteria2RequiresTls)
        }
        StoredNodeCredential::Tuic(_) if !matches!(tls, Some(TlsConfig::Tls { .. })) => {
            Err(ManualNodeDraftError::TuicRequiresTls)
        }
        // SOCKS has no TLS layer of its own in this model.
        StoredNodeCredential::Socks(_) if tls.is_some() => {
            Err(ManualNodeDraftError::SocksRejectsTls)
        }
        // WireGuard authenticates peers by key, not by certificate.
        StoredNodeCredential::WireGuard(_) if tls.is_some() => {
            Err(ManualNodeDraftError::WireGuardRejectsTls)
        }
        // HTTP may run plain or wrapped in standard TLS, but Reality is a
        // stream-protocol feature the HTTP outbound cannot use.
        StoredNodeCredential::Http(_) if matches!(tls, Some(TlsConfig::Reality { .. })) => {
            Err(ManualNodeDraftError::HttpRejectsReality)
        }
        // AnyTLS is TLS from the first byte; v2rayN also exposes Reality for
        // it, so both TLS layers are accepted, but plaintext is not.
        StoredNodeCredential::AnyTls(_)
            if !matches!(tls, Some(TlsConfig::Tls { .. } | TlsConfig::Reality { .. })) =>
        {
            Err(ManualNodeDraftError::AnyTlsRequiresTls)
        }
        _ => Ok(tls),
    }
}

/// Protocol-specific secret fields entered in the manual creation form.
#[derive(Clone, Deserialize, Serialize)]
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
    #[serde(rename_all = "camelCase")]
    Tuic {
        uuid: Uuid,
        #[serde(default)]
        password: Option<String>,
        #[serde(default)]
        congestion_control: Option<TuicCongestionControl>,
        #[serde(default)]
        udp_relay_mode: Option<TuicUdpRelayMode>,
        #[serde(default)]
        udp_over_stream: bool,
        #[serde(default)]
        zero_rtt_handshake: bool,
    },
    #[serde(rename_all = "camelCase")]
    Socks {
        #[serde(default)]
        username: Option<String>,
        #[serde(default)]
        password: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Http {
        #[serde(default)]
        username: Option<String>,
        #[serde(default)]
        password: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    WireGuard {
        private_key: String,
        peer_public_key: String,
        #[serde(default)]
        pre_shared_key: Option<String>,
        local_address: Vec<String>,
        #[serde(default)]
        mtu: Option<u32>,
        #[serde(default)]
        reserved: Option<[u8; 3]>,
    },
    #[serde(rename_all = "camelCase")]
    AnyTls { password: String },
}

impl ManualCredentialDraft {
    fn from_stored(credential: &StoredNodeCredential) -> Self {
        match credential {
            StoredNodeCredential::Vless(value) => Self::Vless {
                user_id: value.user_id(),
                flow: value.flow().map(str::to_owned),
            },
            StoredNodeCredential::Vmess(value) => Self::Vmess {
                user_id: value.user_id(),
                security: value.security(),
                alter_id: value.alter_id(),
            },
            StoredNodeCredential::Trojan(value) => Self::Trojan {
                password: value.password().to_owned(),
            },
            StoredNodeCredential::Shadowsocks(value) => Self::Shadowsocks {
                method: value.method().to_owned(),
                password: value.password().to_owned(),
            },
            StoredNodeCredential::Hysteria2(value) => Self::Hysteria2 {
                authentication: value.authentication().map(str::to_owned),
                obfuscation: value.obfuscation().map(|obfs| ManualObfuscationDraft {
                    method: obfs.method(),
                    password: obfs.password().to_owned(),
                }),
            },
            StoredNodeCredential::Tuic(value) => Self::Tuic {
                uuid: value.uuid(),
                password: value.password().map(str::to_owned),
                congestion_control: value.congestion_control(),
                udp_relay_mode: value.udp_relay_mode(),
                udp_over_stream: value.udp_over_stream(),
                zero_rtt_handshake: value.zero_rtt_handshake(),
            },
            StoredNodeCredential::Socks(value) => Self::Socks {
                username: value.username().map(str::to_owned),
                password: value.password().map(str::to_owned),
            },
            StoredNodeCredential::Http(value) => Self::Http {
                username: value.username().map(str::to_owned),
                password: value.password().map(str::to_owned),
            },
            StoredNodeCredential::WireGuard(value) => Self::WireGuard {
                private_key: value.private_key().to_owned(),
                peer_public_key: value.peer_public_key().to_owned(),
                pre_shared_key: value.pre_shared_key().map(str::to_owned),
                local_address: value.local_address().to_vec(),
                mtu: value.mtu(),
                reserved: value.reserved(),
            },
            StoredNodeCredential::AnyTls(value) => Self::AnyTls {
                password: value.password().to_owned(),
            },
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "each protocol variant is a short arm; splitting would scatter validation"
    )]
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
            Self::Tuic {
                uuid,
                password,
                congestion_control,
                udp_relay_mode,
                udp_over_stream,
                zero_rtt_handshake,
            } => {
                if udp_relay_mode.is_some() && udp_over_stream {
                    return Err(ManualNodeDraftError::ConflictingTuicUdpRelay);
                }
                Ok(StoredNodeCredential::Tuic(TuicCredential {
                    uuid,
                    password: optional(password),
                    congestion_control,
                    udp_relay_mode,
                    udp_over_stream,
                    zero_rtt_handshake,
                }))
            }
            Self::Socks { username, password } => {
                let (username, password) = build_user_pass_credential(
                    username,
                    password,
                    ManualNodeDraftError::SocksPasswordRequiresUsername,
                )?;
                Ok(StoredNodeCredential::Socks(SocksCredential {
                    username,
                    password,
                }))
            }
            Self::Http { username, password } => {
                let (username, password) = build_user_pass_credential(
                    username,
                    password,
                    ManualNodeDraftError::HttpPasswordRequiresUsername,
                )?;
                Ok(StoredNodeCredential::Http(HttpCredential {
                    username,
                    password,
                }))
            }
            Self::WireGuard {
                private_key,
                peer_public_key,
                pre_shared_key,
                local_address,
                mtu,
                reserved,
            } => {
                let private_key = required(
                    &private_key,
                    ManualNodeDraftError::MissingWireGuardPrivateKey,
                )?;
                let peer_public_key = required(
                    &peer_public_key,
                    ManualNodeDraftError::MissingWireGuardPeerPublicKey,
                )?;
                let local_address: Vec<String> = local_address
                    .into_iter()
                    .map(|address| address.trim().to_owned())
                    .filter(|address| !address.is_empty())
                    .collect();
                if local_address.is_empty() {
                    return Err(ManualNodeDraftError::MissingWireGuardLocalAddress);
                }
                Ok(StoredNodeCredential::WireGuard(WireGuardCredential {
                    private_key,
                    peer_public_key,
                    pre_shared_key: optional(pre_shared_key),
                    local_address,
                    mtu,
                    reserved,
                }))
            }
            Self::AnyTls { password } => {
                let password = required(&password, ManualNodeDraftError::MissingAnyTlsPassword)?;
                Ok(StoredNodeCredential::AnyTls(AnyTlsCredential { password }))
            }
        }
    }
}

/// Validates the optional username/password pair SOCKS and HTTP share: a
/// username alone is fine, but a password with no username has nothing to
/// authenticate.
fn build_user_pass_credential(
    username: Option<String>,
    password: Option<String>,
    password_requires_username: ManualNodeDraftError,
) -> Result<(Option<String>, Option<String>), ManualNodeDraftError> {
    let username = optional(username);
    let password = optional(password);
    if username.is_none() && password.is_some() {
        return Err(password_requires_username);
    }
    Ok((username, password))
}

/// Optional Hysteria2 traffic obfuscation entered in the manual creation form.
///
/// Packet-size shaping is deliberately absent: the outbound generator refuses
/// it, so a form that collected it could only produce unusable nodes.
#[derive(Clone, Deserialize, Serialize)]
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
    #[error("TUIC carries its own transport and accepts no transport setting")]
    TuicRejectsTransport,
    #[error("TUIC requires standard TLS")]
    TuicRequiresTls,
    #[error("TUIC cannot set both udp_relay_mode and udp_over_stream")]
    ConflictingTuicUdpRelay,
    #[error("SOCKS only supports the TCP transport")]
    SocksRequiresTcpTransport,
    #[error("SOCKS has no TLS layer")]
    SocksRejectsTls,
    #[error("SOCKS password requires a username")]
    SocksPasswordRequiresUsername,
    #[error("HTTP proxy only supports the TCP transport")]
    HttpRequiresTcpTransport,
    #[error("HTTP proxy does not support Reality")]
    HttpRejectsReality,
    #[error("HTTP proxy password requires a username")]
    HttpPasswordRequiresUsername,
    #[error("WireGuard carries its own tunnel and accepts no transport setting")]
    WireGuardRejectsTransport,
    #[error("WireGuard authenticates peers by key and accepts no TLS setting")]
    WireGuardRejectsTls,
    #[error("WireGuard private key is required")]
    MissingWireGuardPrivateKey,
    #[error("WireGuard peer public key is required")]
    MissingWireGuardPeerPublicKey,
    #[error("WireGuard requires at least one local address")]
    MissingWireGuardLocalAddress,
    #[error("AnyTLS carries its own session layer and accepts no transport setting")]
    AnyTlsRejectsTransport,
    #[error("AnyTLS requires TLS or Reality")]
    AnyTlsRequiresTls,
    #[error("AnyTLS password is required")]
    MissingAnyTlsPassword,
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
