use std::fmt::{Debug, Formatter};
use std::num::NonZeroU16;

use magies_domain::{
    CredentialRef, GrpcMode, NodeModelError, NodeName, ProxyNode, ProxyProtocol, ServerAddress,
    TlsConfig, TransportConfig,
};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use super::{
    QueryParameters, VlessParseError, decode_component, default_name, parse_port, parse_server,
    parse_tls_with_default, validate_percent_encoding,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TrojanParser;

impl TrojanParser {
    #[must_use]
    pub fn can_parse(&self, value: &str) -> bool {
        value.trim().starts_with("trojan://")
    }

    /// Parses a Trojan sharing URI without persisting its password.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the URI is malformed, contains unsupported
    /// fields, or cannot be represented by the shared node model.
    pub fn parse(&self, value: &str) -> Result<ParsedTrojanNode, TrojanParseError> {
        let value = value.trim();
        if !self.can_parse(value) {
            return Err(TrojanParseError::UnsupportedScheme);
        }
        validate_percent_encoding(value).map_err(invalid_uri)?;

        let url = Url::parse(value).map_err(|source| TrojanParseError::MalformedUrl { source })?;
        if !matches!(url.path(), "" | "/") {
            return Err(TrojanParseError::UnexpectedPath);
        }
        if url.password().is_some() {
            return Err(TrojanParseError::UnexpectedPasswordComponent);
        }

        let raw_password = url.username();
        if raw_password.is_empty() {
            return Err(TrojanParseError::MissingPassword);
        }
        let password = decode_component(raw_password).map_err(invalid_uri)?;
        let server = parse_server(&url).map_err(invalid_uri)?;
        let port = parse_port(&url).map_err(invalid_uri)?;

        let mut parameters = QueryParameters::parse(&url).map_err(invalid_uri)?;
        let flow = parameters.take("flow");
        let allow_insecure = parse_insecure_aliases(&mut parameters)?;
        let transport = parse_transport(&mut parameters)?;
        let mut tls =
            parse_tls_with_default(&mut parameters, &server, "tls").map_err(invalid_uri)?;
        parameters.finish().map_err(invalid_uri)?;
        validate_transport_security(&transport, tls.as_ref())?;
        apply_insecure(&mut tls, allow_insecure)?;

        let fallback_name = default_name(&server, port);
        let name = match url.fragment() {
            Some(fragment) => {
                let decoded = decode_component(fragment).map_err(invalid_uri)?;
                if decoded.is_empty() {
                    fallback_name
                } else {
                    decoded
                }
            }
            None => fallback_name,
        };
        let name =
            NodeName::new(name).map_err(|source| TrojanParseError::InvalidNode { source })?;

        Ok(ParsedTrojanNode {
            name,
            server,
            port,
            credential: TrojanCredential { password, flow },
            transport,
            tls,
        })
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct TrojanCredential {
    password: String,
    flow: Option<String>,
}

impl TrojanCredential {
    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }

    #[must_use]
    pub fn flow(&self) -> Option<&str> {
        self.flow.as_deref()
    }
}

impl Debug for TrojanCredential {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TrojanCredential([REDACTED])")
    }
}

#[derive(Debug)]
pub struct ParsedTrojanNode {
    name: NodeName,
    server: ServerAddress,
    port: NonZeroU16,
    credential: TrojanCredential,
    transport: TransportConfig,
    tls: Option<TlsConfig>,
}

impl ParsedTrojanNode {
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
    pub const fn credential(&self) -> &TrojanCredential {
        &self.credential
    }

    #[must_use]
    pub const fn transport(&self) -> &TransportConfig {
        &self.transport
    }

    #[must_use]
    pub const fn tls(&self) -> Option<&TlsConfig> {
        self.tls.as_ref()
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
            ProxyProtocol::Trojan,
            self.server.as_str(),
            u32::from(self.port.get()),
            Some(credential_ref),
        )?;
        node.transport = Some(self.transport);
        node.tls = self.tls;
        Ok(node)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TrojanParseError {
    #[error("URI scheme must be exactly trojan")]
    UnsupportedScheme,
    #[error("malformed Trojan URI")]
    MalformedUrl { source: url::ParseError },
    #[error("Trojan URI path must be empty or /")]
    UnexpectedPath,
    #[error("Trojan password must be encoded as URI user information")]
    UnexpectedPasswordComponent,
    #[error("Trojan password is required")]
    MissingPassword,
    #[error("invalid Trojan URI field")]
    InvalidUri { source: VlessParseError },
    #[error("unsupported Trojan transport: {value}")]
    UnsupportedTransport { value: String },
    #[error("unsupported Trojan TCP header: {value}")]
    UnsupportedTcpHeader { value: String },
    #[error("unsupported Trojan gRPC mode: {value}")]
    UnsupportedGrpcMode { value: String },
    #[error("Trojan parameter {name} must be 0 or 1, got {value}")]
    InvalidBoolean { name: &'static str, value: String },
    #[error("Trojan insecure parameter aliases conflict")]
    ConflictingInsecureAliases,
    #[error("Trojan allowInsecure cannot be used with {security}")]
    UnsupportedInsecureSecurity { security: &'static str },
    #[error("Trojan transport {transport} cannot be used with {security}")]
    UnsupportedCombination {
        transport: &'static str,
        security: &'static str,
    },
    #[error("invalid parsed Trojan node")]
    InvalidNode { source: NodeModelError },
}

fn invalid_uri(source: VlessParseError) -> TrojanParseError {
    TrojanParseError::InvalidUri { source }
}

fn parse_transport(parameters: &mut QueryParameters) -> Result<TransportConfig, TrojanParseError> {
    let transport = parameters
        .take_non_empty("type")
        .map_err(invalid_uri)?
        .unwrap_or_else(|| "tcp".to_owned());
    match transport.as_str() {
        "tcp" | "raw" => {
            let header_type = parameters
                .take_non_empty("headerType")
                .map_err(invalid_uri)?
                .unwrap_or_else(|| "none".to_owned());
            if header_type == "none" {
                Ok(TransportConfig::Tcp)
            } else {
                Err(TrojanParseError::UnsupportedTcpHeader { value: header_type })
            }
        }
        "ws" => Ok(TransportConfig::WebSocket {
            path: parameters
                .take_non_empty("path")
                .map_err(invalid_uri)?
                .unwrap_or_else(|| "/".to_owned()),
            host: parameters.take("host"),
        }),
        "grpc" => {
            let service_name = parameters
                .take_required_non_empty("serviceName")
                .map_err(invalid_uri)?;
            let mode = match parameters
                .take_non_empty("mode")
                .map_err(invalid_uri)?
                .as_deref()
                .unwrap_or("gun")
            {
                "gun" => GrpcMode::Gun,
                "multi" => GrpcMode::Multi,
                "guna" => GrpcMode::Guna,
                value => {
                    return Err(TrojanParseError::UnsupportedGrpcMode {
                        value: value.to_owned(),
                    });
                }
            };
            Ok(TransportConfig::Grpc {
                service_name,
                mode,
                authority: parameters.take("authority"),
            })
        }
        value => Err(TrojanParseError::UnsupportedTransport {
            value: value.to_owned(),
        }),
    }
}

fn parse_insecure_aliases(
    parameters: &mut QueryParameters,
) -> Result<Option<bool>, TrojanParseError> {
    let allow_insecure =
        parse_boolean(parameters.take("allowInsecure").as_deref(), "allowInsecure")?;
    let insecure = parse_boolean(parameters.take("insecure").as_deref(), "insecure")?;
    match (allow_insecure, insecure) {
        (Some(first), Some(second)) if first != second => {
            Err(TrojanParseError::ConflictingInsecureAliases)
        }
        (Some(value), _) | (_, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
}

fn parse_boolean(
    value: Option<&str>,
    name: &'static str,
) -> Result<Option<bool>, TrojanParseError> {
    match value {
        None => Ok(None),
        Some("0") => Ok(Some(false)),
        Some("1") => Ok(Some(true)),
        Some(value) => Err(TrojanParseError::InvalidBoolean {
            name,
            value: value.to_owned(),
        }),
    }
}

fn validate_transport_security(
    transport: &TransportConfig,
    tls: Option<&TlsConfig>,
) -> Result<(), TrojanParseError> {
    if matches!(transport, TransportConfig::WebSocket { .. })
        && matches!(tls, Some(TlsConfig::Reality { .. }))
    {
        Err(TrojanParseError::UnsupportedCombination {
            transport: "websocket",
            security: "reality",
        })
    } else {
        Ok(())
    }
}

fn apply_insecure(
    tls: &mut Option<TlsConfig>,
    allow_insecure: Option<bool>,
) -> Result<(), TrojanParseError> {
    let Some(allow_insecure) = allow_insecure else {
        return Ok(());
    };
    match tls {
        Some(TlsConfig::Tls {
            allow_insecure: value,
            ..
        }) => {
            *value = allow_insecure;
            Ok(())
        }
        Some(TlsConfig::Reality { .. }) if allow_insecure => {
            Err(TrojanParseError::UnsupportedInsecureSecurity {
                security: "reality",
            })
        }
        None if allow_insecure => {
            Err(TrojanParseError::UnsupportedInsecureSecurity { security: "none" })
        }
        Some(TlsConfig::Reality { .. }) | None => Ok(()),
    }
}
