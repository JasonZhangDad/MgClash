//! Live traffic samples from sing-box's loopback-only Clash API.

use std::net::SocketAddr;
use std::time::Duration;

use magies_profiles::ensure_rustls_crypto_provider;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const SAMPLE_BODY_LIMIT: usize = 1_024;

/// Upload and download bytes observed during the previous second.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficRate {
    pub upload_bytes_per_second: u64,
    pub download_bytes_per_second: u64,
}

#[derive(Debug, Deserialize)]
struct ApiTrafficRate {
    up: u64,
    down: u64,
}

/// Why the local Core API could not produce a live traffic sample.
#[derive(Debug, Error)]
pub enum TrafficSampleError {
    #[error("the traffic sample timeout must be greater than zero")]
    InvalidTimeout,
    #[error("the traffic sample HTTP client could not be built")]
    ClientBuild(#[source] reqwest::Error),
    #[error("the traffic sample timed out")]
    TimedOut,
    #[error("the traffic sample request failed")]
    Request(#[source] reqwest::Error),
    #[error("the traffic API returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("the traffic API closed before emitting a sample")]
    StreamEnded,
    #[error("the traffic API sample exceeded the size limit")]
    BodyTooLarge,
    #[error("the traffic API emitted an invalid sample")]
    InvalidBody(#[source] serde_json::Error),
}

/// Reads the next one-second sample from sing-box's `/traffic` stream.
///
/// # Errors
///
/// Returns a typed timeout, request, status, size, or JSON error.
pub async fn sample_traffic(
    api_address: SocketAddr,
    timeout: Duration,
) -> Result<TrafficRate, TrafficSampleError> {
    if timeout.is_zero() {
        return Err(TrafficSampleError::InvalidTimeout);
    }

    ensure_rustls_crypto_provider();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(timeout)
        .build()
        .map_err(|source| TrafficSampleError::ClientBuild(source.without_url()))?;
    let mut response = client
        .get(format!("http://{api_address}/traffic"))
        .send()
        .await
        .map_err(map_request_error)?;
    let status = response.status();
    if !status.is_success() {
        return Err(TrafficSampleError::HttpStatus {
            status: status.as_u16(),
        });
    }

    let mut body = Vec::new();
    loop {
        let chunk = response
            .chunk()
            .await
            .map_err(map_request_error)?
            .ok_or(TrafficSampleError::StreamEnded)?;
        body.extend_from_slice(&chunk);
        if body.len() > SAMPLE_BODY_LIMIT {
            return Err(TrafficSampleError::BodyTooLarge);
        }
        let Some(line_end) = body.iter().position(|byte| *byte == b'\n') else {
            continue;
        };
        let sample: ApiTrafficRate =
            serde_json::from_slice(&body[..line_end]).map_err(TrafficSampleError::InvalidBody)?;
        return Ok(TrafficRate {
            upload_bytes_per_second: sample.up,
            download_bytes_per_second: sample.down,
        });
    }
}

fn map_request_error(source: reqwest::Error) -> TrafficSampleError {
    if source.is_timeout() {
        TrafficSampleError::TimedOut
    } else {
        TrafficSampleError::Request(source.without_url())
    }
}
