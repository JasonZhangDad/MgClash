//! The on-disk copy of every remote rule set the user configured.
//!
//! sing-box can fetch a remote rule set itself, but then the app cannot say
//! when it last succeeded and the user cannot ask for it now. Downloading the
//! file here makes both possible: the generated config points at a local path,
//! and "update" is a button rather than a wait.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use magies_profiles::ensure_rustls_crypto_provider;
use serde::Serialize;
use thiserror::Error;

/// A rule set larger than this is not one the Core would load happily either.
const MAX_BYTES: usize = 32 * 1_024 * 1_024;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

/// What the rule-set table shows for one provider.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleProviderCacheEntry {
    pub name: String,
    pub cached: bool,
    pub bytes: u64,
    /// Seconds since the epoch, or `None` when the set was never downloaded.
    pub updated_at: Option<i64>,
    pub path: String,
}

pub struct RuleProviderCache {
    directory: PathBuf,
}

impl RuleProviderCache {
    /// Opens the cache directory, creating it when missing.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the directory cannot be created.
    pub fn open(directory: impl Into<PathBuf>) -> Result<Self, RuleProviderCacheError> {
        let directory = directory.into();
        fs::create_dir_all(&directory).map_err(|source| {
            RuleProviderCacheError::CreateDirectory {
                path: directory.clone(),
                source,
            }
        })?;
        Ok(Self { directory })
    }

    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// The file a provider's rule set is cached in, whether or not it exists.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the name is not a safe file stem.
    pub fn path_for(&self, name: &str) -> Result<PathBuf, RuleProviderCacheError> {
        Ok(self.directory.join(cache_file_name(name)?))
    }

    /// Reports what is cached for one provider.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the name is not a safe file stem.
    pub fn entry(&self, name: &str) -> Result<RuleProviderCacheEntry, RuleProviderCacheError> {
        let path = self.path_for(name)?;
        let metadata = fs::metadata(&path).ok();
        Ok(RuleProviderCacheEntry {
            name: name.to_owned(),
            cached: metadata.is_some(),
            bytes: metadata.as_ref().map_or(0, std::fs::Metadata::len),
            updated_at: metadata
                .and_then(|metadata| metadata.modified().ok())
                .and_then(system_time_secs),
            path: path.display().to_string(),
        })
    }

    /// Downloads one rule set into the cache, replacing any earlier copy.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a bad name, HTTP failure, empty or oversized
    /// body, or a filesystem failure. The previous copy survives a failure.
    pub async fn update(
        &self,
        name: &str,
        url: &str,
    ) -> Result<RuleProviderCacheEntry, RuleProviderCacheError> {
        let destination = self.path_for(name)?;
        ensure_rustls_crypto_provider();
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(DOWNLOAD_TIMEOUT)
            .user_agent(concat!("MgClash/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|source| RuleProviderCacheError::ClientBuild {
                source: source.without_url(),
            })?;
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|source| RuleProviderCacheError::RequestFailed {
                url: url.to_owned(),
                source: source.without_url(),
            })?
            .error_for_status()
            .map_err(|source| RuleProviderCacheError::RequestFailed {
                url: url.to_owned(),
                source: source.without_url(),
            })?;
        let bytes =
            response
                .bytes()
                .await
                .map_err(|source| RuleProviderCacheError::RequestFailed {
                    url: url.to_owned(),
                    source: source.without_url(),
                })?;
        if bytes.is_empty() {
            return Err(RuleProviderCacheError::EmptyBody {
                url: url.to_owned(),
            });
        }
        if bytes.len() > MAX_BYTES {
            return Err(RuleProviderCacheError::TooLarge {
                url: url.to_owned(),
                bytes: bytes.len(),
            });
        }
        // Written beside the target and renamed, so a failed download cannot
        // leave the Core pointing at half a rule set.
        let temporary = destination.with_extension("partial");
        fs::write(&temporary, &bytes).map_err(|source| RuleProviderCacheError::Write {
            path: temporary.clone(),
            source,
        })?;
        fs::rename(&temporary, &destination).map_err(|source| RuleProviderCacheError::Write {
            path: destination.clone(),
            source,
        })?;
        self.entry(name)
    }

    /// Removes a provider's cached file, if any.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the name is unsafe or the file cannot be
    /// removed.
    pub fn forget(&self, name: &str) -> Result<(), RuleProviderCacheError> {
        let path = self.path_for(name)?;
        if !path.exists() {
            return Ok(());
        }
        fs::remove_file(&path).map_err(|source| RuleProviderCacheError::Write { path, source })
    }
}

/// The cache file for a provider name.
///
/// The name reaches the filesystem, so it has to be one path segment: the
/// routing layer already restricts tags to this alphabet, and this is the
/// second place that has to hold.
fn cache_file_name(name: &str) -> Result<String, RuleProviderCacheError> {
    let trimmed = name.trim().to_ascii_lowercase();
    let valid = !trimmed.is_empty()
        && trimmed.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        });
    if !valid {
        return Err(RuleProviderCacheError::InvalidName {
            name: name.to_owned(),
        });
    }
    Ok(format!("{trimmed}.srs"))
}

fn system_time_secs(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
}

#[derive(Debug, Error)]
pub enum RuleProviderCacheError {
    #[error("failed to create the rule set cache {}: {source}", path.display())]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("rule set name {name:?} is not a safe file name")]
    InvalidName { name: String },
    #[error("failed to build the rule set download client")]
    ClientBuild { source: reqwest::Error },
    #[error("failed to download {url}")]
    RequestFailed { url: String, source: reqwest::Error },
    #[error("{url} returned an empty rule set")]
    EmptyBody { url: String },
    #[error("{url} returned {bytes} bytes, more than the rule set limit")]
    TooLarge { url: String, bytes: usize },
    #[error("failed to write {}: {source}", path.display())]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl RuleProviderCacheError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidName { .. } => "invalid_rule_set_name",
            Self::CreateDirectory { .. } | Self::Write { .. } => "rule_set_write_failed",
            Self::ClientBuild { .. }
            | Self::RequestFailed { .. }
            | Self::EmptyBody { .. }
            | Self::TooLarge { .. } => "rule_set_download_failed",
        }
    }
}
