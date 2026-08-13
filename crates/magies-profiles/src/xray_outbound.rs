//! Generates the selected node's Xray outbound.
//!
//! The Xray schema differs from sing-box in shape, not just in field names: the
//! protocol payload lives under `settings` (as `vnext` for the user-based
//! protocols, `servers` for the rest) and everything about the wire lives under
//! `streamSettings`. This mirrors [`crate::SingBoxOutboundConfigGenerator`] in
//! responsibility, and the capability matrix decides which of the two runs.

use magies_domain::{GrpcMode, ProxyNode, ProxyProtocol, TlsConfig, TransportConfig};
use serde_json::{Value, json};

use crate::sing_box_outbound::NodeCredential;
use crate::{VlessCredential, VmessCredential, VmessSecurity, xhttp_mode_name};

/// The tag every generator gives the proxy outbound, matching the route rules.
const PROXY_TAG: &str = "proxy";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedXrayOutbound {
    json: Value,
}

impl GeneratedXrayOutbound {
    #[must_use]
    pub const fn json(&self) -> &Value {
        &self.json
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XrayOutboundConfigGenerator;

impl XrayOutboundConfigGenerator {
    /// Generates the selected node's Xray outbound.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the node is disabled, the credential belongs
    /// to another protocol, Xray cannot serve the protocol at all, or a field
    /// cannot be mapped without data loss.
    pub fn generate(
        node: &ProxyNode,
        credential: NodeCredential<'_>,
    ) -> Result<GeneratedXrayOutbound, XrayOutboundError> {
        if !node.enabled {
            return Err(XrayOutboundError::NodeDisabled);
        }
        if credential.protocol() != node.protocol_type {
            return Err(XrayOutboundError::CredentialProtocolMismatch {
                expected: node.protocol_type,
                actual: credential.protocol(),
            });
        }

        let mut outbound = json!({
            "protocol": protocol_name(node.protocol_type)?,
            "tag": PROXY_TAG,
        });
        outbound["settings"] = match credential {
            NodeCredential::Vless(credential) => vless_settings(node, credential),
            NodeCredential::Vmess(credential) => vmess_settings(node, credential),
            NodeCredential::Trojan(credential) => {
                if credential.flow().is_some() {
                    return Err(XrayOutboundError::UnsupportedTrojanFlow);
                }
                json!({
                    "servers": [{
                        "address": node.server.as_str(),
                        "port": node.port.get(),
                        "password": credential.password(),
                    }]
                })
            }
            NodeCredential::Shadowsocks(credential) => json!({
                "servers": [{
                    "address": node.server.as_str(),
                    "port": node.port.get(),
                    "method": credential.method(),
                    "password": credential.password(),
                }]
            }),
            NodeCredential::Hysteria2(_) => {
                // Unreachable through the matrix, but the generator is public.
                return Err(XrayOutboundError::ProtocolUnsupported {
                    protocol: ProxyProtocol::Hysteria2,
                });
            }
            NodeCredential::Tuic(_) => {
                // Unreachable through the matrix, but the generator is public.
                return Err(XrayOutboundError::ProtocolUnsupported {
                    protocol: ProxyProtocol::Tuic,
                });
            }
            NodeCredential::WireGuard(_) => {
                // Xray does ship a `wireguard` outbound, but nothing in this
                // codebase's Xray patterns pins a verified field-for-field
                // shape for it, so the capability matrix keeps WireGuard
                // sing-box-only. Unreachable through the matrix, but the
                // generator is public.
                return Err(XrayOutboundError::ProtocolUnsupported {
                    protocol: ProxyProtocol::WireGuard,
                });
            }
            NodeCredential::AnyTls(_) => {
                // Xray ships no AnyTLS outbound at all. Unreachable through
                // the matrix, but the generator is public.
                return Err(XrayOutboundError::ProtocolUnsupported {
                    protocol: ProxyProtocol::AnyTls,
                });
            }
            NodeCredential::Naive(_) => {
                // Xray ships no Naive outbound at all. Unreachable through
                // the matrix, but the generator is public.
                return Err(XrayOutboundError::ProtocolUnsupported {
                    protocol: ProxyProtocol::Naive,
                });
            }
            NodeCredential::Socks(credential) => {
                user_pass_settings(node, credential.username(), credential.password())
            }
            NodeCredential::Http(credential) => {
                user_pass_settings(node, credential.username(), credential.password())
            }
        };
        outbound["streamSettings"] = stream_settings(node)?;

        Ok(GeneratedXrayOutbound { json: outbound })
    }
}

/// Enables Xray mux on a generated outbound when the user asked for it.
///
/// VLESS with a flow (e.g. `xtls-rprx-vision`) cannot share a mux connection.
pub fn apply_xray_mux(outbound: &mut Value, credential: NodeCredential<'_>) {
    if let NodeCredential::Vless(credential) = credential
        && credential.flow().is_some()
    {
        return;
    }
    if matches!(
        credential.protocol(),
        ProxyProtocol::Hysteria2
            | ProxyProtocol::Tuic
            | ProxyProtocol::WireGuard
            | ProxyProtocol::AnyTls
            | ProxyProtocol::Naive
    ) {
        return;
    }
    outbound["mux"] = json!({
        "enabled": true,
        "concurrency": 8,
    });
}

fn vless_settings(node: &ProxyNode, credential: &VlessCredential) -> Value {
    let mut user = json!({
        "id": credential.user_id().to_string(),
        "encryption": credential.encryption(),
    });
    if let Some(flow) = credential.flow() {
        user["flow"] = Value::String(flow.to_owned());
    }
    json!({
        "vnext": [{
            "address": node.server.as_str(),
            "port": node.port.get(),
            "users": [user],
        }]
    })
}

fn vmess_settings(node: &ProxyNode, credential: &VmessCredential) -> Value {
    json!({
        "vnext": [{
            "address": node.server.as_str(),
            "port": node.port.get(),
            "users": [{
                "id": credential.user_id().to_string(),
                "security": vmess_security(credential.security()),
                "alterId": credential.alter_id(),
            }],
        }]
    })
}

/// Builds the `servers` settings SOCKS and HTTP outbounds share: one endpoint,
/// with a single `users` entry only when a username was supplied. Xray
/// requires `pass` whenever `user` is present, so an absent password becomes
/// an empty string rather than an omitted field.
fn user_pass_settings(node: &ProxyNode, username: Option<&str>, password: Option<&str>) -> Value {
    let mut server = json!({
        "address": node.server.as_str(),
        "port": node.port.get(),
    });
    if let Some(username) = username {
        server["users"] = json!([{
            "user": username,
            "pass": password.unwrap_or(""),
        }]);
    }
    json!({ "servers": [server] })
}

/// Builds `streamSettings` from the node's transport and TLS.
fn stream_settings(node: &ProxyNode) -> Result<Value, XrayOutboundError> {
    let transport = node
        .transport
        .as_ref()
        .ok_or(XrayOutboundError::MissingTransport {
            protocol: node.protocol_type,
        })?;

    let mut stream = json!({ "network": network_name(transport) });
    match transport {
        TransportConfig::Tcp => {}
        TransportConfig::WebSocket { path, host } => {
            let mut settings = json!({ "path": path });
            if let Some(host) = host {
                settings["headers"] = json!({ "Host": host });
            }
            stream["wsSettings"] = settings;
        }
        TransportConfig::HttpUpgrade { path, host } => {
            let mut settings = json!({ "path": path });
            if let Some(host) = host {
                settings["host"] = Value::String(host.clone());
            }
            stream["httpupgradeSettings"] = settings;
        }
        TransportConfig::Grpc {
            service_name,
            mode,
            authority,
        } => {
            let mut settings = json!({
                "serviceName": service_name,
                "multiMode": matches!(mode, GrpcMode::Multi),
            });
            if let Some(authority) = authority {
                settings["authority"] = Value::String(authority.clone());
            }
            stream["grpcSettings"] = settings;
        }
        TransportConfig::XHttp { path, host, mode } => {
            let mut settings = json!({ "path": path });
            if let Some(host) = host {
                settings["host"] = Value::String(host.clone());
            }
            settings["mode"] = Value::String(xhttp_mode_name(*mode).to_owned());
            stream["xhttpSettings"] = settings;
        }
    }

    if let Some(tls) = node.tls.as_ref() {
        apply_tls(tls, &mut stream)?;
    }
    Ok(stream)
}

fn apply_tls(tls: &TlsConfig, stream: &mut Value) -> Result<(), XrayOutboundError> {
    match tls {
        TlsConfig::Tls {
            server_name,
            allow_insecure,
            alpn,
            fingerprint,
            pinned_sha256,
        } => {
            // Xray 26.3.27 removed `allowInsecure`, pointing users at
            // `pinnedPeerCertSha256` instead. Emitting it makes Xray refuse to
            // start, and silently dropping it would turn a node the user marked
            // insecure into one that fails its TLS handshake at connect time.
            if *allow_insecure {
                return Err(XrayOutboundError::InsecureTlsUnsupported);
            }
            stream["security"] = Value::String("tls".to_owned());
            let mut settings = json!({});
            if let Some(pin) = pinned_sha256 {
                // Measured against Xray 26.3.27: a hex *string*. An array is
                // rejected as the wrong type, Base64 as the wrong encoding.
                settings["pinnedPeerCertSha256"] = Value::String(pin.as_str().to_owned());
            }
            if let Some(server_name) = server_name {
                settings["serverName"] = Value::String(server_name.clone());
            }
            if !alpn.is_empty() {
                settings["alpn"] = json!(alpn);
            }
            if let Some(fingerprint) = fingerprint {
                settings["fingerprint"] = Value::String(fingerprint.clone());
            }
            stream["tlsSettings"] = settings;
        }
        TlsConfig::Reality {
            server_name,
            public_key,
            short_id,
            fingerprint,
            spider_x,
            ..
        } => {
            stream["security"] = Value::String("reality".to_owned());
            let mut settings = json!({
                "serverName": server_name,
                "publicKey": public_key,
            });
            if let Some(short_id) = short_id {
                settings["shortId"] = Value::String(short_id.clone());
            }
            if let Some(fingerprint) = fingerprint {
                settings["fingerprint"] = Value::String(fingerprint.clone());
            }
            if let Some(spider_x) = spider_x {
                settings["spiderX"] = Value::String(spider_x.clone());
            }
            // Reality carries no ALPN of its own; the field is deliberately
            // dropped rather than forwarded into a place Xray ignores.
            stream["realitySettings"] = settings;
        }
    }
    Ok(())
}

/// Rejects a protocol the capability matrix already excludes, so the two can
/// never disagree about what Xray can run.
fn protocol_name(protocol: ProxyProtocol) -> Result<&'static str, XrayOutboundError> {
    match protocol {
        ProxyProtocol::Vless => Ok("vless"),
        ProxyProtocol::Vmess => Ok("vmess"),
        ProxyProtocol::Trojan => Ok("trojan"),
        ProxyProtocol::Shadowsocks => Ok("shadowsocks"),
        ProxyProtocol::Socks => Ok("socks"),
        ProxyProtocol::Http => Ok("http"),
        ProxyProtocol::Hysteria2
        | ProxyProtocol::Tuic
        | ProxyProtocol::WireGuard
        | ProxyProtocol::AnyTls
        | ProxyProtocol::Naive => Err(XrayOutboundError::ProtocolUnsupported { protocol }),
    }
}

const fn network_name(transport: &TransportConfig) -> &'static str {
    match transport {
        TransportConfig::Tcp => "tcp",
        TransportConfig::WebSocket { .. } => "ws",
        TransportConfig::HttpUpgrade { .. } => "httpupgrade",
        TransportConfig::Grpc { .. } => "grpc",
        TransportConfig::XHttp { .. } => "xhttp",
    }
}

const fn vmess_security(security: VmessSecurity) -> &'static str {
    match security {
        VmessSecurity::Auto => "auto",
        VmessSecurity::Aes128Gcm => "aes-128-gcm",
        VmessSecurity::Chacha20Poly1305 => "chacha20-poly1305",
        VmessSecurity::None => "none",
        VmessSecurity::Zero => "zero",
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum XrayOutboundError {
    #[error("selected proxy node is disabled")]
    NodeDisabled,
    #[error("credential protocol does not match the selected node")]
    CredentialProtocolMismatch {
        expected: ProxyProtocol,
        actual: ProxyProtocol,
    },
    #[error("Xray cannot serve {protocol:?}")]
    ProtocolUnsupported { protocol: ProxyProtocol },
    #[error("{protocol:?} requires a transport configuration")]
    MissingTransport { protocol: ProxyProtocol },
    #[error("Xray does not accept a Trojan flow")]
    UnsupportedTrojanFlow,
    #[error("Xray no longer accepts insecure TLS; this node needs certificate pinning")]
    InsecureTlsUnsupported,
}
