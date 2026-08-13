//! Checks whether newer sing-box / Xray releases exist on GitHub.
//!
//! Like the app release check, this only runs when the user asks from the menu.
//! Actual downloads are handled by [`crate::core_install`].

use magies_platform::release::{ReleaseVersion, UpdateStatus};
use magies_profiles::ensure_rustls_crypto_provider;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

use crate::core_install::CoreInstallStore;

use crate::core_control::{
    read_sing_box_version_from_settings_with_store, read_xray_version_from_settings_with_store,
};

/// sing-box version this build was tested against.
pub const PINNED_SING_BOX_VERSION: &str = "1.13.18";
/// Xray version this build was tested against.
pub const PINNED_XRAY_VERSION: &str = "26.3.27";

const SING_BOX_RELEASE_API: &str = "https://api.github.com/repos/SagerNet/sing-box/releases/latest";
const XRAY_RELEASE_API: &str = "https://api.github.com/repos/XTLS/Xray-core/releases/latest";
const SING_BOX_RELEASE_HTML: &str = "https://github.com/SagerNet/sing-box/releases";
const XRAY_RELEASE_HTML: &str = "https://github.com/XTLS/Xray-core/releases";
const CHECK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// What one Core's release check found.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreVersionCheck {
    pub name: String,
    pub current: String,
    pub latest: String,
    pub url: String,
    pub update_available: bool,
    /// `true` when `current` came from the validated binary's `--version`.
    pub from_binary: bool,
}

/// sing-box and Xray release checks bundled for the menu dialog.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreUpdateCheck {
    pub sing_box: CoreVersionCheck,
    pub xray: CoreVersionCheck,
    pub install: crate::core_install::CoreInstallStatus,
}

/// Asks GitHub whether either Core has a newer release than what is installed.
///
/// # Errors
///
/// Returns a typed error when the HTTP client or either release response fails.
pub async fn check_core_updates(
    install: Option<&CoreInstallStore>,
) -> Result<CoreUpdateCheck, CoreUpdateError> {
    ensure_rustls_crypto_provider();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(CHECK_TIMEOUT)
        .user_agent(concat!("MgClash/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|source| CoreUpdateError::ClientBuild {
            source: source.without_url(),
        })?;

    let sing_box_body = fetch_release_body(&client, SING_BOX_RELEASE_API).await?;
    let xray_body = fetch_release_body(&client, XRAY_RELEASE_API).await?;
    let sing_box_release = parse_release_body(&sing_box_body, SING_BOX_RELEASE_HTML)?;
    let xray_release = parse_release_body(&xray_body, XRAY_RELEASE_HTML)?;

    let install_status = match install {
        Some(store) => store
            .status()
            .map_err(|source| CoreUpdateError::InstallStatus { source })?,
        None => crate::core_install::CoreInstallStatus {
            directory: String::new(),
            sing_box: None,
            xray: None,
        },
    };

    Ok(CoreUpdateCheck {
        sing_box: build_entry_impl(
            "sing-box",
            PINNED_SING_BOX_VERSION,
            read_sing_box_version_from_settings_with_store(install),
            &sing_box_release,
        )?,
        xray: build_entry_impl(
            "Xray",
            PINNED_XRAY_VERSION,
            read_xray_version_from_settings_with_store(install),
            &xray_release,
        )?,
        install: install_status,
    })
}

fn build_entry_impl<E>(
    name: &str,
    pinned: &str,
    installed: Result<String, E>,
    release: &GithubRelease,
) -> Result<CoreVersionCheck, CoreUpdateError> {
    let (current, from_binary) = match installed {
        Ok(version) => (version, true),
        Err(_) => (pinned.to_owned(), false),
    };
    let latest_tag = if release.tag_name.is_empty() {
        current.clone()
    } else {
        release.tag_name.clone()
    };
    let latest =
        ReleaseVersion::parse(&latest_tag).map_err(|source| CoreUpdateError::MalformedVersion {
            core: name.to_owned(),
            value: latest_tag.clone(),
            source,
        })?;
    let current_version =
        ReleaseVersion::parse(&current).map_err(|source| CoreUpdateError::MalformedVersion {
            core: name.to_owned(),
            value: current.clone(),
            source,
        })?;
    Ok(CoreVersionCheck {
        name: name.to_owned(),
        current,
        latest: latest.to_string(),
        url: if release.html_url.is_empty() {
            match name {
                "sing-box" => SING_BOX_RELEASE_HTML.to_owned(),
                _ => XRAY_RELEASE_HTML.to_owned(),
            }
        } else {
            release.html_url.clone()
        },
        update_available: current_version.compare(&latest) == UpdateStatus::UpdateAvailable,
        from_binary,
    })
}

async fn fetch_release_body(
    client: &reqwest::Client,
    url: &str,
) -> Result<String, CoreUpdateError> {
    let response =
        client
            .get(url)
            .send()
            .await
            .map_err(|source| CoreUpdateError::RequestFailed {
                url: url.to_owned(),
                source: source.without_url(),
            })?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(String::new());
    }
    let response =
        response
            .error_for_status()
            .map_err(|source| CoreUpdateError::RequestFailed {
                url: url.to_owned(),
                source: source.without_url(),
            })?;
    response
        .text()
        .await
        .map_err(|source| CoreUpdateError::RequestFailed {
            url: url.to_owned(),
            source: source.without_url(),
        })
}

fn parse_release_body(body: &str, fallback_html: &str) -> Result<GithubRelease, CoreUpdateError> {
    if body.is_empty() {
        return Ok(GithubRelease {
            tag_name: String::new(),
            html_url: fallback_html.to_owned(),
        });
    }
    serde_json::from_str(body).map_err(|source| CoreUpdateError::ParseRelease { source })
}

#[derive(Clone, Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
}

#[derive(Debug, Error)]
pub enum CoreUpdateError {
    #[error("failed to build the Core release-check client")]
    ClientBuild { source: reqwest::Error },
    #[error("failed to download {url}")]
    RequestFailed { url: String, source: reqwest::Error },
    #[error("failed to read the Core install manifest")]
    InstallStatus {
        source: crate::core_install::CoreInstallError,
    },
    #[error("failed to parse a GitHub release response")]
    ParseRelease { source: serde_json::Error },
    #[error("{core} reported version {value:?} is not MAJOR.MINOR.PATCH")]
    MalformedVersion {
        core: String,
        value: String,
        source: magies_platform::release::ReleaseVersionError,
    },
}

impl CoreUpdateError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ClientBuild { .. }
            | Self::RequestFailed { .. }
            | Self::InstallStatus { .. }
            | Self::ParseRelease { .. }
            | Self::MalformedVersion { .. } => "core_update_check_failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_control::SingBoxVersionReadError;

    #[test]
    fn marks_a_newer_github_tag_as_an_update() {
        let release = GithubRelease {
            tag_name: "v1.14.0".to_owned(),
            html_url: "https://example.invalid/sing-box".to_owned(),
        };
        let entry = build_entry_impl::<SingBoxVersionReadError>(
            "sing-box",
            PINNED_SING_BOX_VERSION,
            Ok(PINNED_SING_BOX_VERSION.to_owned()),
            &release,
        )
        .unwrap();
        assert!(entry.update_available);
        assert_eq!(entry.latest, "1.14.0");
    }

    #[test]
    fn reports_up_to_date_when_github_matches_the_installed_version() {
        let release = GithubRelease {
            tag_name: "1.13.18".to_owned(),
            html_url: String::new(),
        };
        let entry = build_entry_impl::<SingBoxVersionReadError>(
            "sing-box",
            PINNED_SING_BOX_VERSION,
            Ok(PINNED_SING_BOX_VERSION.to_owned()),
            &release,
        )
        .unwrap();
        assert!(!entry.update_available);
    }

    #[test]
    fn falls_back_to_the_pinned_version_when_the_binary_is_not_configured() {
        let release = GithubRelease {
            tag_name: "v26.3.27".to_owned(),
            html_url: String::new(),
        };
        let entry = build_entry_impl::<SingBoxVersionReadError>(
            "Xray",
            PINNED_XRAY_VERSION,
            Err(SingBoxVersionReadError::Settings(
                crate::core_control::CoreSettingsError::MissingBinaryPath,
            )),
            &release,
        )
        .unwrap();
        assert!(!entry.update_available);
        assert!(!entry.from_binary);
        assert_eq!(entry.current, PINNED_XRAY_VERSION);
    }
}
