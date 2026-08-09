use std::fmt::{Debug, Formatter};
use std::time::Duration;

use reqwest::header::{
    ETAG, HeaderMap, HeaderValue, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED,
};
use reqwest::redirect::Policy;
use url::Url;

const USER_AGENT: &str = concat!("MgClash/", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubscriptionFetchOptions {
    pub timeout: Duration,
    pub max_body_bytes: usize,
    pub max_redirects: usize,
}

impl Default for SubscriptionFetchOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(15),
            max_body_bytes: 8 * 1024 * 1024,
            max_redirects: 5,
        }
    }
}

#[derive(Debug)]
pub struct SubscriptionFetcher {
    client: reqwest::Client,
    max_body_bytes: usize,
}

impl SubscriptionFetcher {
    /// Creates a fetcher with bounded request and response limits.
    ///
    /// # Errors
    ///
    /// Returns a typed error when a limit is zero or the HTTP client cannot be built.
    pub fn new(options: SubscriptionFetchOptions) -> Result<Self, SubscriptionFetchError> {
        if options.timeout.is_zero() {
            return Err(SubscriptionFetchError::InvalidTimeout);
        }
        if options.max_body_bytes == 0 {
            return Err(SubscriptionFetchError::InvalidBodyLimit);
        }

        ensure_crypto_provider();
        let redirect_policy = if options.max_redirects == 0 {
            Policy::none()
        } else {
            Policy::limited(options.max_redirects)
        };
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(options.timeout)
            .redirect(redirect_policy)
            .build()
            .map_err(|source| SubscriptionFetchError::ClientBuildFailed {
                source: source.without_url(),
            })?;

        Ok(Self {
            client,
            max_body_bytes: options.max_body_bytes,
        })
    }

    /// Fetches subscription bytes without parsing or persisting them.
    ///
    /// # Errors
    ///
    /// Returns a typed error for invalid input, request failures, rejected HTTP
    /// status codes, invalid validators, or responses exceeding the configured limit.
    pub async fn fetch(
        &self,
        url: &str,
        validators: Option<&SubscriptionValidators>,
    ) -> Result<SubscriptionFetchResult, SubscriptionFetchError> {
        let url =
            Url::parse(url).map_err(|source| SubscriptionFetchError::InvalidUrl { source })?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(SubscriptionFetchError::UnsupportedScheme {
                scheme: url.scheme().to_owned(),
            });
        }

        let mut request = self.client.get(url);
        if let Some(validators) = validators {
            if let Some(etag) = validators.etag() {
                request = request.header(IF_NONE_MATCH, request_header(etag, "If-None-Match")?);
            }
            if let Some(last_modified) = validators.last_modified() {
                request = request.header(
                    IF_MODIFIED_SINCE,
                    request_header(last_modified, "If-Modified-Since")?,
                );
            }
        }

        let mut response = request.send().await.map_err(map_request_error)?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_MODIFIED {
            let response_validators = response_validators(response.headers())?;
            return Ok(SubscriptionFetchResult::NotModified {
                validators: response_validators.with_fallback(validators),
            });
        }
        if !status.is_success() {
            return Err(SubscriptionFetchError::HttpStatus {
                status: status.as_u16(),
            });
        }

        let validators = response_validators(response.headers())?;
        let content_length = response.content_length();
        let max_body_bytes = u64::try_from(self.max_body_bytes).unwrap_or(u64::MAX);
        if content_length.is_some_and(|length| length > max_body_bytes) {
            return Err(SubscriptionFetchError::ResponseTooLarge {
                max_bytes: self.max_body_bytes,
            });
        }

        let initial_capacity = content_length
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or_default();
        let mut content = Vec::with_capacity(initial_capacity);
        while let Some(chunk) = response.chunk().await.map_err(map_request_error)? {
            content
                .len()
                .checked_add(chunk.len())
                .filter(|length| *length <= self.max_body_bytes)
                .ok_or(SubscriptionFetchError::ResponseTooLarge {
                    max_bytes: self.max_body_bytes,
                })?;
            content.extend_from_slice(&chunk);
        }

        Ok(SubscriptionFetchResult::Updated {
            content,
            validators,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SubscriptionValidators {
    etag: Option<String>,
    last_modified: Option<String>,
}

impl SubscriptionValidators {
    #[must_use]
    pub const fn new(etag: Option<String>, last_modified: Option<String>) -> Self {
        Self {
            etag,
            last_modified,
        }
    }

    #[must_use]
    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    #[must_use]
    pub fn last_modified(&self) -> Option<&str> {
        self.last_modified.as_deref()
    }

    fn with_fallback(self, fallback: Option<&Self>) -> Self {
        Self {
            etag: self
                .etag
                .or_else(|| fallback.and_then(|value| value.etag.clone())),
            last_modified: self
                .last_modified
                .or_else(|| fallback.and_then(|value| value.last_modified.clone())),
        }
    }
}

pub enum SubscriptionFetchResult {
    Updated {
        content: Vec<u8>,
        validators: SubscriptionValidators,
    },
    NotModified {
        validators: SubscriptionValidators,
    },
}

impl SubscriptionFetchResult {
    #[must_use]
    pub const fn is_not_modified(&self) -> bool {
        matches!(self, Self::NotModified { .. })
    }

    #[must_use]
    pub fn content(&self) -> Option<&[u8]> {
        match self {
            Self::Updated { content, .. } => Some(content),
            Self::NotModified { .. } => None,
        }
    }

    #[must_use]
    pub const fn validators(&self) -> &SubscriptionValidators {
        match self {
            Self::Updated { validators, .. } | Self::NotModified { validators } => validators,
        }
    }
}

impl Debug for SubscriptionFetchResult {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Updated {
                content,
                validators,
            } => formatter
                .debug_struct("Updated")
                .field("content", &"[REDACTED]")
                .field("content_len", &content.len())
                .field("validators", validators)
                .finish(),
            Self::NotModified { validators } => formatter
                .debug_struct("NotModified")
                .field("validators", validators)
                .finish(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SubscriptionFetchError {
    #[error("subscription request timeout must be greater than zero")]
    InvalidTimeout,
    #[error("subscription response size limit must be greater than zero")]
    InvalidBodyLimit,
    #[error("failed to build subscription HTTP client")]
    ClientBuildFailed { source: reqwest::Error },
    #[error("invalid subscription URL")]
    InvalidUrl { source: url::ParseError },
    #[error("unsupported subscription URL scheme: {scheme}")]
    UnsupportedScheme { scheme: String },
    #[error("invalid subscription request header: {name}")]
    InvalidRequestHeader { name: &'static str },
    #[error("subscription request timed out")]
    TimedOut,
    #[error("subscription redirect limit exceeded")]
    RedirectLimitExceeded,
    #[error("subscription request failed")]
    RequestFailed { source: reqwest::Error },
    #[error("subscription server returned HTTP {status}")]
    HttpStatus { status: u16 },
    #[error("subscription response exceeds {max_bytes} bytes")]
    ResponseTooLarge { max_bytes: usize },
    #[error("invalid subscription response header: {name}")]
    InvalidResponseHeader { name: &'static str },
}

fn request_header(value: &str, name: &'static str) -> Result<HeaderValue, SubscriptionFetchError> {
    HeaderValue::from_str(value).map_err(|_| SubscriptionFetchError::InvalidRequestHeader { name })
}

fn response_validators(
    headers: &HeaderMap,
) -> Result<SubscriptionValidators, SubscriptionFetchError> {
    Ok(SubscriptionValidators::new(
        response_header(headers, ETAG, "ETag")?,
        response_header(headers, LAST_MODIFIED, "Last-Modified")?,
    ))
}

fn response_header(
    headers: &HeaderMap,
    header_name: reqwest::header::HeaderName,
    display_name: &'static str,
) -> Result<Option<String>, SubscriptionFetchError> {
    headers
        .get(header_name)
        .map(|value| {
            value
                .to_str()
                .map(str::to_owned)
                .map_err(|_| SubscriptionFetchError::InvalidResponseHeader { name: display_name })
        })
        .transpose()
}

fn map_request_error(source: reqwest::Error) -> SubscriptionFetchError {
    if source.is_timeout() {
        SubscriptionFetchError::TimedOut
    } else if source.is_redirect() {
        SubscriptionFetchError::RedirectLimitExceeded
    } else {
        SubscriptionFetchError::RequestFailed {
            source: source.without_url(),
        }
    }
}

fn ensure_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let provider = rustls::crypto::ring::default_provider();
        if let Err(unused_provider) = provider.install_default() {
            // Another thread installed the process-wide provider first.
            drop(unused_provider);
        }
    }
}
