//! Comparing this build's version against a published release.
//!
//! Only the comparison lives here. Fetching belongs to the shell, which owns the
//! decision to contact anything at all — this crate answers *whether* a version
//! is newer, not *when* to ask.

use std::error::Error;
use std::fmt::{Display, Formatter};

/// A released version, as `Cargo.toml` and a release tag spell it.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReleaseVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

impl ReleaseVersion {
    /// Parses `MAJOR.MINOR.PATCH`, with or without the `v` a tag carries.
    ///
    /// # Errors
    ///
    /// Returns [`ReleaseVersionError::Malformed`] for anything else, including
    /// the pre-release suffixes semver allows: `0.2.0-rc.1` is *older* than
    /// `0.2.0`, and guessing at that would offer a release candidate as a stable
    /// update.
    pub fn parse(value: &str) -> Result<Self, ReleaseVersionError> {
        let trimmed = value.trim();
        let digits = trimmed.strip_prefix('v').unwrap_or(trimmed);
        let mut parts = digits.split('.');
        let mut next = || {
            parts
                .next()
                .and_then(|part| part.parse::<u32>().ok())
                .ok_or_else(|| ReleaseVersionError::Malformed {
                    value: value.to_owned(),
                })
        };
        let version = Self {
            major: next()?,
            minor: next()?,
            patch: next()?,
        };
        if parts.next().is_some() {
            return Err(ReleaseVersionError::Malformed {
                value: value.to_owned(),
            });
        }
        Ok(version)
    }

    /// Whether `published` is worth telling the user about.
    ///
    /// A published version older than this build reports up to date: it happens
    /// when running a development build, and offering a downgrade is not what
    /// "check for updates" means.
    #[must_use]
    pub fn compare(&self, published: &Self) -> UpdateStatus {
        if published > self {
            UpdateStatus::UpdateAvailable
        } else {
            UpdateStatus::UpToDate
        }
    }
}

impl Display for ReleaseVersion {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// What a check concluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateStatus {
    UpToDate,
    UpdateAvailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseVersionError {
    Malformed { value: String },
}

impl Display for ReleaseVersionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed { value } => {
                write!(formatter, "{value:?} is not a MAJOR.MINOR.PATCH version")
            }
        }
    }
}

impl Error for ReleaseVersionError {}
