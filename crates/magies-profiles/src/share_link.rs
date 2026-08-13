use magies_domain::{CredentialRef, NodeModelError, ProxyNode};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    Hysteria2ParseError, Hysteria2Parser, ShadowsocksParseError, ShadowsocksParser,
    StoredNodeCredential, TrojanParseError, TrojanParser, TuicParseError, TuicParser,
    VlessParseError, VlessParser, VmessParseError, VmessParser,
};

/// A sharing URI resolved into the shared node model and its owned credential.
#[derive(Debug)]
pub struct ParsedShareLink {
    node: ProxyNode,
    credential: StoredNodeCredential,
}

impl ParsedShareLink {
    #[must_use]
    pub const fn node(&self) -> &ProxyNode {
        &self.node
    }

    #[must_use]
    pub const fn credential(&self) -> &StoredNodeCredential {
        &self.credential
    }

    #[must_use]
    pub fn into_parts(self) -> (ProxyNode, StoredNodeCredential) {
        (self.node, self.credential)
    }
}

/// Routes a sharing URI to the parser that owns its scheme.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ShareLinkParser;

impl ShareLinkParser {
    /// Parses any supported P0 sharing URI without persisting its credential.
    ///
    /// # Errors
    ///
    /// Returns [`ShareLinkParseError::UnsupportedScheme`] when no parser claims
    /// the URI, the selected parser's typed error, or the shared model's
    /// validation error.
    pub fn parse(
        &self,
        value: &str,
        id: Uuid,
        credential_ref: CredentialRef,
    ) -> Result<ParsedShareLink, ShareLinkParseError> {
        macro_rules! dispatch {
            ($($parser:expr => $variant:ident),+ $(,)?) => {
                $(if $parser.can_parse(value) {
                    let parsed = $parser.parse(value).map_err(ShareLinkParseError::$variant)?;
                    let credential = StoredNodeCredential::from(parsed.credential());
                    let node = parsed
                        .into_proxy_node(id, credential_ref)
                        .map_err(ShareLinkParseError::Node)?;
                    return Ok(ParsedShareLink { node, credential });
                })+
            };
        }

        dispatch! {
            VlessParser => Vless,
            VmessParser => Vmess,
            TrojanParser => Trojan,
            ShadowsocksParser => Shadowsocks,
            Hysteria2Parser => Hysteria2,
            TuicParser => Tuic,
        }
        Err(ShareLinkParseError::UnsupportedScheme)
    }
}

#[derive(Debug, Error)]
pub enum ShareLinkParseError {
    #[error("sharing URI does not use a supported P0 scheme")]
    UnsupportedScheme,
    #[error("failed to parse VLESS sharing URI")]
    Vless(#[source] VlessParseError),
    #[error("failed to parse VMess sharing URI")]
    Vmess(#[source] VmessParseError),
    #[error("failed to parse Trojan sharing URI")]
    Trojan(#[source] TrojanParseError),
    #[error("failed to parse Shadowsocks sharing URI")]
    Shadowsocks(#[source] ShadowsocksParseError),
    #[error("failed to parse Hysteria2 sharing URI")]
    Hysteria2(#[source] Hysteria2ParseError),
    #[error("failed to parse TUIC sharing URI")]
    Tuic(#[source] TuicParseError),
    #[error("failed to build the shared node model")]
    Node(#[source] NodeModelError),
}
