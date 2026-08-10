use std::fmt::{Debug, Formatter};
use std::num::NonZeroU16;

use base64::{Engine as _, engine::general_purpose};
use magies_domain::{
    CredentialRef, NodeModelError, NodeName, ProxyNode, ProxyProtocol, ServerAddress,
    TransportConfig,
};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use super::{
    QueryParameters, VlessParseError, decode_component, default_name, parse_port, parse_server,
    validate_percent_encoding,
};

const SUPPORTED_METHODS: &[&str] = &[
    "2022-blake3-aes-128-gcm",
    "2022-blake3-aes-256-gcm",
    "2022-blake3-chacha20-poly1305",
    "none",
    "plain",
    "aes-128-gcm",
    "aes-192-gcm",
    "aes-256-gcm",
    "chacha20-poly1305",
    "chacha20-ietf-poly1305",
    "xchacha20-poly1305",
    "xchacha20-ietf-poly1305",
    "aes-128-ctr",
    "aes-192-ctr",
    "aes-256-ctr",
    "aes-128-cfb",
    "aes-192-cfb",
    "aes-256-cfb",
    "rc4-md5",
    "chacha20-ietf",
    "xchacha20",
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ShadowsocksParser;

impl ShadowsocksParser {
    #[must_use]
    pub fn can_parse(&self, value: &str) -> bool {
        value.trim().starts_with("ss://")
    }

    /// Parses a Shadowsocks sharing URI without persisting its password.
    ///
    /// Both current SIP002 user-info forms and v2rayN's legacy whole-payload
    /// Base64 form are accepted.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the URI is malformed, uses an unsupported
    /// cipher, contains plugin settings, or cannot fit the shared node model.
    pub fn parse(&self, value: &str) -> Result<ParsedShadowsocksNode, ShadowsocksParseError> {
        let value = value.trim();
        if !self.can_parse(value) {
            return Err(ShadowsocksParseError::UnsupportedScheme);
        }
        let Some(payload) = value.strip_prefix("ss://") else {
            return Err(ShadowsocksParseError::UnsupportedScheme);
        };
        if payload.is_empty() {
            return Err(ShadowsocksParseError::EmptyPayload);
        }
        validate_percent_encoding(value).map_err(invalid_uri)?;

        let before_fragment = payload.split_once('#').map_or(payload, |(value, _)| value);
        if before_fragment.contains('@') {
            parse_sip002(value)
        } else {
            parse_legacy(payload)
        }
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct ShadowsocksCredential {
    method: String,
    password: String,
}

impl ShadowsocksCredential {
    #[must_use]
    pub fn method(&self) -> &str {
        &self.method
    }

    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }
}

impl Debug for ShadowsocksCredential {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ShadowsocksCredential([REDACTED])")
    }
}

#[derive(Debug)]
pub struct ParsedShadowsocksNode {
    name: NodeName,
    server: ServerAddress,
    port: NonZeroU16,
    credential: ShadowsocksCredential,
}

impl ParsedShadowsocksNode {
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
    pub const fn credential(&self) -> &ShadowsocksCredential {
        &self.credential
    }

    #[must_use]
    pub const fn transport(&self) -> &TransportConfig {
        &TransportConfig::Tcp
    }

    #[must_use]
    pub const fn tls(&self) -> Option<&magies_domain::TlsConfig> {
        None
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
            ProxyProtocol::Shadowsocks,
            self.server.as_str(),
            u32::from(self.port.get()),
            Some(credential_ref),
        )?;
        node.transport = Some(TransportConfig::Tcp);
        Ok(node)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ShadowsocksParseError {
    #[error("URI scheme must be exactly ss")]
    UnsupportedScheme,
    #[error("Shadowsocks URI payload is required")]
    EmptyPayload,
    #[error("malformed Shadowsocks URI")]
    MalformedUrl { source: url::ParseError },
    #[error("Shadowsocks URI path must be empty or /")]
    UnexpectedPath,
    #[error("invalid Shadowsocks Base64 payload")]
    InvalidBase64 { source: base64::DecodeError },
    #[error("Shadowsocks credentials are not valid UTF-8")]
    InvalidUtf8,
    #[error("Shadowsocks credentials must contain method:password")]
    MissingCredentialSeparator,
    #[error("Shadowsocks cipher method is required")]
    MissingMethod,
    #[error("Shadowsocks password is required")]
    MissingPassword,
    #[error("unsupported Shadowsocks cipher method: {value}")]
    UnsupportedMethod { value: String },
    #[error("legacy Shadowsocks payload must contain credentials@endpoint")]
    MissingEndpointSeparator,
    #[error("invalid Shadowsocks URI field")]
    InvalidUri { source: VlessParseError },
    #[error("Shadowsocks plugins are not supported by the shared node model")]
    UnsupportedPlugin,
    #[error("invalid parsed Shadowsocks node")]
    InvalidNode { source: NodeModelError },
}

fn parse_sip002(value: &str) -> Result<ParsedShadowsocksNode, ShadowsocksParseError> {
    let url = Url::parse(value).map_err(|source| ShadowsocksParseError::MalformedUrl { source })?;
    if !matches!(url.path(), "" | "/") {
        return Err(ShadowsocksParseError::UnexpectedPath);
    }
    let server = parse_server(&url).map_err(invalid_uri)?;
    let port = parse_port(&url).map_err(invalid_uri)?;

    let mut parameters = QueryParameters::parse(&url).map_err(invalid_uri)?;
    if parameters.take("plugin").is_some() {
        return Err(ShadowsocksParseError::UnsupportedPlugin);
    }
    parameters.finish().map_err(invalid_uri)?;

    let credentials = if let Some(raw_password) = url.password() {
        let method = decode_component(url.username()).map_err(invalid_uri)?;
        let password = decode_component(raw_password).map_err(invalid_uri)?;
        validate_credentials(method, password)?
    } else {
        let user_info = decode_component(url.username()).map_err(invalid_uri)?;
        parse_encoded_credentials(&user_info)?
    };
    build_node(&url, server, port, credentials)
}

fn parse_legacy(payload: &str) -> Result<ParsedShadowsocksNode, ShadowsocksParseError> {
    let (encoded_payload, fragment) = payload
        .split_once('#')
        .map_or((payload, None), |(encoded, fragment)| {
            (encoded, Some(fragment))
        });
    let decoded = decode_base64(encoded_payload)?;
    let decoded = String::from_utf8(decoded).map_err(|_| ShadowsocksParseError::InvalidUtf8)?;
    let (raw_credentials, endpoint) = decoded
        .rsplit_once('@')
        .ok_or(ShadowsocksParseError::MissingEndpointSeparator)?;
    let credentials = parse_credentials(raw_credentials)?;

    let endpoint_url = Url::parse(&format!("ss://placeholder@{endpoint}"))
        .map_err(|source| ShadowsocksParseError::MalformedUrl { source })?;
    if !endpoint_url.path().is_empty() {
        return Err(ShadowsocksParseError::UnexpectedPath);
    }
    let server = parse_server(&endpoint_url).map_err(invalid_uri)?;
    let port = parse_port(&endpoint_url).map_err(invalid_uri)?;
    let fallback_name = default_name(&server, port);
    let name = decode_name(fragment, fallback_name)?;

    Ok(ParsedShadowsocksNode {
        name,
        server,
        port,
        credential: credentials,
    })
}

fn build_node(
    url: &Url,
    server: ServerAddress,
    port: NonZeroU16,
    credential: ShadowsocksCredential,
) -> Result<ParsedShadowsocksNode, ShadowsocksParseError> {
    let fallback_name = default_name(&server, port);
    let name = decode_name(url.fragment(), fallback_name)?;
    Ok(ParsedShadowsocksNode {
        name,
        server,
        port,
        credential,
    })
}

fn decode_name(
    fragment: Option<&str>,
    fallback: String,
) -> Result<NodeName, ShadowsocksParseError> {
    let name = match fragment {
        Some(fragment) => {
            let decoded = decode_component(fragment).map_err(invalid_uri)?;
            if decoded.is_empty() {
                fallback
            } else {
                decoded
            }
        }
        None => fallback,
    };
    NodeName::new(name).map_err(|source| ShadowsocksParseError::InvalidNode { source })
}

fn parse_encoded_credentials(
    encoded: &str,
) -> Result<ShadowsocksCredential, ShadowsocksParseError> {
    let decoded = decode_base64(encoded)?;
    let decoded = String::from_utf8(decoded).map_err(|_| ShadowsocksParseError::InvalidUtf8)?;
    parse_credentials(&decoded)
}

fn decode_base64(value: &str) -> Result<Vec<u8>, ShadowsocksParseError> {
    let mut padded = value.to_owned();
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    general_purpose::STANDARD
        .decode(&padded)
        .or_else(|_| general_purpose::URL_SAFE.decode(&padded))
        .map_err(|source| ShadowsocksParseError::InvalidBase64 { source })
}

fn parse_credentials(value: &str) -> Result<ShadowsocksCredential, ShadowsocksParseError> {
    let (method, password) = value
        .split_once(':')
        .ok_or(ShadowsocksParseError::MissingCredentialSeparator)?;
    validate_credentials(method.to_owned(), password.to_owned())
}

fn validate_credentials(
    method: String,
    password: String,
) -> Result<ShadowsocksCredential, ShadowsocksParseError> {
    if method.is_empty() {
        return Err(ShadowsocksParseError::MissingMethod);
    }
    if password.is_empty() {
        return Err(ShadowsocksParseError::MissingPassword);
    }
    if !SUPPORTED_METHODS.contains(&method.as_str()) {
        return Err(ShadowsocksParseError::UnsupportedMethod { value: method });
    }
    Ok(ShadowsocksCredential { method, password })
}

fn invalid_uri(source: VlessParseError) -> ShadowsocksParseError {
    ShadowsocksParseError::InvalidUri { source }
}
