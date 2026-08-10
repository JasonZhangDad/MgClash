//! The redacted diagnostic bundle behind Settings → Logs → Export diagnostics.
//!
//! PRD section 25.3: "导出诊断包必须先脱敏". The runtime config is the whole
//! point of a bundle — it is what a support conversation actually needs — and
//! also the one artifact that always contains the node's credential, so it is
//! parsed and passed through [`DiagnosticRedactor`] before it is written.
//!
//! Collecting a bundle never fails on a missing or malformed config: an export
//! that refuses to run is useless precisely when something is broken. Whatever
//! could not be read is reported as a reason string instead. A config that
//! failed to parse is never copied in verbatim, because unparsed bytes cannot
//! be redacted field by field.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use magies_profiles::DiagnosticRedactor;
use serde_json::{Value, json};
use thiserror::Error;

use crate::session::SessionStatus;
use crate::{PlatformSummary, platform_summary_for};

/// A redacted snapshot of what the app is doing right now.
#[derive(Clone, Debug)]
pub struct DiagnosticBundle {
    report: Value,
}

impl DiagnosticBundle {
    /// Builds the bundle from the current status and, when a session is
    /// running, its on-disk runtime config.
    #[must_use]
    pub fn collect(status: &SessionStatus, runtime_config: Option<&Path>) -> Self {
        let (config, config_error) = match runtime_config {
            Some(path) => read_redacted_config(path),
            None => (Value::Null, Value::Null),
        };

        Self {
            report: json!({
                "schema": 1,
                "platform": platform_report(),
                "session": session_report(status),
                "runtimeConfig": config,
                "runtimeConfigError": config_error,
            }),
        }
    }

    #[must_use]
    pub const fn to_json(&self) -> &Value {
        &self.report
    }

    /// Writes the bundle into `directory` and returns the file it created.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the bundle cannot be serialized or the file
    /// cannot be written.
    pub fn write_to(&self, directory: &Path) -> Result<PathBuf, DiagnosticBundleError> {
        let contents = serde_json::to_vec_pretty(&self.report)
            .map_err(|source| DiagnosticBundleError::Serialize { source })?;
        let path = directory.join(format!("mgclash-diagnostics-{}.json", export_stamp()));
        fs::write(&path, contents).map_err(|source| DiagnosticBundleError::Write {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    }
}

/// Returns the redacted config and the reason it is absent, at most one of
/// which is ever non-null.
fn read_redacted_config(path: &Path) -> (Value, Value) {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(source) => {
            return (
                Value::Null,
                json!(format!("failed to read {}: {source}", path.display())),
            );
        }
    };
    match serde_json::from_str::<Value>(&contents) {
        // Deliberately drops the raw text: only parsed JSON can be redacted
        // key by key, so anything unparsed must not reach the bundle.
        Err(source) => (
            Value::Null,
            json!(format!("failed to parse {}: {source}", path.display())),
        ),
        Ok(config) => (DiagnosticRedactor::new().redact_json(&config), Value::Null),
    }
}

fn platform_report() -> Value {
    match platform_summary_for(std::env::consts::OS, std::env::consts::ARCH) {
        Ok(PlatformSummary {
            artifact_identifier,
            tun_availability,
        }) => json!({
            "artifactIdentifier": artifact_identifier,
            "tunAvailability": tun_availability,
            "appVersion": env!("CARGO_PKG_VERSION"),
        }),
        Err(error) => json!({
            "artifactIdentifier": format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            "tunAvailability": "unsupportedTarget",
            "appVersion": env!("CARGO_PKG_VERSION"),
            "error": error.message,
        }),
    }
}

/// The node's name, protocol, and address stay in the bundle: they are what
/// makes a report actionable, and none of them is a credential.
fn session_report(status: &SessionStatus) -> Value {
    json!({
        "connected": status.connected,
        "core": status.core,
        "mode": status.mode,
        "systemProxy": status.system_proxy,
        "socksPort": status.socks_port,
        "httpPort": status.http_port,
        "node": status.node.as_ref().map(|node| json!({
            "name": node.name,
            "protocol": node.protocol,
            "server": node.server,
            "port": node.port,
        })),
    })
}

/// Seconds since the epoch, so two exports in the same session never collide.
fn export_stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[derive(Debug, Error)]
pub enum DiagnosticBundleError {
    #[error("failed to serialize the diagnostic bundle")]
    Serialize {
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to write the diagnostic bundle to {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl DiagnosticBundleError {
    /// The stable machine-readable code the UI branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Serialize { .. } => "diagnostics_serialize_failed",
            Self::Write { .. } => "diagnostics_write_failed",
        }
    }
}
