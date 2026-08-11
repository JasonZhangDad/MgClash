use std::borrow::Cow;
use std::str;

use base64::{Engine as _, engine::general_purpose};
use magies_domain::{CredentialRef, NodeModelError, ProxyNode};
use thiserror::Error;
use uuid::Uuid;

use crate::{ShareLinkParseError, ShareLinkParser, StoredNodeCredential};

/// One fully parsed subscription node before its credential is persisted.
#[derive(Debug)]
pub struct ParsedSubscriptionNode {
    node: ProxyNode,
    credential: StoredNodeCredential,
}

impl ParsedSubscriptionNode {
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

/// Parses the two common sharing-link subscription envelopes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SubscriptionContentParser;

impl SubscriptionContentParser {
    /// Parses a plain line-separated or whole-body `Base64` subscription.
    ///
    /// # Errors
    ///
    /// Returns a typed error for empty, malformed, non-UTF-8, or partially
    /// invalid content. No partial node collection is returned.
    pub fn parse(
        &self,
        content: &[u8],
        subscription_id: Uuid,
    ) -> Result<Vec<ParsedSubscriptionNode>, SubscriptionContentError> {
        let text = subscription_text(content)?;
        let mut nodes = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let value = line.trim().trim_start_matches('\u{feff}');
            if value.is_empty() {
                continue;
            }

            let line = index + 1;
            let id = Uuid::new_v4();
            let credential_ref =
                CredentialRef::new(format!("subscription/{subscription_id}/node/{id}"))
                    .map_err(|source| SubscriptionContentError::CredentialRef { line, source })?;
            let parsed = ShareLinkParser
                .parse(value, id, credential_ref)
                .map_err(|source| SubscriptionContentError::InvalidLine { line, source })?;
            let (mut node, credential) = parsed.into_parts();
            node.subscription_id = Some(subscription_id);
            nodes.push(ParsedSubscriptionNode { node, credential });
        }

        if nodes.is_empty() {
            return Err(SubscriptionContentError::Empty);
        }
        Ok(nodes)
    }
}

fn subscription_text(content: &[u8]) -> Result<Cow<'_, str>, SubscriptionContentError> {
    let text = str::from_utf8(content)
        .map_err(|source| SubscriptionContentError::InvalidUtf8 { source })?;
    let trimmed = text.trim().trim_start_matches('\u{feff}');
    if trimmed.is_empty() {
        return Err(SubscriptionContentError::Empty);
    }
    if trimmed.lines().any(|line| line.trim().contains("://")) {
        return Ok(Cow::Borrowed(trimmed));
    }

    let mut encoded: String = trimmed
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let missing_padding = (4 - encoded.len() % 4) % 4;
    encoded.extend(std::iter::repeat_n('=', missing_padding));
    let decoded = general_purpose::STANDARD
        .decode(&encoded)
        .or_else(|_| general_purpose::URL_SAFE.decode(&encoded))
        .map_err(|source| SubscriptionContentError::InvalidBase64 { source })?;
    let decoded = str::from_utf8(&decoded)
        .map_err(|source| SubscriptionContentError::InvalidUtf8 { source })?;
    let decoded = decoded.trim().trim_start_matches('\u{feff}');
    if decoded.is_empty() {
        return Err(SubscriptionContentError::Empty);
    }
    Ok(Cow::Owned(decoded.to_owned()))
}

#[derive(Debug, Error)]
pub enum SubscriptionContentError {
    #[error("subscription contains no nodes")]
    Empty,
    #[error("subscription body is not valid Base64")]
    InvalidBase64 { source: base64::DecodeError },
    #[error("subscription body is not valid UTF-8")]
    InvalidUtf8 { source: str::Utf8Error },
    #[error("failed to build the credential reference for subscription line {line}")]
    CredentialRef { line: usize, source: NodeModelError },
    #[error("subscription line {line} is invalid")]
    InvalidLine {
        line: usize,
        source: ShareLinkParseError,
    },
}
