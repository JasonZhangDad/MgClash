//! `MgClash` Tauri desktop shell.

pub mod core_control;
pub mod diagnostics;
pub mod node_latency;
pub mod platform_proxy;
pub mod session;
mod subscriptions;
mod tray;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
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
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::core_control::{LazySingBoxControl, describe};
use crate::diagnostics::DiagnosticBundle;
use crate::node_latency::{TcpLatencyError, probe_tcp};
use crate::platform_proxy::{PlatformProxyControl, PlatformProxyError, SystemProxyStartupStatus};
use crate::session::{SessionCommandError, SessionDefaults, SessionService, SessionStatus};
use crate::subscriptions::{
    DesktopSubscriptionController, DesktopSubscriptionError, DesktopSubscriptionSummary,
};
use crate::tray::{TrayAction, TrayUi, menu_model};

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

/// How often enabled subscriptions are checked for an elapsed update interval.
const SUBSCRIPTION_UPDATE_TICK: Duration = Duration::from_secs(60);

/// How often the native tray mirrors recovery-driven session changes.
const TRAY_REFRESH_TICK: Duration = Duration::from_secs(3);

/// A direct node endpoint test has five seconds to complete, including DNS.
const NODE_TEST_TIMEOUT: Duration = Duration::from_secs(5);

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum NodeTestStatus {
    Success,
    Timeout,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeTestResult {
    id: Uuid,
    status: NodeTestStatus,
    latency_ms: Option<u32>,
}

fn node_test_result(id: Uuid, result: &Result<u32, TcpLatencyError>) -> NodeTestResult {
    match result {
        Ok(latency_ms) => NodeTestResult {
            id,
            status: NodeTestStatus::Success,
            latency_ms: Some(*latency_ms),
        },
        Err(TcpLatencyError::Timeout) => NodeTestResult {
            id,
            status: NodeTestStatus::Timeout,
            latency_ms: None,
        },
        Err(_) => NodeTestResult {
            id,
            status: NodeTestStatus::Failed,
            latency_ms: None,
        },
    }
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
    tray: TrayUi,
    allow_exit: AtomicBool,
    exit_in_progress: AtomicBool,
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

/// Refreshes due subscriptions while the session is stopped. Runs for the
/// lifetime of the app so automatic updates do not depend on the window being
/// visible.
fn spawn_subscription_update_loop(
    subscriptions: Arc<DesktopSubscriptionController<PlatformSecretStore>>,
    service: Arc<Mutex<HostSessionService>>,
) {
    thread::spawn(move || {
        loop {
            thread::sleep(SUBSCRIPTION_UPDATE_TICK);
            if lock(&service).status().connected {
                continue;
            }

            let now = current_timestamp();
            let due_ids = match subscriptions.due_auto_update_ids(now) {
                Ok(ids) => ids,
                Err(error) => {
                    eprintln!(
                        "automatic subscription scheduling failed: {}",
                        describe(&error)
                    );
                    continue;
                }
            };

            let mut refreshed = false;
            for id in due_ids {
                if lock(&service).status().connected {
                    break;
                }
                match subscriptions.refresh(id, now) {
                    Ok(_) => refreshed = true,
                    Err(error) => eprintln!(
                        "automatic subscription refresh failed: {}",
                        describe(&error)
                    ),
                }
            }

            if refreshed {
                if let Err(error) = lock(&service).sync_selected_node() {
                    eprintln!(
                        "automatic subscription node sync failed: {}",
                        describe(&error)
                    );
                }
            }
        }
    });
}

fn spawn_tray_refresh_loop(app: AppHandle) {
    thread::spawn(move || {
        loop {
            thread::sleep(TRAY_REFRESH_TICK);
            if let Err(error) = refresh_tray(&app) {
                eprintln!("tray refresh failed: {}", error.message);
            }
        }
    });
}

fn refresh_tray(app: &AppHandle) -> Result<(), CommandError> {
    let state = app.state::<AppState>();
    let (status, nodes) = {
        let service = state.service();
        let status = service.status();
        let nodes = service.nodes().map_err(|error| command_error(&error))?;
        (status, nodes)
    };
    state
        .tray
        .refresh(app, menu_model(&status, &nodes))
        .map_err(|error| CommandError {
            code: "tray_update_failed",
            message: error.to_string(),
        })
}

fn handle_tray_action(app: &AppHandle, action: TrayAction) {
    if action == TrayAction::Open {
        show_main_window(app);
        return;
    }
    if action == TrayAction::Quit {
        request_app_exit(app);
        return;
    }

    let app = app.clone();
    thread::spawn(move || handle_background_tray_action(&app, action));
}

fn handle_background_tray_action(app: &AppHandle, action: TrayAction) {
    let result = match action {
        TrayAction::Open | TrayAction::Quit => Ok(()),
        TrayAction::Toggle => toggle_from_tray(app),
        TrayAction::SelectNode(id) => app
            .state::<AppState>()
            .service()
            .select_node(id)
            .map(|_| ())
            .map_err(|error| command_error(&error)),
    };

    match result {
        Ok(()) => {
            if let Err(error) = refresh_tray(app) {
                eprintln!("tray refresh failed: {}", error.message);
            }
        }
        Err(error) => {
            eprintln!("tray action failed: {}", error.message);
            app.state::<AppState>().tray.show_action_failure();
        }
    }
}

fn request_app_exit(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.exit_in_progress.swap(true, Ordering::AcqRel) {
        return;
    }

    let app = app.clone();
    thread::spawn(move || match disconnect_for_quit(&app) {
        Ok(()) => {
            app.state::<AppState>()
                .allow_exit
                .store(true, Ordering::Release);
            app.exit(0);
        }
        Err(error) => {
            eprintln!("application exit cleanup failed: {}", error.message);
            let state = app.state::<AppState>();
            state.exit_in_progress.store(false, Ordering::Release);
            state.tray.show_action_failure();
        }
    });
}

fn toggle_from_tray(app: &AppHandle) -> Result<(), CommandError> {
    let state = app.state::<AppState>();
    if state.service().status().connected {
        return state
            .service()
            .disconnect()
            .map(|_| ())
            .map_err(|error| command_error(&error));
    }

    let startup_status = state
        .system_proxy
        .startup_status()
        .map_err(|error| system_proxy_error(&error))?;
    ensure_system_proxy_ready(startup_status)?;
    state
        .service()
        .connect()
        .map(|_| ())
        .map_err(|error| command_error(&error))
}

fn disconnect_for_quit(app: &AppHandle) -> Result<(), CommandError> {
    let state = app.state::<AppState>();
    if !state.service().status().connected {
        return Ok(());
    }
    state
        .service()
        .disconnect()
        .map(|_| ())
        .map_err(|error| command_error(&error))
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window.show().and_then(|()| window.set_focus()) {
            eprintln!("main window could not be shown: {error}");
        }
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

#[tauri::command]
async fn session_test_node(
    id: String,
    state: State<'_, AppState>,
) -> Result<NodeTestResult, CommandError> {
    let id = parse_node_id(&id)?;
    let node = state
        .service()
        .node(id)
        .map_err(|error| command_error(&error))?;
    let probe = tauri::async_runtime::spawn_blocking(move || {
        probe_tcp(&node.server, node.port, NODE_TEST_TIMEOUT)
    })
    .await
    .map_err(|error| CommandError {
        code: "node_test_task_failed",
        message: error.to_string(),
    })?;
    let result = node_test_result(id, &probe);
    state
        .service()
        .record_node_latency(id, result.latency_ms, current_timestamp())
        .map_err(|error| command_error(&error))?;
    Ok(result)
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

#[tauri::command]
async fn subscription_refresh_all(
    state: State<'_, AppState>,
) -> Result<Vec<DesktopSubscriptionSummary>, CommandError> {
    ensure_subscription_mutation_ready(state.service().status().connected)?;
    let subscriptions = Arc::clone(&state.subscriptions);
    let summaries = tauri::async_runtime::spawn_blocking(move || {
        subscriptions.refresh_all(current_timestamp())
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
    Ok(summaries)
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
            let initial_tray_model = {
                let service = lock(&service);
                menu_model(&service.status(), &service.nodes()?)
            };
            let tray = TrayUi::install(app, initial_tray_model, handle_tray_action)?;
            app.manage(AppState {
                service: service.clone(),
                subscriptions: subscriptions.clone(),
                system_proxy,
                export_directory: data_directory,
                tray,
                allow_exit: AtomicBool::new(false),
                exit_in_progress: AtomicBool::new(false),
            });
            spawn_recovery_loop(
                service.clone(),
                TcpHealthProbe::new(health_address, PROBE_TIMEOUT),
            );
            spawn_subscription_update_loop(subscriptions.clone(), service.clone());
            spawn_tray_refresh_loop(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if let Err(error) = window.hide() {
                        eprintln!("main window could not be hidden: {error}");
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            platform_summary,
            session_status,
            session_import_node,
            session_nodes,
            session_test_node,
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
            subscription_refresh,
            subscription_refresh_all
        ])
        .build(tauri::generate_context!())
        .expect("failed to build MgClash desktop shell")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                let state = app.state::<AppState>();
                if !state.allow_exit.load(Ordering::Acquire) {
                    api.prevent_exit();
                    request_app_exit(app);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{
        NodeTestStatus, ensure_subscription_mutation_ready, ensure_system_proxy_ready,
        node_test_result, parse_node_id, parse_subscription_id, platform_summary,
    };
    use crate::node_latency::TcpLatencyError;
    use crate::platform_proxy::SystemProxyStartupStatus;
    use std::io;
    use uuid::Uuid;

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
    fn node_test_results_separate_success_timeout_and_failure() {
        let id = Uuid::nil();

        let success = node_test_result(id, &Ok(42));
        assert_eq!(success.status, NodeTestStatus::Success);
        assert_eq!(success.latency_ms, Some(42));

        let timeout = node_test_result(id, &Err(TcpLatencyError::Timeout));
        assert_eq!(timeout.status, NodeTestStatus::Timeout);
        assert_eq!(timeout.latency_ms, None);

        let failure = node_test_result(
            id,
            &Err(TcpLatencyError::Connect(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "refused",
            ))),
        );
        assert_eq!(failure.status, NodeTestStatus::Failed);
        assert_eq!(failure.latency_ms, None);
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
