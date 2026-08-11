//! `MgClash` Tauri desktop shell.

pub mod core_control;
pub mod diagnostics;
pub mod platform_proxy;
pub mod session;
mod subscriptions;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use magies_domain::TimestampMillis;
use magies_platform::network_path::NetworkPathReader;
use magies_platform::{TargetPlatform, TunAvailability};
use magies_profiles::{SqliteManualNodeStore, SqliteSubscriptionStore};
use magies_session::{DesktopSession, NetworkWatcher, TcpHealthProbe};
use magies_storage::PlatformSecretStore;
use serde::Serialize;
use tauri::{Manager, State};
use uuid::Uuid;

use crate::core_control::{LazySingBoxControl, describe};
use crate::diagnostics::DiagnosticBundle;
use crate::platform_proxy::{PlatformProxyControl, PlatformProxyError, SystemProxyStartupStatus};
use crate::session::{SessionCommandError, SessionDefaults, SessionService, SessionStatus};
use crate::subscriptions::{
    DesktopSubscriptionController, DesktopSubscriptionError, DesktopSubscriptionSummary,
};

/// How long a started Core has to accept connections on its local SOCKS port.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the recovery probe waits for the Core's local proxy port.
const PROBE_TIMEOUT: Duration = Duration::from_millis(500);

/// How often the recovery loop re-reads the host's default network path.
///
/// Polling is the price of `unsafe_code = "forbid"`: `NWPathMonitor` and its
/// Windows/Linux equivalents all need FFI. The loop skips the read entirely
/// while no session is running, so an idle app spawns no subprocesses.
const PATH_TICK: Duration = Duration::from_secs(5);

/// A wall-clock gap this much larger than `PATH_TICK` means the machine slept.
const SLEEP_THRESHOLD: Duration = Duration::from_secs(30);

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

struct AppState {
    service: Arc<Mutex<HostSessionService>>,
    subscriptions: Arc<DesktopSubscriptionController<PlatformSecretStore>>,
    system_proxy: PlatformProxyControl,
    /// Where an exported diagnostic bundle is written.
    export_directory: PathBuf,
}

impl AppState {
    fn service(&self) -> MutexGuard<'_, HostSessionService> {
        lock(&self.service)
    }
}

/// A poisoned lock means an earlier command panicked; the session state itself
/// stays valid, so recover it rather than break every later command.
fn lock(service: &Mutex<HostSessionService>) -> MutexGuard<'_, HostSessionService> {
    service.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Watches for network changes and wakes, and reconnects when the Core stopped
/// answering. Runs for the lifetime of the app.
fn spawn_recovery_loop(service: Arc<Mutex<HostSessionService>>, probe: TcpHealthProbe) {
    thread::spawn(move || {
        let reader = NetworkPathReader::for_host();
        let mut watcher = NetworkWatcher::new(PATH_TICK, SLEEP_THRESHOLD);
        loop {
            thread::sleep(watcher.tick_interval());

            let connected = lock(&service).status().connected;
            let fingerprint = if connected {
                reader.fingerprint()
            } else {
                None
            };
            if let Some(event) = watcher.tick(SystemTime::now(), fingerprint.as_deref()) {
                lock(&service).observe_network(event, Instant::now());
            }

            let now = Instant::now();
            let due = lock(&service)
                .recovery_due_at()
                .is_some_and(|due_at| now >= due_at);
            if due {
                if let Err(error) = lock(&service).recover(now, &probe) {
                    eprintln!("network recovery failed: {}", describe(&error));
                }
            }
        }
    });
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

fn system_proxy_error(error: &PlatformProxyError) -> CommandError {
    CommandError {
        code: error.code(),
        message: describe(error),
    }
}

fn subscription_error(error: &DesktopSubscriptionError) -> CommandError {
    CommandError {
        code: error.code(),
        message: describe(error),
    }
}

fn current_timestamp() -> TimestampMillis {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    TimestampMillis::new(i64::try_from(millis).unwrap_or(i64::MAX))
}

fn ensure_system_proxy_ready(status: SystemProxyStartupStatus) -> Result<(), CommandError> {
    match status {
        SystemProxyStartupStatus::Clean => Ok(()),
        SystemProxyStartupStatus::RestoreRequired => Err(CommandError {
            code: "system_proxy_recovery_required",
            message:
                "resolve the System Proxy settings left by the previous session before connecting"
                    .to_owned(),
        }),
    }
}

fn ensure_subscription_mutation_ready(connected: bool) -> Result<(), CommandError> {
    if connected {
        Err(CommandError {
            code: "session_active",
            message: "disconnect before changing or refreshing subscriptions".to_owned(),
        })
    } else {
        Ok(())
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
fn session_nodes(
    state: State<'_, AppState>,
) -> Result<Vec<crate::session::NodeSummary>, CommandError> {
    state
        .service()
        .nodes()
        .map_err(|error| command_error(&error))
}

fn parse_node_id(id: &str) -> Result<Uuid, CommandError> {
    Uuid::parse_str(id).map_err(|error| CommandError {
        code: "invalid_node_id",
        message: format!("invalid node identifier: {error}"),
    })
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State and deserialized arguments by value"
)]
fn session_select_node(
    id: String,
    state: State<'_, AppState>,
) -> Result<SessionStatus, CommandError> {
    let id = parse_node_id(&id)?;
    state
        .service()
        .select_node(id)
        .map_err(|error| command_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State and deserialized arguments by value"
)]
fn session_delete_node(
    id: String,
    state: State<'_, AppState>,
) -> Result<SessionStatus, CommandError> {
    let id = parse_node_id(&id)?;
    state
        .service()
        .delete_node(id)
        .map_err(|error| command_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn session_connect(state: State<'_, AppState>) -> Result<SessionStatus, CommandError> {
    let startup_status = state
        .system_proxy
        .startup_status()
        .map_err(|error| system_proxy_error(&error))?;
    ensure_system_proxy_ready(startup_status)?;
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

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn system_proxy_startup_status(
    state: State<'_, AppState>,
) -> Result<SystemProxyStartupStatus, CommandError> {
    state
        .system_proxy
        .startup_status()
        .map_err(|error| system_proxy_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn system_proxy_recover(
    state: State<'_, AppState>,
) -> Result<SystemProxyStartupStatus, CommandError> {
    state
        .system_proxy
        .recover_startup()
        .map_err(|error| system_proxy_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn system_proxy_dismiss(
    state: State<'_, AppState>,
) -> Result<SystemProxyStartupStatus, CommandError> {
    state
        .system_proxy
        .dismiss_startup()
        .map_err(|error| system_proxy_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn export_diagnostics(state: State<'_, AppState>) -> Result<PathBuf, CommandError> {
    let bundle = {
        let service = state.service();
        DiagnosticBundle::collect(&service.status(), service.runtime_config_path())
    };
    bundle
        .write_to(&state.export_directory)
        .map_err(|error| CommandError {
            code: error.code(),
            message: describe(&error),
        })
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn subscription_list(
    state: State<'_, AppState>,
) -> Result<Vec<DesktopSubscriptionSummary>, CommandError> {
    state
        .subscriptions
        .list()
        .map_err(|error| subscription_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State and deserialized arguments by value"
)]
fn subscription_create(
    name: String,
    url: String,
    update_interval_minutes: u32,
    auto_update: bool,
    state: State<'_, AppState>,
) -> Result<DesktopSubscriptionSummary, CommandError> {
    state
        .subscriptions
        .create(&name, &url, update_interval_minutes, auto_update)
        .map_err(|error| subscription_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State and deserialized arguments by value"
)]
fn subscription_update(
    id: String,
    name: String,
    update_interval_minutes: u32,
    auto_update: bool,
    enabled: bool,
    url: Option<String>,
    state: State<'_, AppState>,
) -> Result<DesktopSubscriptionSummary, CommandError> {
    ensure_subscription_mutation_ready(state.service().status().connected)?;
    let id = parse_subscription_id(&id)?;
    let summary = state
        .subscriptions
        .update(
            id,
            &name,
            update_interval_minutes,
            auto_update,
            enabled,
            url.as_deref(),
        )
        .map_err(|error| subscription_error(&error))?;
    state
        .service()
        .sync_selected_node()
        .map_err(|error| command_error(&error))?;
    Ok(summary)
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State and deserialized arguments by value"
)]
fn subscription_delete(id: String, state: State<'_, AppState>) -> Result<(), CommandError> {
    ensure_subscription_mutation_ready(state.service().status().connected)?;
    let id = parse_subscription_id(&id)?;
    state
        .subscriptions
        .delete(id)
        .map_err(|error| subscription_error(&error))?;
    state
        .service()
        .sync_selected_node()
        .map_err(|error| command_error(&error))?;
    Ok(())
}

#[tauri::command]
async fn subscription_refresh(
    id: String,
    state: State<'_, AppState>,
) -> Result<DesktopSubscriptionSummary, CommandError> {
    ensure_subscription_mutation_ready(state.service().status().connected)?;
    let id = parse_subscription_id(&id)?;
    let subscriptions = Arc::clone(&state.subscriptions);
    let summary = tauri::async_runtime::spawn_blocking(move || {
        subscriptions.refresh(id, current_timestamp())
    })
    .await
    .map_err(|error| CommandError {
        code: "subscription_task_failed",
        message: error.to_string(),
    })?
    .map_err(|error| subscription_error(&error))?;
    state
        .service()
        .sync_selected_node()
        .map_err(|error| command_error(&error))?;
    Ok(summary)
}

fn parse_subscription_id(id: &str) -> Result<Uuid, CommandError> {
    Uuid::parse_str(id).map_err(|error| CommandError {
        code: "invalid_subscription_id",
        message: format!("invalid subscription identifier: {error}"),
    })
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
            let system_proxy =
                PlatformProxyControl::for_host(data_directory.join("system-proxy-recovery.json"));
            let session = DesktopSession::new(
                PlatformSecretStore,
                LazySingBoxControl::from_env(health_address, HEALTH_TIMEOUT),
                system_proxy.clone(),
                runtime_directory,
            );
            let node_database = data_directory.join("nodes.sqlite");
            let nodes = SqliteManualNodeStore::open(&node_database)?;
            let subscriptions = Arc::new(DesktopSubscriptionController::new(
                SqliteSubscriptionStore::open(&node_database)?,
                PlatformSecretStore,
            ));
            let service = Arc::new(Mutex::new(SessionService::new(
                session,
                defaults,
                nodes,
                SqliteSubscriptionStore::open(&node_database)?,
            )?));
            spawn_recovery_loop(
                service.clone(),
                TcpHealthProbe::new(health_address, PROBE_TIMEOUT),
            );
            app.manage(AppState {
                service,
                subscriptions,
                system_proxy,
                export_directory: data_directory,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            platform_summary,
            session_status,
            session_import_node,
            session_nodes,
            session_select_node,
            session_delete_node,
            session_connect,
            session_disconnect,
            system_proxy_startup_status,
            system_proxy_recover,
            system_proxy_dismiss,
            export_diagnostics,
            subscription_list,
            subscription_create,
            subscription_update,
            subscription_delete,
            subscription_refresh
        ])
        .run(tauri::generate_context!())
        .expect("failed to run MgClash desktop shell");
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_subscription_mutation_ready, ensure_system_proxy_ready, parse_node_id,
        parse_subscription_id, platform_summary,
    };
    use crate::platform_proxy::SystemProxyStartupStatus;

    #[test]
    fn command_supports_the_build_host() {
        platform_summary().expect("CI must run on a supported V0.1 target");
    }

    #[test]
    fn connection_waits_for_startup_system_proxy_recovery() {
        ensure_system_proxy_ready(SystemProxyStartupStatus::Clean).unwrap();

        let error =
            ensure_system_proxy_ready(SystemProxyStartupStatus::RestoreRequired).unwrap_err();
        assert_eq!(error.code, "system_proxy_recovery_required");
    }

    #[test]
    fn node_commands_reject_an_invalid_identifier() {
        let error = parse_node_id("not-a-uuid").unwrap_err();

        assert_eq!(error.code, "invalid_node_id");
    }

    #[test]
    fn subscription_commands_reject_an_invalid_identifier() {
        let error = parse_subscription_id("not-a-uuid").unwrap_err();

        assert_eq!(error.code, "invalid_subscription_id");
    }

    #[test]
    fn subscription_mutations_wait_for_disconnect() {
        ensure_subscription_mutation_ready(false).unwrap();

        let error = ensure_subscription_mutation_ready(true).unwrap_err();

        assert_eq!(error.code, "session_active");
    }
}
