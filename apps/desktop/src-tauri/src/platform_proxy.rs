//! System Proxy control for the host platform.
//!
//! Every OS gets the same [`SystemProxyRecoveryManager`] behaviour; only the
//! adapter underneath differs. Building that adapter can fail (a Linux desktop
//! without the GNOME proxy schema, for example), so the failure is carried into
//! the session and reported when the user connects rather than at startup.
//! `stop` stays infallible in that state so a shutdown is never blocked by an
//! adapter that never applied anything.

use std::path::PathBuf;

use magies_platform::system_proxy::SystemProxyState;
use magies_platform::system_proxy_recovery::{
    JsonRecoveryStore, JsonRecoveryStoreError, SystemProxyControl, SystemProxyRecoveryError,
    SystemProxyRecoveryManager,
};
use magies_session::SystemProxySessionControl;
use thiserror::Error;

use crate::core_control::describe;

#[cfg(target_os = "linux")]
pub use linux::adapter;
#[cfg(target_os = "macos")]
pub use macos::{NETWORK_SERVICE_VARIABLE, adapter};
#[cfg(target_os = "windows")]
pub use windows::adapter;

#[cfg(target_os = "macos")]
type PlatformAdapter = magies_platform::macos_system_proxy::MacOsSystemProxyAdapter;
#[cfg(target_os = "windows")]
type PlatformAdapter = magies_platform::windows_system_proxy::WindowsSystemProxyAdapter;
#[cfg(target_os = "linux")]
type PlatformAdapter = magies_platform::linux_system_proxy::LinuxSystemProxyAdapter;

type PlatformSnapshot = <PlatformAdapter as SystemProxyControl>::Snapshot;
type PlatformAdapterError = <PlatformAdapter as SystemProxyControl>::Error;

/// The recovery manager this build drives, with the host's adapter.
pub type PlatformRecoveryManager =
    SystemProxyRecoveryManager<PlatformAdapter, JsonRecoveryStore<PlatformSnapshot>>;

/// A [`SystemProxySessionControl`] that reports an unavailable adapter instead
/// of preventing the app from starting.
pub struct PlatformProxyControl {
    manager: Result<PlatformRecoveryManager, PlatformProxySetupError>,
}

impl PlatformProxyControl {
    #[must_use]
    pub const fn new(manager: Result<PlatformRecoveryManager, PlatformProxySetupError>) -> Self {
        Self { manager }
    }

    /// Builds the host's adapter and snapshots recovery state at
    /// `recovery_store_path`.
    #[must_use]
    pub fn for_host(recovery_store_path: PathBuf) -> Self {
        Self::new(adapter().map(|adapter| {
            SystemProxyRecoveryManager::new(adapter, JsonRecoveryStore::new(recovery_store_path))
        }))
    }

    fn manager(&self) -> Result<&PlatformRecoveryManager, PlatformProxyError> {
        self.manager
            .as_ref()
            .map_err(|source| PlatformProxyError::Setup(source.clone()))
    }
}

impl SystemProxySessionControl for PlatformProxyControl {
    type Error = PlatformProxyError;

    fn enable(&mut self, state: &SystemProxyState) -> Result<(), Self::Error> {
        self.manager()?
            .enable(state)
            .map_err(PlatformProxyError::Recovery)
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        match self.manager.as_ref() {
            Ok(manager) => manager.stop().map_err(PlatformProxyError::Recovery),
            Err(_) => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("the System Proxy adapter for this platform is unavailable: {message}")]
pub struct PlatformProxySetupError {
    message: String,
}

impl PlatformProxySetupError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Error)]
pub enum PlatformProxyError {
    #[error("System Proxy is unavailable on this desktop")]
    Setup(#[source] PlatformProxySetupError),
    #[error("the System Proxy change failed")]
    Recovery(#[source] SystemProxyRecoveryError<PlatformAdapterError, JsonRecoveryStoreError>),
}

impl PlatformProxyError {
    /// The stable machine-readable code the UI branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Setup(_) => "system_proxy_unavailable",
            Self::Recovery(_) => "system_proxy_failed",
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use magies_platform::macos_system_proxy::MacOsSystemProxyAdapter;

    use super::{PlatformProxySetupError, describe};

    pub const NETWORK_SERVICE_VARIABLE: &str = "MAGIES_MACOS_NETWORK_SERVICE";
    const DEFAULT_NETWORK_SERVICE: &str = "Wi-Fi";

    /// Builds the macOS adapter for the configured network service.
    ///
    /// # Errors
    ///
    /// Returns a setup error when the configured service name is empty.
    pub fn adapter() -> Result<MacOsSystemProxyAdapter, PlatformProxySetupError> {
        let network_service = std::env::var(NETWORK_SERVICE_VARIABLE)
            .unwrap_or_else(|_| DEFAULT_NETWORK_SERVICE.to_owned());
        MacOsSystemProxyAdapter::new(&network_service)
            .map_err(|source| PlatformProxySetupError::new(describe(&source)))
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use magies_platform::windows_system_proxy::WindowsSystemProxyAdapter;

    use super::PlatformProxySetupError;

    /// Builds the Windows registry adapter.
    ///
    /// # Errors
    ///
    /// Never fails; the registry adapter has no startup requirements.
    pub fn adapter() -> Result<WindowsSystemProxyAdapter, PlatformProxySetupError> {
        Ok(WindowsSystemProxyAdapter::new())
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use magies_platform::linux_system_proxy::LinuxSystemProxyAdapter;

    use super::{PlatformProxySetupError, describe};

    /// Builds the GNOME/GSettings adapter.
    ///
    /// # Errors
    ///
    /// Returns a setup error when the GNOME proxy schema is absent.
    pub fn adapter() -> Result<LinuxSystemProxyAdapter, PlatformProxySetupError> {
        LinuxSystemProxyAdapter::new()
            .map_err(|source| PlatformProxySetupError::new(describe(&source)))
    }
}
