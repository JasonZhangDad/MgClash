use std::fmt::{Debug, Formatter};

use magies_domain::{GrpcMode, ProxyNode, ProxyProtocol, TlsConfig, TransportConfig};
use serde_json::{Value, json};

use crate::{
    AnyTlsCredential, CustomCredential, HttpCredential, Hysteria2Credential,
    Hysteria2ObfuscationMethod, NaiveCredential, ShadowsocksCredential, SocksCredential,
    TrojanCredential, TuicCongestionControl, TuicCredential, TuicUdpRelayMode, VlessCredential,
    VmessCredential, VmessSecurity, WireGuardCredential,
};

#[derive(Clone, Copy)]
pub enum NodeCredential<'a> {
    Vless(&'a VlessCredential),
    Vmess(&'a VmessCredential),
    Trojan(&'a TrojanCredential),
    Shadowsocks(&'a ShadowsocksCredential),
    Hysteria2(&'a Hysteria2Credential),
    Tuic(&'a TuicCredential),
    Socks(&'a SocksCredential),
    Http(&'a HttpCredential),
    WireGuard(&'a WireGuardCredential),
    AnyTls(&'a AnyTlsCredential),
    Naive(&'a NaiveCredential),
    Custom(&'a CustomCredential),
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
            Self::Tuic(_) => ProxyProtocol::Tuic,
            Self::Socks(_) => ProxyProtocol::Socks,
            Self::Http(_) => ProxyProtocol::Http,
            Self::WireGuard(_) => ProxyProtocol::WireGuard,
            Self::AnyTls(_) => ProxyProtocol::AnyTls,
            Self::Naive(_) => ProxyProtocol::Naive,
            Self::Custom(_) => ProxyProtocol::Custom,
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

impl<'a> From<&'a TuicCredential> for NodeCredential<'a> {
    fn from(value: &'a TuicCredential) -> Self {
        Self::Tuic(value)
    }
}

impl<'a> From<&'a SocksCredential> for NodeCredential<'a> {
    fn from(value: &'a SocksCredential) -> Self {
        Self::Socks(value)
    }
}

impl<'a> From<&'a HttpCredential> for NodeCredential<'a> {
    fn from(value: &'a HttpCredential) -> Self {
        Self::Http(value)
    }
}

impl<'a> From<&'a WireGuardCredential> for NodeCredential<'a> {
    fn from(value: &'a WireGuardCredential) -> Self {
        Self::WireGuard(value)
    }
}

impl<'a> From<&'a AnyTlsCredential> for NodeCredential<'a> {
    fn from(value: &'a AnyTlsCredential) -> Self {
        Self::AnyTls(value)
    }
}

impl<'a> From<&'a NaiveCredential> for NodeCredential<'a> {
    fn from(value: &'a NaiveCredential) -> Self {
        Self::Naive(value)
    }
}

impl<'a> From<&'a CustomCredential> for NodeCredential<'a> {
    fn from(value: &'a CustomCredential) -> Self {
        Self::Custom(value)
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
            NodeCredential::Tuic(credential) => {
                generate_tuic(node, credential, &mut outbound)?;
            }
            NodeCredential::Socks(credential) => {
                generate_socks(node, credential, &mut outbound)?;
            }
            NodeCredential::Http(credential) => {
                generate_http(node, credential, &mut outbound)?;
            }
            NodeCredential::WireGuard(credential) => {
                generate_wireguard(node, credential, &mut outbound)?;
            }
            NodeCredential::AnyTls(credential) => {
                generate_anytls(node, credential, &mut outbound)?;
            }
            NodeCredential::Naive(credential) => {
                generate_naive(node, credential, &mut outbound)?;
            }
            NodeCredential::Custom(_) => {
                return Err(OutboundConfigError::ProtocolUnsupported {
                    protocol: ProxyProtocol::Custom,
                });
            }
        }
        Ok(GeneratedSingBoxOutbound { json: outbound })
    }
}

/// Enables sing-box multiplex on a generated outbound when the user asked for it.
///
/// Hysteria2 and TUIC already multiplex over QUIC, so mux is skipped there.
/// `WireGuard` is its own tunnel with no stream layer to multiplex over.
/// `AnyTLS` carries its own session-pooling multiplex scheme and rejects the
/// standard `multiplex` block outright. `Naive` tunnels over HTTP/2 or QUIC
/// and has no stream mux either.
pub fn apply_sing_box_multiplex(outbound: &mut Value, protocol: ProxyProtocol) {
    if matches!(
        protocol,
        ProxyProtocol::Hysteria2
            | ProxyProtocol::Tuic
            | ProxyProtocol::WireGuard
            | ProxyProtocol::AnyTls
            | ProxyProtocol::Naive
    ) {
        return;
    }
    outbound["multiplex"] = json!({
        "enabled": true,
        "protocol": "h2mux",
    });
}

/// Enables sing-box TLS `ClientHello` fragmentation on a generated outbound
/// when the user asked for it, v2rayN's Fragment toggle style.
///
/// `fragment` and `record_fragment` are outbound TLS fields (since sing-box
/// 1.12), so this only has anything to set on outbounds that carry a `tls`
/// object; protocols with no TLS (Shadowsocks, plain SOCKS/HTTP, `WireGuard`)
/// are skipped silently rather than gaining a `tls` block that was never
/// there.
pub fn apply_sing_box_fragment(outbound: &mut Value) {
    let Some(tls) = outbound.get_mut("tls").filter(|tls| tls.is_object()) else {
        return;
    };
    tls["fragment"] = Value::Bool(true);
    tls["record_fragment"] = Value::Bool(true);
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
        ProxyProtocol::Tuic => "tuic",
        ProxyProtocol::Socks => "socks",
        ProxyProtocol::Http => "http",
        ProxyProtocol::WireGuard => "wireguard",
        ProxyProtocol::AnyTls => "anytls",
        ProxyProtocol::Naive => "naive",
        ProxyProtocol::Custom => "custom",
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

fn generate_socks(
    node: &ProxyNode,
    credential: &SocksCredential,
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
    outbound["version"] = Value::String("5".to_owned());
    if let Some(username) = credential.username() {
        outbound["username"] = Value::String(username.to_owned());
    }
    if let Some(password) = credential.password() {
        outbound["password"] = Value::String(password.to_owned());
    }
    apply_network(node, outbound);
    Ok(())
}

fn generate_http(
    node: &ProxyNode,
    credential: &HttpCredential,
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
    if let Some(username) = credential.username() {
        outbound["username"] = Value::String(username.to_owned());
    }
    if let Some(password) = credential.password() {
        outbound["password"] = Value::String(password.to_owned());
    }
    if let Some(tls) = node.tls.as_ref() {
        outbound["tls"] = generated_tls(tls)?;
    }
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

fn generate_tuic(
    node: &ProxyNode,
    credential: &TuicCredential,
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
    outbound["uuid"] = Value::String(credential.uuid().to_string());
    if let Some(password) = credential.password() {
        outbound["password"] = Value::String(password.to_owned());
    }
    if let Some(congestion_control) = credential.congestion_control() {
        outbound["congestion_control"] =
            Value::String(tuic_congestion_control_name(congestion_control).to_owned());
    }
    if let Some(udp_relay_mode) = credential.udp_relay_mode() {
        outbound["udp_relay_mode"] =
            Value::String(tuic_udp_relay_mode_name(udp_relay_mode).to_owned());
    }
    if credential.udp_over_stream() {
        outbound["udp_over_stream"] = Value::Bool(true);
    }
    if credential.zero_rtt_handshake() {
        outbound["zero_rtt_handshake"] = Value::Bool(true);
    }
    outbound["tls"] = generated_tls(tls)?;
    apply_network(node, outbound);
    Ok(())
}

/// `AnyTLS` is TLS from the first byte, so it never carries a stream
/// transport; unlike Hysteria2/TUIC, its TLS layer may also be Reality
/// (v2rayN exposes this combination even though upstream `AnyTLS` is
/// TLS-only).
fn generate_anytls(
    node: &ProxyNode,
    credential: &AnyTlsCredential,
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
    outbound["password"] = Value::String(credential.password().to_owned());
    outbound["tls"] = generated_tls(tls)?;
    apply_network(node, outbound);
    Ok(())
}

/// `Naive` tunnels over HTTP/2 or QUIC. Official sing-box builds need platform
/// libcronet on some targets; the outbound shape still matches what v2rayN
/// emits for the pinned 1.13.18 Core.
///
/// sing-box Naive TLS accepts only `server_name` (plus certificate / ECH paths
/// this model does not expose), so fingerprint / ALPN / insecure / pin /
/// Reality are refused rather than dropped.
fn generate_naive(
    node: &ProxyNode,
    credential: &NaiveCredential,
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
    if let Some(username) = credential.username() {
        outbound["username"] = Value::String(username.to_owned());
    }
    if let Some(password) = credential.password() {
        outbound["password"] = Value::String(password.to_owned());
    }
    if credential.quic() {
        outbound["quic"] = Value::Bool(true);
    }
    if let Some(control) = credential.quic_congestion_control() {
        outbound["quic_congestion_control"] = Value::String(control.as_str().to_owned());
    }
    outbound["tls"] = generated_naive_tls(tls)?;
    apply_network(node, outbound);
    Ok(())
}

fn generated_naive_tls(tls: &TlsConfig) -> Result<Value, OutboundConfigError> {
    match tls {
        TlsConfig::Tls {
            server_name,
            allow_insecure,
            alpn,
            fingerprint,
            pinned_sha256,
        } => {
            if *allow_insecure
                || !alpn.is_empty()
                || fingerprint.is_some()
                || pinned_sha256.is_some()
            {
                return Err(OutboundConfigError::UnsupportedTls {
                    protocol: ProxyProtocol::Naive,
                });
            }
            let mut tls = json!({ "enabled": true });
            if let Some(server_name) = server_name {
                tls["server_name"] = Value::String(server_name.clone());
            }
            Ok(tls)
        }
        TlsConfig::Reality { .. } => Err(OutboundConfigError::UnsupportedTls {
            protocol: ProxyProtocol::Naive,
        }),
    }
}

/// `WireGuard` is its own tunnel: no stream transport, no TLS, and no `network`
/// restriction sing-box's `wireguard` outbound has no field for.
fn generate_wireguard(
    node: &ProxyNode,
    credential: &WireGuardCredential,
    outbound: &mut Value,
) -> Result<(), OutboundConfigError> {
    if node.transport.is_some() {
        return Err(OutboundConfigError::UnsupportedTransport {
            protocol: node.protocol_type,
        });
    }
    if node.tls.is_some() {
        return Err(OutboundConfigError::UnsupportedTls {
            protocol: node.protocol_type,
        });
    }
    outbound["local_address"] = json!(credential.local_address());
    outbound["private_key"] = Value::String(credential.private_key().to_owned());
    outbound["peer_public_key"] = Value::String(credential.peer_public_key().to_owned());
    if let Some(pre_shared_key) = credential.pre_shared_key() {
        outbound["pre_shared_key"] = Value::String(pre_shared_key.to_owned());
    }
    if let Some(mtu) = credential.mtu() {
        outbound["mtu"] = json!(mtu);
    }
    if let Some(reserved) = credential.reserved() {
        outbound["reserved"] = json!(reserved);
    }
    Ok(())
}

const fn tuic_congestion_control_name(value: TuicCongestionControl) -> &'static str {
    match value {
        TuicCongestionControl::Cubic => "cubic",
        TuicCongestionControl::NewReno => "new_reno",
        TuicCongestionControl::Bbr => "bbr",
    }
}

const fn tuic_udp_relay_mode_name(value: TuicUdpRelayMode) -> &'static str {
    match value {
        TuicUdpRelayMode::Native => "native",
        TuicUdpRelayMode::Quic => "quic",
    }
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
        TransportConfig::HttpUpgrade { path, host } => {
            let mut transport = json!({ "type": "httpupgrade", "path": path });
            if let Some(host) = host {
                transport["host"] = Value::String(host.clone());
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
        // Pinned sing-box 1.13.18 has no XHTTP transport; the capability
        // matrix routes these nodes to Xray instead of inventing a wire format.
        TransportConfig::XHttp { .. } => Err(OutboundConfigError::XhttpUnsupported),
        // Pinned sing-box 1.13.18 has no mKCP transport either; the
        // capability matrix routes these nodes to Xray the same way it does
        // for XHTTP.
        TransportConfig::Kcp { .. } => Err(OutboundConfigError::KcpUnsupported),
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
    #[error("XHTTP transport is unsupported by sing-box")]
    XhttpUnsupported,
    #[error("mKCP transport is unsupported by sing-box")]
    KcpUnsupported,
    #[error("{protocol:?} is not generated by the sing-box outbound builder")]
    ProtocolUnsupported { protocol: ProxyProtocol },
}
