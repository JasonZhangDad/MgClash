//! Download and status for Xray `geoip.dat` / `geosite.dat`.
//!
//! Xray reads these from `XRAY_LOCATION_ASSET`. The desktop keeps a copy under
//! the app data directory and points that variable at it so Rule-mode Geo
//! matchers work without shipping the databases next to a user-supplied binary.
//!
//! sing-box continues to pull remote rule sets at connect time; this module is
//! the v2rayN-style explicit update path for the Xray databases.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use magies_profiles::ensure_rustls_crypto_provider;
use serde::Serialize;
use thiserror::Error;

const GEOIP_URL: &str =
    "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geoip.dat";
const GEOSITE_URL: &str =
    "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geosite.dat";
const GEOIP_FILE: &str = "geoip.dat";
const GEOSITE_FILE: &str = "geosite.dat";
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_BYTES: usize = 64 * 1024 * 1024;

/// One on-disk Geo database the UI can show.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoFileStatus {
    pub name: String,
    pub present: bool,
    pub bytes: u64,
    pub modified_at: Option<i64>,
}

/// What the Geo update dialog renders.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoAssetsStatus {
    pub directory: String,
    pub geoip: GeoFileStatus,
    pub geosite: GeoFileStatus,
    pub asset_env_applied: bool,
}

pub struct GeoAssetsStore {
    directory: PathBuf,
}

impl GeoAssetsStore {
    /// Opens the Geo assets directory, creating it when missing.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the directory cannot be created.
    pub fn open(directory: impl Into<PathBuf>) -> Result<Self, GeoAssetsError> {
        let directory = directory.into();
        fs::create_dir_all(&directory).map_err(|source| GeoAssetsError::CreateDirectory {
            path: directory.clone(),
            source,
        })?;
        Ok(Self { directory })
    }

    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Returns whether both databases are present.
    #[must_use]
    pub fn status(&self) -> GeoAssetsStatus {
        let geoip = file_status(&self.directory.join(GEOIP_FILE), GEOIP_FILE);
        let geosite = file_status(&self.directory.join(GEOSITE_FILE), GEOSITE_FILE);
        GeoAssetsStatus {
            directory: self.directory.display().to_string(),
            geoip: geoip.clone(),
            geosite: geosite.clone(),
            asset_env_applied: geoip.present && geosite.present,
        }
    }

    /// Downloads the latest `geoip.dat` and `geosite.dat`.
    ///
    /// # Errors
    ///
    /// Returns a typed error for HTTP, size, or filesystem failures.
    pub async fn update(&self) -> Result<GeoAssetsStatus, GeoAssetsError> {
        download_file(GEOIP_URL, &self.directory.join(GEOIP_FILE)).await?;
        download_file(GEOSITE_URL, &self.directory.join(GEOSITE_FILE)).await?;
        Ok(self.status())
    }
}

fn file_status(path: &Path, name: &str) -> GeoFileStatus {
    match fs::metadata(path) {
        Ok(meta) => GeoFileStatus {
            name: name.to_owned(),
            present: true,
            bytes: meta.len(),
            modified_at: meta.modified().ok().and_then(system_time_secs),
        },
        Err(_) => GeoFileStatus {
            name: name.to_owned(),
            present: false,
            bytes: 0,
            modified_at: None,
        },
    }
}

fn system_time_secs(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
}

async fn download_file(url: &str, destination: &Path) -> Result<(), GeoAssetsError> {
    ensure_rustls_crypto_provider();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(DOWNLOAD_TIMEOUT)
        .user_agent(concat!("MgClash/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|source| GeoAssetsError::ClientBuild {
            source: source.without_url(),
        })?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|source| GeoAssetsError::RequestFailed {
            url: url.to_owned(),
            source: source.without_url(),
        })?
        .error_for_status()
        .map_err(|source| GeoAssetsError::RequestFailed {
            url: url.to_owned(),
            source: source.without_url(),
        })?;
    let bytes = response
        .bytes()
        .await
        .map_err(|source| GeoAssetsError::RequestFailed {
            url: url.to_owned(),
            source: source.without_url(),
        })?;
    if bytes.len() > MAX_BYTES {
        return Err(GeoAssetsError::TooLarge {
            url: url.to_owned(),
            bytes: bytes.len(),
        });
    }
    if bytes.is_empty() {
        return Err(GeoAssetsError::EmptyBody {
            url: url.to_owned(),
        });
    }
    let temporary = destination.with_extension("dat.partial");
    fs::write(&temporary, &bytes).map_err(|source| GeoAssetsError::Write {
        path: temporary.clone(),
        source,
    })?;
    fs::rename(&temporary, destination).map_err(|source| GeoAssetsError::Write {
        path: destination.to_path_buf(),
        source,
    })?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum GeoAssetsError {
    #[error("failed to create Geo assets directory {}: {source}", path.display())]
    CreateDirectory { path: PathBuf, source: io::Error },
    #[error("failed to build the Geo download client")]
    ClientBuild { source: reqwest::Error },
    #[error("failed to download {url}")]
    RequestFailed { url: String, source: reqwest::Error },
    #[error("Geo download from {url} exceeded {bytes} bytes")]
    TooLarge { url: String, bytes: usize },
    #[error("Geo download from {url} returned an empty body")]
    EmptyBody { url: String },
    #[error("failed to write Geo file {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
}

impl GeoAssetsError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::CreateDirectory { .. } | Self::Write { .. } => "geo_assets_store_failed",
            Self::ClientBuild { .. }
            | Self::RequestFailed { .. }
            | Self::TooLarge { .. }
            | Self::EmptyBody { .. } => "geo_assets_update_failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_missing_files_before_download() {
        let directory = std::env::temp_dir().join(format!(
            "mgclash-geo-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = GeoAssetsStore::open(&directory).unwrap();
        let status = store.status();
        assert!(!status.geoip.present);
        assert!(!status.geosite.present);
        let _ = fs::remove_dir_all(directory);
    }
}
