//! Subscription URL lists and subconverter wrapping.

use std::str;

use magies_storage::SecretValue;
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use thiserror::Error;
use url::Url;

/// Matches JavaScript `encodeURIComponent`, which subconverter clients expect.
const SUBCONVERTER_URL: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

/// Splits a stored subscription URL secret into individual fetch targets.
///
/// Lines and `|` separators both work, matching v2rayN's multi-URL input.
#[must_use]
pub fn split_subscription_urls(raw: &str) -> Vec<String> {
    raw.split(['\n', '|'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Wraps one source URL through a subconverter `/sub` endpoint.
///
/// # Errors
///
/// Returns a typed error when `base` is not a valid HTTP(S) URL.
pub fn wrap_subconverter(source: &str, base: &str) -> Result<String, SubscriptionUrlError> {
    let base = base.trim();
    if base.is_empty() {
        return Ok(source.to_owned());
    }
    validate_http_url(base)?;
    let separator = if base.contains('?') { '&' } else { '?' };
    let encoded = utf8_percent_encode(source, SUBCONVERTER_URL);
    Ok(format!("{base}{separator}target=v2ray&url={encoded}"))
}

/// Builds the HTTP(S) URLs the fetcher should request.
///
/// # Errors
///
/// Returns a typed error when any URL is invalid.
pub fn effective_fetch_urls(
    sources: &[String],
    subconverter: Option<&str>,
) -> Result<Vec<String>, SubscriptionUrlError> {
    let subconverter = subconverter
        .map(str::trim)
        .filter(|value| !value.is_empty());
    sources
        .iter()
        .map(|source| {
            validate_http_url(source)?;
            if let Some(base) = subconverter {
                wrap_subconverter(source, base)
            } else {
                Ok(source.clone())
            }
        })
        .collect()
}

/// Validates one or more subscription URLs and stores them as a newline secret.
///
/// # Errors
///
/// Returns a typed error when the list is empty or any URL is invalid.
pub fn validated_url_secret(raw: &str) -> Result<SecretValue, SubscriptionUrlError> {
    let urls = split_subscription_urls(raw);
    if urls.is_empty() {
        return Err(SubscriptionUrlError::Empty);
    }
    for url in &urls {
        validate_http_url(url)?;
    }
    let normalized = urls.join("\n");
    SecretValue::new(normalized.into_bytes())
        .map_err(|source| SubscriptionUrlError::Secret { source })
}

fn validate_http_url(value: &str) -> Result<(), SubscriptionUrlError> {
    let parsed = Url::parse(value).map_err(|source| SubscriptionUrlError::InvalidUrl { source })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(SubscriptionUrlError::UnsupportedScheme {
            scheme: parsed.scheme().to_owned(),
        });
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum SubscriptionUrlError {
    #[error("subscription URL list cannot be empty")]
    Empty,
    #[error("invalid subscription URL")]
    InvalidUrl { source: url::ParseError },
    #[error("unsupported subscription URL scheme: {scheme}")]
    UnsupportedScheme { scheme: String },
    #[error("failed to store the subscription URL secret")]
    Secret {
        source: magies_storage::SecretStoreError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_lines_and_pipe_separators() {
        assert_eq!(
            split_subscription_urls("https://a.example\nhttps://b.example|https://c.example"),
            vec![
                "https://a.example".to_owned(),
                "https://b.example".to_owned(),
                "https://c.example".to_owned(),
            ]
        );
    }

    #[test]
    fn wraps_subconverter_urls() {
        let wrapped =
            wrap_subconverter("https://example.com/sub", "http://127.0.0.1:25500/sub").unwrap();
        assert!(wrapped.starts_with("http://127.0.0.1:25500/sub?target=v2ray&url="));
        assert!(wrapped.contains("https%3A%2F%2Fexample.com%2Fsub"));
    }

    #[test]
    fn rejects_empty_and_invalid_urls() {
        assert!(matches!(
            validated_url_secret(""),
            Err(SubscriptionUrlError::Empty)
        ));
        assert!(matches!(
            validated_url_secret("not-a-url"),
            Err(SubscriptionUrlError::InvalidUrl { .. })
        ));
    }
}
