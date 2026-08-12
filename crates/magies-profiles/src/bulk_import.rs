//! Parses many sharing links at once into manual nodes.
//!
//! This is the pasted-text and opened-file counterpart to
//! [`crate::SubscriptionContentParser`]. The two differ on purpose:
//!
//! - a subscription is a snapshot, so one bad line invalidates the whole body;
//!   pasted text is a user's own list, so bad lines are reported and the rest
//!   still import
//! - subscription nodes are read-only and owned by their subscription; nodes
//!   parsed here are manual, so they stay editable and deletable

use std::collections::HashSet;

use magies_domain::{CredentialRef, NodeModelError, ProxyNode};
use thiserror::Error;
use uuid::Uuid;

use crate::node_dedup::{CredentialIdentity, NodeDedupCandidate, NodeFingerprint};
use crate::node_list_text::{NodeListTextError, decode_node_list};
use crate::{CredentialCodec, ShareLinkParseError, ShareLinkParser, StoredNodeCredential};

/// Parses a whole list of sharing links into manual nodes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BulkNodeImportParser;

impl BulkNodeImportParser {
    /// Parses every line, keeping the ones that succeed.
    ///
    /// Lines that repeat a node already seen earlier in the same body are
    /// dropped: they are exact duplicates down to the credential, so keeping
    /// them would only produce indistinguishable rows.
    ///
    /// # Errors
    ///
    /// Returns a typed error only when the body as a whole cannot be read —
    /// empty, not UTF-8, or an undecodable `Base64` envelope. A body where
    /// every line fails to parse still succeeds, with the failures reported.
    pub fn parse(&self, content: &[u8]) -> Result<BulkImportOutcome, BulkImportError> {
        let text = decode_node_list(content)?;
        let mut nodes = Vec::new();
        let mut failures = Vec::new();
        let mut seen: HashSet<NodeFingerprint> = HashSet::new();
        let mut duplicates = 0;

        for (index, line) in text.lines().enumerate() {
            let value = line.trim().trim_start_matches('\u{feff}');
            if value.is_empty() {
                continue;
            }

            let line = index + 1;
            match parse_line(value, line) {
                Ok((node, credential)) => {
                    let Some(fingerprint) = fingerprint(&node, &credential) else {
                        // An unencodable credential cannot be persisted either,
                        // so report the line rather than importing a node whose
                        // secret would fail to save.
                        failures.push(BulkImportFailure {
                            line,
                            reason: BulkImportLineError::UnreadableCredential,
                        });
                        continue;
                    };
                    if !seen.insert(fingerprint) {
                        duplicates += 1;
                        continue;
                    }
                    nodes.push(ParsedBulkNode { node, credential });
                }
                Err(failure) => failures.push(failure),
            }
        }

        Ok(BulkImportOutcome {
            nodes,
            failures,
            duplicates,
        })
    }
}

fn parse_line(
    value: &str,
    line: usize,
) -> Result<(ProxyNode, StoredNodeCredential), BulkImportFailure> {
    let id = Uuid::new_v4();
    let credential_ref =
        CredentialRef::new(format!("node/{id}")).map_err(|source| BulkImportFailure {
            line,
            reason: BulkImportLineError::CredentialRef { source },
        })?;
    let parsed = ShareLinkParser
        .parse(value, id, credential_ref)
        .map_err(|source| BulkImportFailure {
            line,
            reason: BulkImportLineError::InvalidLink { source },
        })?;
    Ok(parsed.into_parts())
}

/// Derives the dedup fingerprint from the node and its encoded credential.
fn fingerprint(node: &ProxyNode, credential: &StoredNodeCredential) -> Option<NodeFingerprint> {
    let secret = CredentialCodec::encode(credential).ok()?;
    let identity = CredentialIdentity::from_bytes(secret.expose_secret());
    Some(NodeDedupCandidate::new(node.clone(), identity).fingerprint())
}

/// One parsed node and its credential, before either is persisted.
#[derive(Debug)]
pub struct ParsedBulkNode {
    node: ProxyNode,
    credential: StoredNodeCredential,
}

impl ParsedBulkNode {
    #[must_use]
    pub const fn node(&self) -> &ProxyNode {
        &self.node
    }

    #[must_use]
    pub fn into_parts(self) -> (ProxyNode, StoredNodeCredential) {
        (self.node, self.credential)
    }
}

/// What one body of pasted or opened text yielded.
#[derive(Debug)]
pub struct BulkImportOutcome {
    pub nodes: Vec<ParsedBulkNode>,
    pub failures: Vec<BulkImportFailure>,
    /// Lines dropped because an identical node appeared earlier in this body.
    pub duplicates: usize,
}

#[derive(Debug, Error)]
#[error("line {line} could not be imported")]
pub struct BulkImportFailure {
    pub line: usize,
    #[source]
    pub reason: BulkImportLineError,
}

#[derive(Debug, Error)]
pub enum BulkImportLineError {
    #[error("failed to build the credential reference")]
    CredentialRef {
        #[source]
        source: NodeModelError,
    },
    #[error("not a supported sharing link")]
    InvalidLink {
        #[source]
        source: ShareLinkParseError,
    },
    #[error("the credential could not be encoded")]
    UnreadableCredential,
}

#[derive(Debug, Error)]
pub enum BulkImportError {
    #[error("the pasted text contains no nodes")]
    Empty,
    /// Reached when the body holds no bare `scheme://` at all, so it was read
    /// as a `Base64` envelope and failed to decode.
    #[error("the text is neither sharing links nor a Base64 node list")]
    InvalidBase64 { source: base64::DecodeError },
    #[error("the pasted text is not valid UTF-8")]
    InvalidUtf8 { source: std::str::Utf8Error },
}

impl From<NodeListTextError> for BulkImportError {
    fn from(value: NodeListTextError) -> Self {
        match value {
            NodeListTextError::Empty => Self::Empty,
            NodeListTextError::InvalidBase64 { source } => Self::InvalidBase64 { source },
            NodeListTextError::InvalidUtf8 { source } => Self::InvalidUtf8 { source },
        }
    }
}
