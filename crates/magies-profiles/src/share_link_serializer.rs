//! Turns a stored node back into the sharing URI it came from.
//!
//! This is the inverse of [`crate::ShareLinkParser`], and the only property it
//! guarantees is that the parser accepts what it writes: exporting exists so a
//! link can be read again, here or in another client. The round trip is pinned by
//! `tests/share_link_serializer.rs`.

use std::fmt::Write as _;

use base64::Engine as _;
use base64::engine::general_purpose;
use magies_domain::{GrpcMode, ProxyNode, ProxyProtocol, TlsConfig, TransportConfig, XhttpMode};

use crate::xhttp_mode_name;
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use serde_json::{Value, json};
use thiserror::Error;

use crate::credential_codec::StoredNodeCredential;
use crate::hysteria2::{Hysteria2Credential, Hysteria2ObfuscationMethod};
use crate::tuic::{TuicCongestionControl, TuicCredential, TuicUdpRelayMode};
use crate::vmess::{VmessCredential, VmessSecurity};
use crate::wireguard::WireGuardCredential;
use crate::{TrojanCredential, VlessCredential};

/// Everything a URI component must escape.
///
/// Query values and the fragment sit in the same set here: over-escaping is
/// harmless because the parser percent-decodes, while under-escaping produces a
/// link that reparses into a different node.
const ESCAPED: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'&')
    .add(b'+')
    .add(b'/')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'\\')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

/// Writes a node and its credential as a sharing URI.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ShareLinkSerializer;

impl ShareLinkSerializer {
    /// Builds the sharing URI for one node.
    ///
    /// # Errors
    ///
    /// Returns [`ShareLinkSerializerError::CredentialProtocolMismatch`] when the
    /// credential belongs to a different protocol than the node, and
    /// [`ShareLinkSerializerError::MissingTransport`] when a protocol that needs
    /// a transport has none.
    pub fn serialize(
        node: &ProxyNode,
        credential: &StoredNodeCredential,
    ) -> Result<String, ShareLinkSerializerError> {
        if credential.protocol() != node.protocol_type {
            return Err(ShareLinkSerializerError::CredentialProtocolMismatch {
                expected: node.protocol_type,
                actual: credential.protocol(),
            });
        }
        if matches!(credential, StoredNodeCredential::Custom(_)) {
            return Err(ShareLinkSerializerError::UnrepresentableCustomNode);
        }
        // `VMess` is written as the Base64 JSON document, which is what v2rayN
        // exports and the only form that carries `alterId`.
        if let StoredNodeCredential::Vmess(vmess) = credential {
            return serialize_vmess(node, vmess);
        }
        let mut query = Query::default();
        let authority = match credential {
            StoredNodeCredential::Vless(value) => vless_authority(value, &mut query),
            StoredNodeCredential::Vmess(_) => unreachable!("VMess returned above"),
            StoredNodeCredential::Trojan(value) => trojan_authority(value, &mut query),
            StoredNodeCredential::Shadowsocks(value) => {
                format!("{}:{}", encode(value.method()), encode(value.password()))
            }
            StoredNodeCredential::Hysteria2(value) => hysteria2_authority(value, &mut query),
            StoredNodeCredential::Tuic(value) => tuic_authority(value, &mut query),
            StoredNodeCredential::Socks(value) => {
                user_pass_authority(value.username(), value.password())
            }
            StoredNodeCredential::Http(value) => {
                user_pass_authority(value.username(), value.password())
            }
            StoredNodeCredential::WireGuard(value) => wireguard_authority(value, &mut query),
            StoredNodeCredential::AnyTls(value) => encode(value.password()),
            StoredNodeCredential::Naive(value) => {
                if let Some(control) = value.quic_congestion_control() {
                    query.set("congestion_control", control.as_str());
                }
                user_pass_authority(value.username(), value.password())
            }
            StoredNodeCredential::Custom(_) => {
                unreachable!("custom nodes returned above")
            }
        };
        // Only VLESS and Trojan read a `type` parameter. Shadowsocks rejects
        // unknown parameters outright, and Hysteria2/TUIC carry their own QUIC
        // transport, so a missing transport is expected for all three. SOCKS
        // and HTTP are the same story: their transport is always plain TCP.
        let carries_transport = matches!(
            node.protocol_type,
            ProxyProtocol::Vless | ProxyProtocol::Trojan
        );
        if carries_transport {
            let transport =
                node.transport
                    .as_ref()
                    .ok_or(ShareLinkSerializerError::MissingTransport {
                        protocol: node.protocol_type,
                    })?;
            write_transport(transport, &mut query);
        }
        // Shadowsocks has no TLS layer of its own in this model; SOCKS never
        // carries TLS either, HTTP signals it through the `https` scheme
        // rather than a query parameter, and `WireGuard` authenticates peers by
        // key instead of certificate, so none of the four write TLS fields here.
        // Naive TLS may only carry `sni` — sing-box rejects the rest.
        if node.protocol_type == ProxyProtocol::Naive {
            write_naive_tls(node.tls.as_ref(), &mut query)?;
        } else if let Some(tls) = node.tls.as_ref()
            && !matches!(
                node.protocol_type,
                ProxyProtocol::Shadowsocks
                    | ProxyProtocol::Socks
                    | ProxyProtocol::Http
                    | ProxyProtocol::WireGuard
            )
        {
            write_tls(node.protocol_type, tls, &mut query);
        }

        let scheme = match credential {
            StoredNodeCredential::Naive(value) if value.quic() => "naive+quic",
            StoredNodeCredential::Naive(_) => "naive",
            _ => scheme(node.protocol_type, node.tls.is_some()),
        };
        let mut link = format!(
            "{scheme}://{authority}@{}:{}",
            node.server.as_str(),
            node.port.get()
        );
        if let Some(rendered) = query.render() {
            link.push('?');
            link.push_str(&rendered);
        }
        // The name always survives as the fragment: it is the only field the
        // node model holds that no other component carries.
        link.push('#');
        let _ = write!(link, "{}", encode(node.name.as_str()));
        Ok(link)
    }
}

/// Query parameters in insertion order, so the output is stable.
#[derive(Debug, Default)]
struct Query(Vec<(&'static str, String)>);

impl Query {
    fn set(&mut self, name: &'static str, value: impl AsRef<str>) {
        self.0.push((name, encode(value.as_ref())));
    }

    fn render(&self) -> Option<String> {
        if self.0.is_empty() {
            return None;
        }
        Some(
            self.0
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join("&"),
        )
    }
}

fn encode(value: &str) -> String {
    utf8_percent_encode(value, ESCAPED).to_string()
}

/// The sharing URI scheme for `protocol`.
///
/// HTTP is the only protocol whose scheme depends on more than the protocol
/// itself: `has_tls` picks `https` over `http` since that link carries no
/// other way to signal it.
const fn scheme(protocol: ProxyProtocol, has_tls: bool) -> &'static str {
    match protocol {
        ProxyProtocol::Vless => "vless",
        ProxyProtocol::Vmess => "vmess",
        ProxyProtocol::Trojan => "trojan",
        ProxyProtocol::Shadowsocks => "ss",
        ProxyProtocol::Hysteria2 => "hysteria2",
        ProxyProtocol::Tuic => "tuic",
        ProxyProtocol::Socks => "socks",
        ProxyProtocol::Http if has_tls => "https",
        ProxyProtocol::Http => "http",
        ProxyProtocol::WireGuard => "wireguard",
        ProxyProtocol::AnyTls => "anytls",
        // Naive's scheme also encodes QUIC vs HTTP/2; callers that know the
        // credential pick `naive` / `naive+quic` themselves before this runs.
        ProxyProtocol::Naive => "naive",
        ProxyProtocol::Custom => "custom",
    }
}

/// The user-info authority SOCKS and HTTP share: both credentials have an
/// optional username and password, unlike every other protocol's mandatory
/// secret.
fn user_pass_authority(username: Option<&str>, password: Option<&str>) -> String {
    match (username, password) {
        (Some(username), Some(password)) => format!("{}:{}", encode(username), encode(password)),
        (Some(username), None) => encode(username),
        (None, _) => String::new(),
    }
}

fn vless_authority(credential: &VlessCredential, query: &mut Query) -> String {
    query.set("encryption", credential.encryption());
    if let Some(flow) = credential.flow() {
        query.set("flow", flow);
    }
    credential.user_id().to_string()
}

/// Writes a `VMess` node as `vmess://<base64 json>`.
///
/// The URI form cannot carry `alterId`, so everything that can be expressed goes
/// through the JSON document instead. What the document cannot express is
/// refused rather than dropped: the parser reading it back would produce a
/// different node, which is worse than an error at export time.
fn serialize_vmess(
    node: &ProxyNode,
    credential: &VmessCredential,
) -> Result<String, ShareLinkSerializerError> {
    let mut document = json!({
        "v": "2",
        "ps": node.name.as_str(),
        "add": node.server.as_str(),
        "port": node.port.get().to_string(),
        "id": credential.user_id().to_string(),
        "aid": credential.alter_id().to_string(),
        "scy": security_name(credential.security()),
    });
    let transport = node
        .transport
        .as_ref()
        .ok_or(ShareLinkSerializerError::MissingTransport {
            protocol: ProxyProtocol::Vmess,
        })?;
    write_legacy_vmess_transport(transport, &mut document)?;
    match node.tls.as_ref() {
        None => document["tls"] = Value::String("none".to_owned()),
        Some(TlsConfig::Tls {
            server_name,
            allow_insecure,
            alpn,
            fingerprint,
            pinned_sha256,
        }) => {
            // `pcs` exists in the document but the parser rejects it, so a
            // pinned VMess node cannot be read back from this form.
            if pinned_sha256.is_some() {
                return Err(ShareLinkSerializerError::UnrepresentableVmessTls);
            }
            document["tls"] = Value::String("tls".to_owned());
            if let Some(server_name) = server_name {
                document["sni"] = Value::String(server_name.clone());
            }
            if *allow_insecure {
                document["insecure"] = Value::String("1".to_owned());
            }
            if !alpn.is_empty() {
                document["alpn"] = Value::String(alpn.join(","));
            }
            if let Some(fingerprint) = fingerprint {
                document["fp"] = Value::String(fingerprint.clone());
            }
        }
        // Reality has no representation in the VMess document at all.
        Some(TlsConfig::Reality { .. }) => {
            return Err(ShareLinkSerializerError::UnrepresentableVmessTls);
        }
    }

    let payload = serde_json::to_vec(&document)
        .map_err(|_| ShareLinkSerializerError::UnrepresentableVmessTls)?;
    Ok(format!(
        "vmess://{}",
        general_purpose::STANDARD.encode(payload)
    ))
}

/// Writes a node's transport into the legacy `VMess` JSON document.
///
/// What the document cannot express is refused rather than dropped: the
/// parser reading it back would produce a different node, which is worse
/// than an error at export time.
fn write_legacy_vmess_transport(
    transport: &TransportConfig,
    document: &mut Value,
) -> Result<(), ShareLinkSerializerError> {
    match transport {
        TransportConfig::Tcp => document["net"] = Value::String("tcp".to_owned()),
        TransportConfig::WebSocket { path, host } => {
            document["net"] = Value::String("ws".to_owned());
            document["path"] = Value::String(path.clone());
            if let Some(host) = host {
                document["host"] = Value::String(host.clone());
            }
        }
        TransportConfig::HttpUpgrade { path, host } => {
            document["net"] = Value::String("httpupgrade".to_owned());
            document["path"] = Value::String(path.clone());
            if let Some(host) = host {
                document["host"] = Value::String(host.clone());
            }
        }
        TransportConfig::Grpc {
            service_name,
            mode,
            authority,
        } => {
            // The document has one field for the service name and no room for a
            // mode or an authority, so anything beyond plain gun mode is lost.
            if *mode != GrpcMode::Gun || authority.is_some() {
                return Err(ShareLinkSerializerError::UnrepresentableVmessTransport);
            }
            document["net"] = Value::String("grpc".to_owned());
            document["path"] = Value::String(service_name.clone());
        }
        TransportConfig::XHttp { path, host, mode } => {
            // Legacy VMess JSON has no `mode` field, so only Auto survives a
            // round-trip; any other mode would come back as Auto and lie.
            if *mode != XhttpMode::Auto {
                return Err(ShareLinkSerializerError::UnrepresentableVmessTransport);
            }
            document["net"] = Value::String("xhttp".to_owned());
            document["path"] = Value::String(path.clone());
            if let Some(host) = host {
                document["host"] = Value::String(host.clone());
            }
        }
        TransportConfig::Kcp {
            mtu,
            tti,
            uplink_capacity,
            downlink_capacity,
            congestion,
            header_type,
            seed,
        } => {
            write_legacy_vmess_kcp(
                *mtu,
                *tti,
                *uplink_capacity,
                *downlink_capacity,
                *congestion,
                header_type.as_deref(),
                seed.as_deref(),
                document,
            )?;
        }
    }
    Ok(())
}

/// Writes a KCP transport into the legacy `VMess` JSON document.
///
/// The document only has room for `type` (header) and `path` (seed),
/// matching the v2rayN convention the parser reads back; the numeric knobs
/// have no field and would silently vanish, so they are rejected instead of
/// lying about a round trip.
#[expect(
    clippy::too_many_arguments,
    reason = "one argument per KCP field, mirroring the transport variant's own shape"
)]
fn write_legacy_vmess_kcp(
    mtu: Option<u32>,
    tti: Option<u32>,
    uplink_capacity: Option<u32>,
    downlink_capacity: Option<u32>,
    congestion: bool,
    header_type: Option<&str>,
    seed: Option<&str>,
    document: &mut Value,
) -> Result<(), ShareLinkSerializerError> {
    if mtu.is_some()
        || tti.is_some()
        || uplink_capacity.is_some()
        || downlink_capacity.is_some()
        || congestion
    {
        return Err(ShareLinkSerializerError::UnrepresentableVmessTransport);
    }
    document["net"] = Value::String("kcp".to_owned());
    document["type"] = Value::String(header_type.unwrap_or("none").to_owned());
    if let Some(seed) = seed {
        document["path"] = Value::String(seed.to_owned());
    }
    Ok(())
}

fn trojan_authority(credential: &TrojanCredential, query: &mut Query) -> String {
    if let Some(flow) = credential.flow() {
        query.set("flow", flow);
    }
    encode(credential.password())
}

fn hysteria2_authority(credential: &Hysteria2Credential, query: &mut Query) -> String {
    if let Some(obfuscation) = credential.obfuscation() {
        query.set(
            "obfs",
            match obfuscation.method() {
                Hysteria2ObfuscationMethod::Salamander => "salamander",
                Hysteria2ObfuscationMethod::Gecko => "gecko",
            },
        );
        query.set("obfs-password", obfuscation.password());
        if let Some(size) = obfuscation.min_packet_size() {
            query.set("min-packet-size", size.to_string());
        }
        if let Some(size) = obfuscation.max_packet_size() {
            query.set("max-packet-size", size.to_string());
        }
    }
    credential.authentication().map_or_else(String::new, encode)
}

fn tuic_authority(credential: &TuicCredential, query: &mut Query) -> String {
    if let Some(congestion_control) = credential.congestion_control() {
        query.set(
            "congestion_control",
            tuic_congestion_control_name(congestion_control),
        );
    }
    if let Some(udp_relay_mode) = credential.udp_relay_mode() {
        query.set("udp_relay_mode", tuic_udp_relay_mode_name(udp_relay_mode));
    }
    if credential.udp_over_stream() {
        query.set("udp_over_stream", "1");
    }
    if credential.zero_rtt_handshake() {
        query.set("zero_rtt_handshake", "1");
    }
    let uuid = encode(&credential.uuid().to_string());
    credential.password().map_or_else(
        || uuid.clone(),
        |password| format!("{uuid}:{}", encode(password)),
    )
}

/// The private key is the userinfo; everything else `WireGuard` needs has no
/// natural home in the authority, so it all becomes query parameters.
fn wireguard_authority(credential: &WireGuardCredential, query: &mut Query) -> String {
    query.set("publickey", credential.peer_public_key());
    query.set("address", credential.local_address().join(","));
    if let Some(pre_shared_key) = credential.pre_shared_key() {
        query.set("presharedkey", pre_shared_key);
    }
    if let Some(mtu) = credential.mtu() {
        query.set("mtu", mtu.to_string());
    }
    if let Some(reserved) = credential.reserved() {
        query.set(
            "reserved",
            reserved
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    encode(credential.private_key())
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

const fn security_name(security: VmessSecurity) -> &'static str {
    match security {
        VmessSecurity::Auto => "auto",
        VmessSecurity::Aes128Gcm => "aes-128-gcm",
        VmessSecurity::Chacha20Poly1305 => "chacha20-poly1305",
        VmessSecurity::None => "none",
        VmessSecurity::Zero => "zero",
    }
}

fn write_transport(transport: &TransportConfig, query: &mut Query) {
    match transport {
        TransportConfig::Tcp => query.set("type", "tcp"),
        TransportConfig::WebSocket { path, host } => {
            query.set("type", "ws");
            query.set("path", path);
            if let Some(host) = host {
                query.set("host", host);
            }
        }
        TransportConfig::HttpUpgrade { path, host } => {
            query.set("type", "httpupgrade");
            query.set("path", path);
            if let Some(host) = host {
                query.set("host", host);
            }
        }
        TransportConfig::Grpc {
            service_name,
            mode,
            authority,
        } => {
            query.set("type", "grpc");
            query.set("serviceName", service_name);
            query.set("mode", format!("{mode:?}").to_lowercase());
            if let Some(authority) = authority {
                query.set("authority", authority);
            }
        }
        TransportConfig::XHttp { path, host, mode } => {
            query.set("type", "xhttp");
            query.set("path", path);
            if let Some(host) = host {
                query.set("host", host);
            }
            if *mode != XhttpMode::Auto {
                query.set("mode", xhttp_mode_name(*mode));
            }
        }
        TransportConfig::Kcp {
            mtu,
            tti,
            uplink_capacity,
            downlink_capacity,
            congestion,
            header_type,
            seed,
        } => {
            query.set("type", "kcp");
            if let Some(mtu) = mtu {
                query.set("mtu", mtu.to_string());
            }
            if let Some(tti) = tti {
                query.set("tti", tti.to_string());
            }
            if let Some(uplink_capacity) = uplink_capacity {
                query.set("uplinkCapacity", uplink_capacity.to_string());
            }
            if let Some(downlink_capacity) = downlink_capacity {
                query.set("downlinkCapacity", downlink_capacity.to_string());
            }
            if *congestion {
                query.set("congestion", "1");
            }
            if let Some(header_type) = header_type {
                query.set("headerType", header_type);
            }
            if let Some(seed) = seed {
                query.set("seed", seed);
            }
        }
    }
}

fn write_tls(protocol: ProxyProtocol, tls: &TlsConfig, query: &mut Query) {
    match tls {
        TlsConfig::Tls {
            server_name,
            allow_insecure,
            alpn,
            fingerprint,
            pinned_sha256,
        } => {
            // Hysteria2 and TUIC are TLS by definition and their parsers
            // reject `security`. AnyTLS defaults `security` to `tls` when the
            // link omits it, so a plain TLS node round-trips without it too.
            if !matches!(
                protocol,
                ProxyProtocol::Hysteria2 | ProxyProtocol::Tuic | ProxyProtocol::AnyTls
            ) {
                query.set("security", "tls");
            }
            if let Some(server_name) = server_name {
                query.set("sni", server_name);
            }
            if *allow_insecure {
                query.set("insecure", "1");
            }
            if !alpn.is_empty() {
                query.set("alpn", alpn.join(","));
            }
            if let Some(fingerprint) = fingerprint {
                query.set("fp", fingerprint);
            }
            if let Some(pin) = pinned_sha256 {
                query.set("pinSHA256", pin.as_str());
            }
        }
        TlsConfig::Reality {
            server_name,
            public_key,
            short_id,
            fingerprint,
            alpn,
            spider_x,
        } => {
            query.set("security", "reality");
            query.set("sni", server_name);
            query.set("pbk", public_key);
            if let Some(short_id) = short_id {
                query.set("sid", short_id);
            }
            if let Some(fingerprint) = fingerprint {
                query.set("fp", fingerprint);
            }
            if !alpn.is_empty() {
                query.set("alpn", alpn.join(","));
            }
            if let Some(spider_x) = spider_x {
                query.set("spx", spider_x);
            }
        }
    }
}

/// Naive TLS may only carry `sni`. sing-box rejects fingerprint / ALPN /
/// insecure / pin / Reality for this outbound, so anything else refuses export
/// rather than producing a link that would parse back differently.
fn write_naive_tls(
    tls: Option<&TlsConfig>,
    query: &mut Query,
) -> Result<(), ShareLinkSerializerError> {
    match tls {
        Some(TlsConfig::Tls {
            server_name,
            allow_insecure,
            alpn,
            fingerprint,
            pinned_sha256,
        }) => {
            if *allow_insecure
                || !alpn.is_empty()
                || fingerprint.is_some()
                || pinned_sha256.is_some()
            {
                return Err(ShareLinkSerializerError::UnrepresentableNaiveTls);
            }
            if let Some(server_name) = server_name {
                query.set("sni", server_name);
            }
            Ok(())
        }
        Some(TlsConfig::Reality { .. }) | None => {
            Err(ShareLinkSerializerError::UnrepresentableNaiveTls)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ShareLinkSerializerError {
    #[error("credential protocol {actual:?} does not match node protocol {expected:?}")]
    CredentialProtocolMismatch {
        expected: ProxyProtocol,
        actual: ProxyProtocol,
    },
    #[error("{protocol:?} requires a transport configuration")]
    MissingTransport { protocol: ProxyProtocol },
    #[error("the VMess sharing document cannot carry this transport")]
    UnrepresentableVmessTransport,
    #[error("the VMess sharing document cannot carry this TLS configuration")]
    UnrepresentableVmessTls,
    #[error("the Naive sharing URI cannot carry this TLS configuration")]
    UnrepresentableNaiveTls,
    #[error("custom nodes cannot be exported as sharing links")]
    UnrepresentableCustomNode,
}
