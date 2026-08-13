//! Export and restore of desktop preferences (app / route / DNS).
//!
//! Nodes and keyring secrets stay out of this bundle — that is a separate
//! backup path. The file is enough to recreate how the shell behaves after a
//! reinstall without carrying credentials.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use magies_profiles::DnsConfigError;

use crate::app_settings::AppSettings;
use crate::dns_settings::DnsSettings;
use crate::route_settings::RouteSettings;

const PREFERENCES_SCHEMA: u32 = 1;

/// One portable snapshot of App / Route / DNS settings.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesBundle {
    pub schema: u32,
    pub app: AppSettings,
    pub route: RouteSettings,
    pub dns: DnsSettings,
}

impl PreferencesBundle {
    #[must_use]
    pub fn new(app: AppSettings, route: RouteSettings, dns: DnsSettings) -> Self {
        Self {
            schema: PREFERENCES_SCHEMA,
            app,
            route,
            dns,
        }
    }

    /// Writes the bundle into `directory` and returns the file it created.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the bundle cannot be serialized or written.
    pub fn write_to(&self, directory: &Path) -> Result<PathBuf, PreferencesBackupError> {
        let contents = serde_json::to_vec_pretty(self)
            .map_err(|source| PreferencesBackupError::Serialize { source })?;
        let path = directory.join(format!("mgclash-preferences-{}.json", export_stamp()));
        fs::write(&path, contents).map_err(|source| PreferencesBackupError::Write {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    }

    /// Reads and validates a previously exported preferences file.
    ///
    /// # Errors
    ///
    /// Returns a typed read, parse, or schema error.
    pub fn read_from(path: &Path) -> Result<Self, PreferencesBackupError> {
        let contents = fs::read_to_string(path).map_err(|source| PreferencesBackupError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let bundle: Self = serde_json::from_str(&contents)
            .map_err(|source| PreferencesBackupError::InvalidJson { source })?;
        if bundle.schema != PREFERENCES_SCHEMA {
            return Err(PreferencesBackupError::UnsupportedSchema {
                schema: bundle.schema,
            });
        }
        // Route / DNS must still form valid Core profiles before they replace
        // the user's current settings.
        bundle
            .route
            .profile(magies_routing::RoutingMode::Rule)
            .map_err(PreferencesBackupError::InvalidRoute)?;
        bundle
            .dns
            .profile()
            .map_err(PreferencesBackupError::InvalidDns)?;
        Ok(bundle)
    }
}

fn export_stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[derive(Debug, Error)]
pub enum PreferencesBackupError {
    #[error("failed to serialize preferences")]
    Serialize { source: serde_json::Error },
    #[error("failed to write preferences to {}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("failed to read preferences from {}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("preferences file is not valid JSON")]
    InvalidJson { source: serde_json::Error },
    #[error("preferences schema {schema} is not supported")]
    UnsupportedSchema { schema: u32 },
    #[error("preferences file contains invalid route settings")]
    InvalidRoute(#[source] crate::route_settings::RouteSettingsError),
    #[error("preferences file contains invalid DNS settings")]
    InvalidDns(#[source] DnsConfigError),
}

impl PreferencesBackupError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Serialize { .. } | Self::Write { .. } => "preferences_export_failed",
            Self::Read { .. } | Self::InvalidJson { .. } | Self::UnsupportedSchema { .. } => {
                "preferences_import_failed"
            }
            Self::InvalidRoute(_) => "invalid_route_settings",
            Self::InvalidDns(_) => "invalid_dns_settings",
        }
    }
}
