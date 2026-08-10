//! Lazily validated sing-box control for the desktop shell.
//!
//! Release artifacts are unsigned and ship the pinned Core next to the app, so
//! its location and SHA-256 come from configuration. Resolving that pin is
//! deferred to the first connect: an unconfigured or tampered Core then fails
//! with a typed error the dashboard can show, instead of preventing startup.

use std::error::Error;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use magies_core_runtime::{
    CoreBinaryError, CoreBinaryRequirement, CoreOutput, Sha256Hash, Sha256HashParseError,
    SingBoxAdapter, locate_core_binary,
};
use magies_platform::{PlatformError, TargetPlatform};
use magies_session::{CoreSessionControl, SingBoxCoreControl, SingBoxCoreSessionError};
use thiserror::Error;

pub const BINARY_PATH_VARIABLE: &str = "MAGIES_SING_BOX_BIN";
pub const SHA256_VARIABLE: &str = "MAGIES_SING_BOX_SHA256";

/// The pinned sing-box binary this build is allowed to run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreSettings {
    pub binary: PathBuf,
    pub sha256: Sha256Hash,
}

impl CoreSettings {
    /// Reads the pinned Core location and digest from the environment.
    ///
    /// # Errors
    ///
    /// Returns a typed error when either variable is missing or the digest is
    /// not 32 hex-encoded bytes.
    pub fn from_env() -> Result<Self, CoreSettingsError> {
        Self::from_values(
            std::env::var_os(BINARY_PATH_VARIABLE).map(PathBuf::from),
            std::env::var(SHA256_VARIABLE).ok(),
        )
    }

    /// Validates an already-read Core location and digest.
    ///
    /// # Errors
    ///
    /// Returns a typed error when either value is missing or the digest is not
    /// 32 hex-encoded bytes.
    pub fn from_values(
        binary: Option<PathBuf>,
        sha256: Option<String>,
    ) -> Result<Self, CoreSettingsError> {
        let binary = binary.ok_or(CoreSettingsError::MissingBinaryPath)?;
        let sha256 = sha256
            .ok_or(CoreSettingsError::MissingSha256)?
            .parse()
            .map_err(CoreSettingsError::InvalidSha256)?;
        Ok(Self { binary, sha256 })
    }
}

/// A [`CoreSessionControl`] that validates its binary on the first start.
pub struct LazySingBoxControl {
    settings: Result<CoreSettings, CoreSettingsError>,
    health_address: SocketAddr,
    health_timeout: Duration,
    control: Option<SingBoxCoreControl>,
}

impl LazySingBoxControl {
    #[must_use]
    pub const fn new(
        settings: Result<CoreSettings, CoreSettingsError>,
        health_address: SocketAddr,
        health_timeout: Duration,
    ) -> Self {
        Self {
            settings,
            health_address,
            health_timeout,
            control: None,
        }
    }

    /// Reads the pinned Core from the environment, deferring failures to the
    /// first connect.
    #[must_use]
    pub fn from_env(health_address: SocketAddr, health_timeout: Duration) -> Self {
        Self::new(CoreSettings::from_env(), health_address, health_timeout)
    }

    fn resolve_control(&mut self) -> Result<(), LazySingBoxError> {
        if self.control.is_some() {
            return Ok(());
        }

        let settings = self
            .settings
            .as_ref()
            .map_err(|source| LazySingBoxError::Settings(source.clone()))?;
        let target = TargetPlatform::parse(std::env::consts::OS, std::env::consts::ARCH)
            .map_err(LazySingBoxError::Target)?;
        let binary = locate_core_binary(
            &settings.binary,
            CoreBinaryRequirement::new(target.architecture(), settings.sha256),
        )
        .map_err(|source| LazySingBoxError::Binary(Box::new(source)))?;
        self.control = Some(SingBoxCoreControl::new(
            SingBoxAdapter::new(binary),
            self.health_address,
            self.health_timeout,
        ));
        Ok(())
    }
}

impl CoreSessionControl for LazySingBoxControl {
    type Error = LazySingBoxError;
    type Output = CoreOutput;

    fn start(&mut self, config_path: &Path) -> Result<Self::Output, Self::Error> {
        self.resolve_control()?;
        self.control
            .as_mut()
            .expect("resolve_control creates the control whenever it is missing")
            .start(config_path)
            .map_err(|source| LazySingBoxError::Session(Box::new(source)))
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        match self.control.as_mut() {
            Some(control) => control
                .stop()
                .map_err(|source| LazySingBoxError::Session(Box::new(source))),
            None => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CoreSettingsError {
    #[error("{BINARY_PATH_VARIABLE} must point to the pinned sing-box binary")]
    MissingBinaryPath,
    #[error("{SHA256_VARIABLE} must contain the pinned sing-box SHA-256")]
    MissingSha256,
    #[error("{SHA256_VARIABLE} is not a SHA-256 digest")]
    InvalidSha256(#[source] Sha256HashParseError),
}

#[derive(Debug, Error)]
pub enum LazySingBoxError {
    #[error("the pinned sing-box binary is not configured")]
    Settings(#[source] CoreSettingsError),
    #[error("this build runs on a target outside the V0.1 support matrix")]
    Target(#[source] PlatformError),
    #[error("the configured sing-box binary does not match its pin")]
    Binary(#[source] Box<CoreBinaryError>),
    #[error("the sing-box session failed")]
    Session(#[source] Box<SingBoxCoreSessionError>),
}

impl LazySingBoxError {
    /// The stable machine-readable code the UI branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Settings(_) => "core_not_configured",
            Self::Target(_) => "unsupported_target",
            Self::Binary(_) => "core_binary_rejected",
            Self::Session(_) => "core_session_failed",
        }
    }
}

/// Renders an error and every cause behind it for the developer-facing log.
pub fn describe(error: &dyn Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}
