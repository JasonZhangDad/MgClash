//! Turns a stored node back into the sharing URI it came from.
//!
//! This is the inverse of [`crate::ShareLinkParser`], and the only property it
//! guarantees is that the parser accepts what it writes: exporting exists so a
//! link can be read again, here or in another client. The round trip is pinned by
//! `tests/share_link_serializer.rs`.

use std::fmt::Write as _;

use base64::Engine as _;
use base64::engine::general_purpose;
use magies_domain::{GrpcMode, ProxyNode, ProxyProtocol, TlsConfig, TransportConfig};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use serde_json::{Value, json};
use thiserror::Error;

use crate::credential_codec::StoredNodeCredential;
use crate::hysteria2::{Hysteria2Credential, Hysteria2ObfuscationMethod};
use crate::vmess::{VmessCredential, VmessSecurity};
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
        };
        // Only VLESS and Trojan read a `type` parameter. Shadowsocks rejects
        // unknown parameters outright, and Hysteria2 carries its own QUIC
        // transport, so a missing transport is expected for both.
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
        // Shadowsocks has no TLS layer of its own in this model.
        if let Some(tls) = node.tls.as_ref() {
            if node.protocol_type != ProxyProtocol::Shadowsocks {
                write_tls(node.protocol_type, tls, &mut query);
            }
        }

        let mut link = format!(
            "{}://{authority}@{}:{}",
            scheme(node.protocol_type),
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

const fn scheme(protocol: ProxyProtocol) -> &'static str {
    match protocol {
        ProxyProtocol::Vless => "vless",
        ProxyProtocol::Vmess => "vmess",
        ProxyProtocol::Trojan => "trojan",
        ProxyProtocol::Shadowsocks => "ss",
        ProxyProtocol::Hysteria2 => "hysteria2",
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
    match transport {
        TransportConfig::Tcp => document["net"] = Value::String("tcp".to_owned()),
        TransportConfig::WebSocket { path, host } => {
            document["net"] = Value::String("ws".to_owned());
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
    }
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
            // Hysteria2 is TLS by definition and its parser rejects `security`.
            if protocol != ProxyProtocol::Hysteria2 {
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
}
