use std::fmt::{Debug, Formatter};

use magies_domain::{GrpcMode, ProxyNode, ProxyProtocol, TlsConfig, TransportConfig};
use serde_json::{Value, json};

use crate::{
    Hysteria2Credential, Hysteria2ObfuscationMethod, ShadowsocksCredential, TrojanCredential,
    VlessCredential, VmessCredential, VmessSecurity,
};

#[derive(Clone, Copy)]
pub enum NodeCredential<'a> {
    Vless(&'a VlessCredential),
    Vmess(&'a VmessCredential),
    Trojan(&'a TrojanCredential),
    Shadowsocks(&'a ShadowsocksCredential),
    Hysteria2(&'a Hysteria2Credential),
}

impl NodeCredential<'_> {
    #[must_use]
    pub const fn protocol(self) -> ProxyProtocol {
        match self {
            Self::Vless(_) => ProxyProtocol::Vless,
            Self::Vmess(_) => ProxyProtocol::Vmess,
            Self::Trojan(_) => ProxyProtocol::Trojan,
            Self::Shadowsocks(_) => ProxyProtocol::Shadowsocks,
            Self::Hysteria2(_) => ProxyProtocol::Hysteria2,
        }
    }
}

impl Debug for NodeCredential<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("NodeCredential([REDACTED])")
    }
}

impl<'a> From<&'a VlessCredential> for NodeCredential<'a> {
    fn from(value: &'a VlessCredential) -> Self {
        Self::Vless(value)
    }
}

impl<'a> From<&'a VmessCredential> for NodeCredential<'a> {
    fn from(value: &'a VmessCredential) -> Self {
        Self::Vmess(value)
    }
}

impl<'a> From<&'a TrojanCredential> for NodeCredential<'a> {
    fn from(value: &'a TrojanCredential) -> Self {
        Self::Trojan(value)
    }
}

impl<'a> From<&'a ShadowsocksCredential> for NodeCredential<'a> {
    fn from(value: &'a ShadowsocksCredential) -> Self {
        Self::Shadowsocks(value)
    }
}

impl<'a> From<&'a Hysteria2Credential> for NodeCredential<'a> {
    fn from(value: &'a Hysteria2Credential) -> Self {
        Self::Hysteria2(value)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct GeneratedSingBoxOutbound {
    json: Value,
}

impl GeneratedSingBoxOutbound {
    #[must_use]
    pub const fn json(&self) -> &Value {
        &self.json
    }
}

impl Debug for GeneratedSingBoxOutbound {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("GeneratedSingBoxOutbound([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SingBoxOutboundConfigGenerator;

impl SingBoxOutboundConfigGenerator {
    /// Generates the selected node's sing-box outbound.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the node is disabled, the credential belongs
    /// to another protocol, or a field cannot be mapped without data loss.
    pub fn generate(
        node: &ProxyNode,
        credential: NodeCredential<'_>,
    ) -> Result<GeneratedSingBoxOutbound, OutboundConfigError> {
        if !node.enabled {
            return Err(OutboundConfigError::NodeDisabled);
        }
        if credential.protocol() != node.protocol_type {
            return Err(OutboundConfigError::CredentialProtocolMismatch {
                expected: node.protocol_type,
                actual: credential.protocol(),
            });
        }

        let mut outbound = base_outbound(node);
        match credential {
            NodeCredential::Vless(credential) => {
                generate_vless(node, credential, &mut outbound)?;
            }
            NodeCredential::Vmess(credential) => {
                generate_vmess(node, credential, &mut outbound)?;
            }
            NodeCredential::Trojan(credential) => {
                generate_trojan(node, credential, &mut outbound)?;
            }
            NodeCredential::Shadowsocks(credential) => {
                generate_shadowsocks(node, credential, &mut outbound)?;
            }
            NodeCredential::Hysteria2(credential) => {
                generate_hysteria2(node, credential, &mut outbound)?;
            }
        }
        Ok(GeneratedSingBoxOutbound { json: outbound })
    }
}

fn base_outbound(node: &ProxyNode) -> Value {
    json!({
        "type": protocol_name(node.protocol_type),
        "tag": "proxy",
        "server": node.server.as_str(),
        "server_port": node.port.get()
    })
}

const fn protocol_name(protocol: ProxyProtocol) -> &'static str {
    match protocol {
        ProxyProtocol::Vless => "vless",
        ProxyProtocol::Vmess => "vmess",
        ProxyProtocol::Trojan => "trojan",
        ProxyProtocol::Shadowsocks => "shadowsocks",
        ProxyProtocol::Hysteria2 => "hysteria2",
    }
}

fn generate_vless(
    node: &ProxyNode,
    credential: &VlessCredential,
    outbound: &mut Value,
) -> Result<(), OutboundConfigError> {
    if credential.encryption() != "none" {
        return Err(OutboundConfigError::UnsupportedVlessEncryption);
    }
    outbound["uuid"] = Value::String(credential.user_id().to_string());
    if let Some(flow) = credential.flow() {
        outbound["flow"] = Value::String(flow.to_owned());
    }
    apply_common_stream(node, outbound)
}

fn generate_vmess(
    node: &ProxyNode,
    credential: &VmessCredential,
    outbound: &mut Value,
) -> Result<(), OutboundConfigError> {
    outbound["uuid"] = Value::String(credential.user_id().to_string());
    outbound["security"] = Value::String(vmess_security(credential.security()).to_owned());
    if credential.alter_id() != 0 {
        outbound["alter_id"] = json!(credential.alter_id());
    }
    apply_common_stream(node, outbound)
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

fn generate_trojan(
    node: &ProxyNode,
    credential: &TrojanCredential,
    outbound: &mut Value,
) -> Result<(), OutboundConfigError> {
    if credential.flow().is_some() {
        return Err(OutboundConfigError::UnsupportedTrojanFlow);
    }
    outbound["password"] = Value::String(credential.password().to_owned());
    apply_common_stream(node, outbound)
}

fn generate_shadowsocks(
    node: &ProxyNode,
    credential: &ShadowsocksCredential,
    outbound: &mut Value,
) -> Result<(), OutboundConfigError> {
    match node.transport.as_ref() {
        Some(TransportConfig::Tcp) => {}
        Some(_) => {
            return Err(OutboundConfigError::UnsupportedTransport {
                protocol: node.protocol_type,
            });
        }
        None => {
            return Err(OutboundConfigError::MissingTransport {
                protocol: node.protocol_type,
            });
        }
    }
    if node.tls.is_some() {
        return Err(OutboundConfigError::UnsupportedTls {
            protocol: node.protocol_type,
        });
    }
    outbound["method"] = Value::String(credential.method().to_owned());
    outbound["password"] = Value::String(credential.password().to_owned());
    apply_network(node, outbound);
    Ok(())
}

fn generate_hysteria2(
    node: &ProxyNode,
    credential: &Hysteria2Credential,
    outbound: &mut Value,
) -> Result<(), OutboundConfigError> {
    if node.transport.is_some() {
        return Err(OutboundConfigError::UnsupportedTransport {
            protocol: node.protocol_type,
        });
    }
    let tls = node.tls.as_ref().ok_or(OutboundConfigError::MissingTls {
        protocol: node.protocol_type,
    })?;
    if !matches!(tls, TlsConfig::Tls { .. }) {
        return Err(OutboundConfigError::UnsupportedTls {
            protocol: node.protocol_type,
        });
    }
    if let Some(password) = credential.authentication() {
        outbound["password"] = Value::String(password.to_owned());
    }
    if let Some(obfuscation) = credential.obfuscation() {
        if obfuscation.min_packet_size().is_some() || obfuscation.max_packet_size().is_some() {
            return Err(OutboundConfigError::UnsupportedHysteria2PacketSizes);
        }
        let kind = match obfuscation.method() {
            Hysteria2ObfuscationMethod::Salamander => "salamander",
            Hysteria2ObfuscationMethod::Gecko => "gecko",
        };
        outbound["obfs"] = json!({
            "type": kind,
            "password": obfuscation.password()
        });
    }
    outbound["tls"] = generated_tls(tls)?;
    apply_network(node, outbound);
    Ok(())
}

fn apply_common_stream(node: &ProxyNode, outbound: &mut Value) -> Result<(), OutboundConfigError> {
    let transport = node
        .transport
        .as_ref()
        .ok_or(OutboundConfigError::MissingTransport {
            protocol: node.protocol_type,
        })?;
    if let Some(tls) = &node.tls {
        outbound["tls"] = generated_tls(tls)?;
    }
    if let Some(transport) = generated_transport(transport)? {
        outbound["transport"] = transport;
    }
    apply_network(node, outbound);
    Ok(())
}

fn generated_transport(transport: &TransportConfig) -> Result<Option<Value>, OutboundConfigError> {
    match transport {
        TransportConfig::Tcp => Ok(None),
        TransportConfig::WebSocket { path, host } => {
            let mut transport = json!({ "type": "ws", "path": path });
            if let Some(host) = host {
                transport["headers"] = json!({ "Host": host });
            }
            Ok(Some(transport))
        }
        TransportConfig::Grpc {
            service_name,
            mode,
            authority,
        } => {
            if *mode != GrpcMode::Gun {
                return Err(OutboundConfigError::UnsupportedGrpcMode { mode: *mode });
            }
            if authority.is_some() {
                return Err(OutboundConfigError::UnsupportedGrpcAuthority);
            }
            Ok(Some(json!({
                "type": "grpc",
                "service_name": service_name
            })))
        }
    }
}

fn generated_tls(tls: &TlsConfig) -> Result<Value, OutboundConfigError> {
    match tls {
        TlsConfig::Tls {
            server_name,
            allow_insecure,
            alpn,
            fingerprint,
            pinned_sha256,
        } => {
            // sing-box 1.13.18 has no `tls.pinned_sha256`; it verifies against a
            // whole certificate (`certificate` / `certificate_path`) instead.
            // Dropping the pin here would silently fall back to the CA chain the
            // user pinned around, so the refusal is explicit.
            if pinned_sha256.is_some() {
                return Err(OutboundConfigError::CertificatePinUnsupported);
            }
            let mut tls = json!({ "enabled": true });
            if let Some(server_name) = server_name {
                tls["server_name"] = Value::String(server_name.clone());
            }
            if *allow_insecure {
                tls["insecure"] = Value::Bool(true);
            }
            add_tls_client_options(&mut tls, alpn, fingerprint.as_deref());
            Ok(tls)
        }
        TlsConfig::Reality {
            server_name,
            public_key,
            short_id,
            fingerprint,
            alpn,
            spider_x,
        } => {
            if spider_x.is_some() {
                return Err(OutboundConfigError::UnsupportedRealitySpiderX);
            }
            let mut reality = json!({
                "enabled": true,
                "public_key": public_key
            });
            if let Some(short_id) = short_id {
                reality["short_id"] = Value::String(short_id.clone());
            }
            let mut tls = json!({
                "enabled": true,
                "server_name": server_name,
                "reality": reality
            });
            add_tls_client_options(&mut tls, alpn, fingerprint.as_deref());
            Ok(tls)
        }
    }
}

fn add_tls_client_options(tls: &mut Value, alpn: &[String], fingerprint: Option<&str>) {
    if !alpn.is_empty() {
        tls["alpn"] = json!(alpn);
    }
    if let Some(fingerprint) = fingerprint {
        tls["utls"] = json!({
            "enabled": true,
            "fingerprint": fingerprint
        });
    }
}

fn apply_network(node: &ProxyNode, outbound: &mut Value) {
    if !node.udp_enabled {
        outbound["network"] = Value::String("tcp".to_owned());
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum OutboundConfigError {
    #[error("selected proxy node is disabled")]
    NodeDisabled,
    #[error("credential protocol does not match the selected node")]
    CredentialProtocolMismatch {
        expected: ProxyProtocol,
        actual: ProxyProtocol,
    },
    #[error("{protocol:?} requires a transport configuration")]
    MissingTransport { protocol: ProxyProtocol },
    #[error("{protocol:?} transport is unsupported by sing-box")]
    UnsupportedTransport { protocol: ProxyProtocol },
    #[error("{protocol:?} requires TLS")]
    MissingTls { protocol: ProxyProtocol },
    #[error("{protocol:?} TLS configuration is unsupported by sing-box")]
    UnsupportedTls { protocol: ProxyProtocol },
    #[error("sing-box cannot verify a pinned certificate digest")]
    CertificatePinUnsupported,
    #[error("VLESS encryption is unsupported by sing-box")]
    UnsupportedVlessEncryption,
    #[error("Trojan flow is unsupported by sing-box")]
    UnsupportedTrojanFlow,
    #[error("gRPC mode {mode:?} is unsupported by sing-box")]
    UnsupportedGrpcMode { mode: GrpcMode },
    #[error("gRPC authority is unsupported by sing-box")]
    UnsupportedGrpcAuthority,
    #[error("Reality spiderX is unsupported by sing-box")]
    UnsupportedRealitySpiderX,
    #[error("Hysteria2 Gecko packet sizes are unsupported by sing-box")]
    UnsupportedHysteria2PacketSizes,
}
