//! `MgClash` Tauri desktop shell.

pub mod core_control;
pub mod platform_proxy;
pub mod session;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use magies_platform::{TargetPlatform, TunAvailability};
use magies_session::DesktopSession;
use magies_storage::PlatformSecretStore;
use serde::Serialize;
use tauri::{Manager, State};

use crate::core_control::{LazySingBoxControl, describe};
use crate::platform_proxy::PlatformProxyControl;
use crate::session::{SessionCommandError, SessionDefaults, SessionService, SessionStatus};

/// How long a started Core has to accept connections on its local SOCKS port.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformSummary {
    pub artifact_identifier: &'static str,
    pub tun_availability: &'static str,
}

/// The shape every failed command returns: a stable code for the UI to branch
/// on and a human-readable cause chain.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

/// Creates the serializable platform summary returned to the desktop UI.
///
/// # Errors
///
/// Returns a typed `unsupported_target` error when the OS/CPU pair is outside
/// the V0.1 support matrix.
pub fn platform_summary_for(os: &str, architecture: &str) -> Result<PlatformSummary, CommandError> {
    let target = TargetPlatform::parse(os, architecture).map_err(|error| CommandError {
        code: "unsupported_target",
        message: error.to_string(),
    })?;
    let tun_availability = match target.unsigned_tun_availability() {
        TunAvailability::UnavailableInUnsignedBuild => "unavailableInUnsignedBuild",
        TunAvailability::RequiresElevation => "requiresElevation",
    };

    Ok(PlatformSummary {
        artifact_identifier: target.artifact_identifier(),
        tun_availability,
    })
}

#[tauri::command]
fn platform_summary() -> Result<PlatformSummary, CommandError> {
    platform_summary_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// The session the desktop shell drives on the host platform.
type HostSessionService =
    SessionService<PlatformSecretStore, LazySingBoxControl, PlatformProxyControl>;

struct AppState(Mutex<HostSessionService>);

impl AppState {
    /// A poisoned lock means an earlier command panicked; the session state
    /// itself stays valid, so recover it rather than break every later command.
    fn service(&self) -> std::sync::MutexGuard<'_, HostSessionService> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Keeps the developer-facing cause chain out of the UI's control flow: the UI
/// branches on `code` and only displays `message`.
fn command_error<C, P>(error: &SessionCommandError<C, P>) -> CommandError
where
    C: std::error::Error + 'static,
    P: std::error::Error + 'static,
{
    CommandError {
        code: error.code(),
        message: describe(error),
    }
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn session_status(state: State<'_, AppState>) -> SessionStatus {
    state.service().status()
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State and deserialized arguments by value"
)]
fn session_import_node(
    uri: String,
    state: State<'_, AppState>,
) -> Result<SessionStatus, CommandError> {
    state
        .service()
        .import_node(&uri)
        .map_err(|error| command_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn session_connect(state: State<'_, AppState>) -> Result<SessionStatus, CommandError> {
    state
        .service()
        .connect()
        .map_err(|error| command_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn session_disconnect(state: State<'_, AppState>) -> Result<SessionStatus, CommandError> {
    state
        .service()
        .disconnect()
        .map_err(|error| command_error(&error))
}

/// Starts the desktop application event loop.
///
/// # Panics
///
/// Panics when Tauri cannot initialize or run the desktop shell.
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_directory = app.path().app_data_dir()?;
            let runtime_directory = data_directory.join("runtime");
            std::fs::create_dir_all(&runtime_directory)?;

            let defaults = SessionDefaults::v01();
            let health_address =
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), defaults.socks.port().get());
            let session = DesktopSession::new(
                PlatformSecretStore,
                LazySingBoxControl::from_env(health_address, HEALTH_TIMEOUT),
                PlatformProxyControl::for_host(data_directory.join("system-proxy-recovery.json")),
                runtime_directory,
            );
            app.manage(AppState(Mutex::new(SessionService::new(session, defaults))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            platform_summary,
            session_status,
            session_import_node,
            session_connect,
            session_disconnect
        ])
        .run(tauri::generate_context!())
        .expect("failed to run MgClash desktop shell");
}

#[cfg(test)]
mod tests {
    use super::platform_summary;

    #[test]
    fn command_supports_the_build_host() {
        platform_summary().expect("CI must run on a supported V0.1 target");
    }
}
