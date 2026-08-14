//! The live connection list from sing-box's loopback-only Clash API.
//!
//! The window shows what the Core is carrying right now — host, process, the
//! rule that matched, the outbound chain — and can close a connection. Xray has
//! no such API, so the caller only reaches this module for a sing-box session.

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use magies_profiles::ensure_rustls_crypto_provider;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A connection list larger than this is a Core that is not answering sanely;
/// the window could not render it either.
const BODY_LIMIT: usize = 4 * 1_024 * 1_024;

/// One live connection, in the shape the connection table renders.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSummary {
    pub id: String,
    /// The requested host, falling back to the destination address.
    pub host: String,
    pub destination: String,
    pub network: String,
    /// The program that opened the connection, empty when the Core cannot tell.
    pub process: String,
    /// The matched rule, with its payload when the Core reports one.
    pub rule: String,
    /// The outbound chain, outermost last, as one readable line.
    pub chain: String,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    /// When the connection opened, in the Core's own timestamp format.
    pub start: String,
}

/// Every live connection plus the session totals the API reports with them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSnapshot {
    pub upload_total_bytes: u64,
    pub download_total_bytes: u64,
    pub connections: Vec<ConnectionSummary>,
}

/// Why a connection request did not produce an answer.
#[derive(Debug, Error)]
pub enum ConnectionsError {
    #[error("a connection request needs a non-zero timeout")]
    InvalidTimeout,
    #[error("the connection id is not a single path segment")]
    InvalidId,
    #[error("the connections client could not be built")]
    ClientBuild(#[source] reqwest::Error),
    #[error("the connections API did not answer in time")]
    TimedOut,
    #[error("the connections API could not be reached")]
    Request(#[source] reqwest::Error),
    #[error("the connections API returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("the connections API response exceeded the size limit")]
    BodyTooLarge,
    #[error("the connections API returned an invalid response")]
    InvalidBody(#[source] serde_json::Error),
}

/// Reads every connection the Core currently holds.
///
/// # Errors
///
/// Returns a typed timeout, request, status, size, or JSON error.
pub async fn load_connections(
    api_address: SocketAddr,
    timeout: Duration,
) -> Result<ConnectionSnapshot, ConnectionsError> {
    let response = client(timeout)?
        .get(format!("http://{api_address}/connections"))
        .send()
        .await
        .map_err(map_request_error)?;
    let response = succeeding(response)?;
    let body = response.bytes().await.map_err(map_request_error)?;
    if body.len() > BODY_LIMIT {
        return Err(ConnectionsError::BodyTooLarge);
    }
    let api: ApiConnections =
        serde_json::from_slice(&body).map_err(ConnectionsError::InvalidBody)?;
    Ok(ConnectionSnapshot {
        upload_total_bytes: api.upload_total,
        download_total_bytes: api.download_total,
        connections: api.connections.into_iter().map(summarize).collect(),
    })
}

/// Asks the Core to close one connection.
///
/// # Errors
///
/// Returns a typed id, timeout, request, or status error.
pub async fn close_connection(
    api_address: SocketAddr,
    id: &str,
    timeout: Duration,
) -> Result<(), ConnectionsError> {
    if id.is_empty()
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(ConnectionsError::InvalidId);
    }
    let response = client(timeout)?
        .delete(format!("http://{api_address}/connections/{id}"))
        .send()
        .await
        .map_err(map_request_error)?;
    succeeding(response).map(|_| ())
}

/// Asks the Core to close every connection at once.
///
/// # Errors
///
/// Returns a typed timeout, request, or status error.
pub async fn close_all_connections(
    api_address: SocketAddr,
    timeout: Duration,
) -> Result<(), ConnectionsError> {
    let response = client(timeout)?
        .delete(format!("http://{api_address}/connections"))
        .send()
        .await
        .map_err(map_request_error)?;
    succeeding(response).map(|_| ())
}

fn client(timeout: Duration) -> Result<reqwest::Client, ConnectionsError> {
    if timeout.is_zero() {
        return Err(ConnectionsError::InvalidTimeout);
    }
    ensure_rustls_crypto_provider();
    reqwest::Client::builder()
        .no_proxy()
        .timeout(timeout)
        .build()
        .map_err(|source| ConnectionsError::ClientBuild(source.without_url()))
}

fn succeeding(response: reqwest::Response) -> Result<reqwest::Response, ConnectionsError> {
    let status = response.status();
    if status.is_success() {
        Ok(response)
    } else {
        Err(ConnectionsError::HttpStatus {
            status: status.as_u16(),
        })
    }
}

fn map_request_error(source: reqwest::Error) -> ConnectionsError {
    if source.is_timeout() {
        ConnectionsError::TimedOut
    } else {
        ConnectionsError::Request(source.without_url())
    }
}

fn summarize(connection: ApiConnection) -> ConnectionSummary {
    let destination = if connection.metadata.destination_port.is_empty() {
        connection.metadata.destination_ip.clone()
    } else {
        format!(
            "{}:{}",
            connection.metadata.destination_ip, connection.metadata.destination_port
        )
    };
    let host = if connection.metadata.host.is_empty() {
        connection.metadata.destination_ip.clone()
    } else {
        connection.metadata.host.clone()
    };
    let rule = match connection.rule_payload.as_deref() {
        Some(payload) if !payload.is_empty() => format!("{}({payload})", connection.rule),
        _ => connection.rule,
    };
    // The API lists the chain innermost first; reading it outbound-last matches
    // how the route is written down.
    let chain = connection
        .chains
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join(" → ");
    ConnectionSummary {
        id: connection.id,
        host,
        destination,
        network: connection.metadata.network,
        process: process_name(&connection.metadata.process_path),
        rule,
        chain,
        upload_bytes: connection.upload,
        download_bytes: connection.download,
        start: connection.start,
    }
}

fn process_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiConnections {
    #[serde(default)]
    upload_total: u64,
    #[serde(default)]
    download_total: u64,
    #[serde(default)]
    connections: Vec<ApiConnection>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiConnection {
    id: String,
    #[serde(default)]
    upload: u64,
    #[serde(default)]
    download: u64,
    #[serde(default)]
    start: String,
    #[serde(default)]
    chains: Vec<String>,
    #[serde(default)]
    rule: String,
    #[serde(default)]
    rule_payload: Option<String>,
    #[serde(default)]
    metadata: ApiMetadata,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiMetadata {
    #[serde(default)]
    network: String,
    #[serde(default)]
    host: String,
    #[serde(default, rename = "destinationIP")]
    destination_ip: String,
    #[serde(default)]
    destination_port: String,
    #[serde(default)]
    process_path: String,
}
