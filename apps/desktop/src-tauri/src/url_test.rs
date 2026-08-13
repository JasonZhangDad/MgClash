//! HTTP URL tests routed through the running desktop proxy session.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use magies_profiles::ensure_rustls_crypto_provider;
use reqwest::redirect::Policy;
use thiserror::Error;
use url::Url;

const USER_AGENT: &str = concat!("MgClash/", env!("CARGO_PKG_VERSION"));

/// Why an HTTP URL test could not produce a latency value.
#[derive(Debug, Error)]
pub enum UrlTestError {
    #[error("the URL test timeout must be greater than zero")]
    InvalidTimeout,
    #[error("the URL test address is invalid")]
    InvalidUrl(#[source] url::ParseError),
    #[error("unsupported URL test scheme: {scheme}")]
    UnsupportedScheme { scheme: String },
    #[error("the URL test HTTP client could not be built")]
    ClientBuild(#[source] reqwest::Error),
    #[error("the URL test timed out")]
    TimedOut,
    #[error("the URL test request failed")]
    Request(#[source] reqwest::Error),
    #[error("the URL test returned HTTP status {status}")]
    HttpStatus { status: u16 },
}

/// Requests `url` through `proxy_address` and returns time to a successful
/// response header in milliseconds.
///
/// # Errors
///
/// Returns a typed validation, client, request, timeout, or HTTP status error.
pub async fn probe_url(
    url: &str,
    proxy_address: SocketAddr,
    timeout: Duration,
) -> Result<u32, UrlTestError> {
    if timeout.is_zero() {
        return Err(UrlTestError::InvalidTimeout);
    }
    let url = Url::parse(url.trim()).map_err(UrlTestError::InvalidUrl)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(UrlTestError::UnsupportedScheme {
            scheme: url.scheme().to_owned(),
        });
    }

    ensure_rustls_crypto_provider();
    let proxy = reqwest::Proxy::all(format!("http://{proxy_address}"))
        .map_err(|source| UrlTestError::ClientBuild(source.without_url()))?;
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(timeout)
        .redirect(Policy::limited(3))
        .proxy(proxy)
        .build()
        .map_err(|source| UrlTestError::ClientBuild(source.without_url()))?;

    let started = Instant::now();
    let response = client.get(url).send().await.map_err(|source| {
        if source.is_timeout() {
            UrlTestError::TimedOut
        } else {
            UrlTestError::Request(source.without_url())
        }
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(UrlTestError::HttpStatus {
            status: status.as_u16(),
        });
    }

    Ok(u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX))
}

/// Throughput measured by downloading through the running local HTTP proxy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpeedTestOutcome {
    pub bytes_per_second: u64,
    pub bytes_read: u64,
    pub elapsed_ms: u32,
}

/// Downloads from `url` through `proxy_address` and returns observed throughput.
///
/// Stops after `max_bytes` or when the body ends, whichever comes first.
///
/// # Errors
///
/// Returns a typed validation, client, request, timeout, or HTTP status error.
pub async fn probe_download_speed(
    url: &str,
    proxy_address: SocketAddr,
    timeout: Duration,
    max_bytes: u64,
) -> Result<SpeedTestOutcome, UrlTestError> {
    if timeout.is_zero() {
        return Err(UrlTestError::InvalidTimeout);
    }
    if max_bytes == 0 {
        return Err(UrlTestError::InvalidTimeout);
    }
    let url = Url::parse(url.trim()).map_err(UrlTestError::InvalidUrl)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(UrlTestError::UnsupportedScheme {
            scheme: url.scheme().to_owned(),
        });
    }

    ensure_rustls_crypto_provider();
    let proxy = reqwest::Proxy::all(format!("http://{proxy_address}"))
        .map_err(|source| UrlTestError::ClientBuild(source.without_url()))?;
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(timeout)
        .redirect(Policy::limited(3))
        .proxy(proxy)
        .build()
        .map_err(|source| UrlTestError::ClientBuild(source.without_url()))?;

    let started = Instant::now();
    let response = client.get(url).send().await.map_err(|source| {
        if source.is_timeout() {
            UrlTestError::TimedOut
        } else {
            UrlTestError::Request(source.without_url())
        }
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(UrlTestError::HttpStatus {
            status: status.as_u16(),
        });
    }

    let body = response.bytes().await.map_err(|source| {
        if source.is_timeout() {
            UrlTestError::TimedOut
        } else {
            UrlTestError::Request(source.without_url())
        }
    })?;
    let bytes_read = u64::try_from(body.len()).unwrap_or(u64::MAX).min(max_bytes);

    let elapsed = started.elapsed();
    let elapsed_ms = u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX);
    let elapsed_secs = elapsed.as_secs_f64().max(0.001);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    let bytes_per_second = (bytes_read as f64 / elapsed_secs).round() as u64;
    Ok(SpeedTestOutcome {
        bytes_per_second,
        bytes_read,
        elapsed_ms,
    })
}
