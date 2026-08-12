//! Decodes the two envelopes a list of sharing links arrives in.
//!
//! Both subscription bodies and text the user pastes or opens from a file use
//! the same shapes: one link per line, or the whole body `Base64`-encoded.

use std::borrow::Cow;
use std::str;

use base64::{Engine as _, engine::general_purpose};
use thiserror::Error;

/// Returns the plain text of a node list, decoding a whole-body `Base64`
/// envelope when the content holds no bare sharing link.
///
/// # Errors
///
/// Returns a typed error when the content is empty, is not valid UTF-8, or
/// looks like `Base64` but cannot be decoded.
pub(crate) fn decode_node_list(content: &[u8]) -> Result<Cow<'_, str>, NodeListTextError> {
    let text =
        str::from_utf8(content).map_err(|source| NodeListTextError::InvalidUtf8 { source })?;
    let trimmed = text.trim().trim_start_matches('\u{feff}');
    if trimmed.is_empty() {
        return Err(NodeListTextError::Empty);
    }
    // A bare scheme anywhere means the body is already plain text.
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
        .map_err(|source| NodeListTextError::InvalidBase64 { source })?;
    let decoded =
        str::from_utf8(&decoded).map_err(|source| NodeListTextError::InvalidUtf8 { source })?;
    let decoded = decoded.trim().trim_start_matches('\u{feff}');
    if decoded.is_empty() {
        return Err(NodeListTextError::Empty);
    }
    Ok(Cow::Owned(decoded.to_owned()))
}

#[derive(Debug, Error)]
pub(crate) enum NodeListTextError {
    #[error("node list is empty")]
    Empty,
    #[error("node list is not valid Base64")]
    InvalidBase64 { source: base64::DecodeError },
    #[error("node list is not valid UTF-8")]
    InvalidUtf8 { source: str::Utf8Error },
}
