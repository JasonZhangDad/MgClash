//! `MgClash` Tauri desktop shell.

pub mod app_settings;
pub mod core_control;
pub mod diagnostics;
pub mod dns_settings;
pub mod logs;
pub mod node_latency;
pub mod platform_proxy;
pub mod route_settings;
pub mod routing_mode;
pub mod session;
mod subscriptions;
pub mod traffic;
mod tray;
pub mod url_test;

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::Local;
use magies_domain::TimestampMillis;
use magies_platform::network_path::NetworkPathReader;
use magies_platform::{TargetPlatform, TunAvailability};
use magies_profiles::{
    ManualNodeDraft, ShareLinkQrScanError, ShareLinkQrScanner, SqliteManualNodeStore,
    SqliteNodeGroupStore, SqliteNodeOrderStore, SqliteSubscriptionStore,
};
use magies_session::{DesktopSession, NetworkWatcher, RecoveryOutcome, TcpHealthProbe};
use magies_storage::PlatformSecretStore;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::app_settings::{AppSettings, SqliteAppSettingsStore, SystemProxyModeSetting};
use crate::core_control::{HostCoreControl, describe};
use crate::diagnostics::DiagnosticBundle;
use crate::dns_settings::{DnsSettings, SqliteDnsSettingsStore};
use crate::logs::{
    LogBuffer, LogBufferLayer, LogEntry, LogLevel, LogSource, spawn_core_log_reader,
};
use crate::node_latency::{TcpLatencyError, probe_tcp};
use crate::platform_proxy::{PlatformProxyControl, PlatformProxyError, SystemProxyStartupStatus};
use crate::route_settings::{RouteSettings, SqliteRouteSettingsStore};
use crate::routing_mode::{SqliteRoutingModeStore, parse_routing_mode};
use crate::session::{
    NodeMoveDirection, NodeStores, NodeSummary, SessionCommandError, SessionDefaults,
    SessionService, SessionStatus,
};
use crate::subscriptions::{
    DesktopSubscriptionController, DesktopSubscriptionError, DesktopSubscriptionSummary,
};
use crate::traffic::{
    NodeTraffic, SqliteTrafficCounter, TrafficCounterError, TrafficSnapshot, sample_traffic,
};
use crate::tray::{TrayAction, TrayUi, menu_model};
use crate::url_test::{UrlTestError, probe_url};
use magies_platform::pac::{PacScript, PacServer};

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

/// A real URL test uses the same per-node timeout as TCP Connect.
const URL_TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// The Core emits a traffic sample every second; keep the PRD's bounded timeout.
const TRAFFIC_SAMPLE_TIMEOUT: Duration = Duration::from_secs(5);

/// Avoid a hot retry loop while the session is stopped or the Core API fails.
const TRAFFIC_RETRY_DELAY: Duration = Duration::from_secs(1);

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

fn url_test_result(
    id: Uuid,
    result: &Result<u32, UrlTestError>,
) -> Result<NodeTestResult, CommandError> {
    match result {
        Ok(latency_ms) => Ok(NodeTestResult {
            id,
            status: NodeTestStatus::Success,
            latency_ms: Some(*latency_ms),
        }),
        Err(UrlTestError::TimedOut) => Ok(NodeTestResult {
            id,
            status: NodeTestStatus::Timeout,
            latency_ms: None,
        }),
        Err(
            error @ (UrlTestError::InvalidTimeout
            | UrlTestError::InvalidUrl(_)
            | UrlTestError::UnsupportedScheme { .. }),
        ) => Err(CommandError {
            code: "invalid_url_test",
            message: describe(error),
        }),
        Err(_) => Ok(NodeTestResult {
            id,
            status: NodeTestStatus::Failed,
            latency_ms: None,
        }),
    }
}

fn traffic_counter_command_error(error: &TrafficCounterError) -> CommandError {
    CommandError {
        code: match error {
            TrafficCounterError::CounterOverflow => "traffic_counter_overflow",
            _ => "traffic_persist_failed",
        },
        message: describe(error),
    }
}

fn open_traffic_counter(
    path: &std::path::Path,
) -> Result<SqliteTrafficCounter, TrafficCounterError> {
    SqliteTrafficCounter::open(path, Local::now().date_naive(), Instant::now())
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
    SessionService<PlatformSecretStore, HostCoreControl, PlatformProxyControl>;

struct AppState {
    service: Arc<Mutex<HostSessionService>>,
    subscriptions: Arc<DesktopSubscriptionController<PlatformSecretStore>>,
    traffic: Arc<Mutex<SqliteTrafficCounter>>,
    system_proxy: PlatformProxyControl,
    logs: Arc<LogBuffer>,
    settings_store: Mutex<SqliteAppSettingsStore>,
    /// Cached so the window and exit handlers can read the settings without
    /// touching `SQLite` on every event.
    settings: Mutex<AppSettings>,
    /// Serves the proxy auto-configuration script while PAC mode is selected.
    ///
    /// Held only while that mode is in use: a server nobody asked for would
    /// keep a loopback port bound for the whole session.
    pac: Mutex<Option<PacServer>>,
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

    fn traffic(&self) -> MutexGuard<'_, SqliteTrafficCounter> {
        lock(&self.traffic)
    }
}

/// A poisoned lock means an earlier command panicked; the session state itself
/// stays valid, so recover it rather than break every later command.
fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Samples Core traffic for the lifetime of the app, independent of whether
/// the main window is visible.
fn spawn_traffic_loop(
    service: Arc<Mutex<HostSessionService>>,
    traffic: Arc<Mutex<SqliteTrafficCounter>>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            let api_address = lock(&service).traffic_api_address().ok();
            let Some(api_address) = api_address else {
                if let Err(error) = lock(&traffic).tick(Local::now().date_naive(), Instant::now()) {
                    tracing::warn!(
                        "traffic counters could not be persisted: {}",
                        describe(&error)
                    );
                }
                tokio::time::sleep(TRAFFIC_RETRY_DELAY).await;
                continue;
            };

            match sample_traffic(api_address, TRAFFIC_SAMPLE_TIMEOUT).await {
                Ok(rate) => {
                    // Read together with the address check: attributing this
                    // second's bytes to a node the session has already moved off
                    // would credit them to the wrong row.
                    let (still_current, node) = {
                        let service = lock(&service);
                        (
                            service.traffic_api_address().ok() == Some(api_address),
                            service.status().node.map(|node| node.id),
                        )
                    };
                    let result = if still_current {
                        lock(&traffic).record(Local::now().date_naive(), rate, Instant::now(), node)
                    } else {
                        lock(&traffic).tick(Local::now().date_naive(), Instant::now())
                    };
                    if let Err(error) = result {
                        tracing::warn!(
                            "traffic counters could not be updated: {}",
                            describe(&error)
                        );
                    }
                }
                Err(error) => {
                    tracing::warn!("traffic sample failed: {}", describe(&error));
                    if let Err(error) =
                        lock(&traffic).tick(Local::now().date_naive(), Instant::now())
                    {
                        tracing::warn!(
                            "traffic counters could not be persisted: {}",
                            describe(&error)
                        );
                    }
                    tokio::time::sleep(TRAFFIC_RETRY_DELAY).await;
                }
            }
        }
    });
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
                tracing::info!(?event, "network change observed");
                lock(&service).observe_network(event, Instant::now());
            }

            match lock(&service).monitor_recovery(Instant::now(), &probe) {
                Ok(outcome) => {
                    // Only a real reconnect is worth a line; the common case is
                    // a healthy probe that would otherwise flood the panel.
                    if !matches!(outcome, RecoveryOutcome::Idle | RecoveryOutcome::Healthy) {
                        tracing::info!(?outcome, "session recovery acted");
                    }
                }
                Err(error) => {
                    tracing::error!("session recovery failed: {}", describe(&error));
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
                    tracing::warn!(
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
                    Err(error) => tracing::warn!(
                        "automatic subscription refresh failed: {}",
                        describe(&error)
                    ),
                }
            }

            if refreshed {
                if let Err(error) = lock(&service).sync_selected_node() {
                    tracing::warn!(
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
                tracing::warn!("tray refresh failed: {}", error.message);
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
    let traffic = state.traffic().snapshot();
    state
        .tray
        .refresh(app, menu_model(&status, &nodes, traffic))
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
        TrayAction::SetRoutingMode(mode) => app
            .state::<AppState>()
            .service()
            .set_routing_mode(mode)
            .map(|_| ())
            .map_err(|error| command_error(&error)),
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
                tracing::warn!("tray refresh failed: {}", error.message);
            }
        }
        Err(error) => {
            tracing::warn!("tray action failed: {}", error.message);
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
            tracing::error!("application exit cleanup failed: {}", error.message);
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
    let mut service = state.service();
    let status = service.connect().map_err(|error| command_error(&error))?;
    // The tray is a second way in; without this the Core's output would be
    // dropped for every session started from the menu.
    if let Some(output) = service.take_core_output() {
        spawn_core_log_reader(output, state.logs.clone());
    }
    drop(service);
    tracing::info!(
        node = status.node.as_ref().map_or("", |node| &node.name),
        "session connected from the tray"
    );
    Ok(())
}

fn disconnect_for_quit(app: &AppHandle) -> Result<(), CommandError> {
    let state = app.state::<AppState>();
    if state.service().status().connected {
        state
            .service()
            .disconnect()
            .map_err(|error| command_error(&error))?;
    }
    state
        .traffic()
        .flush()
        .map_err(|error| traffic_counter_command_error(&error))
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window.show().and_then(|()| window.set_focus()) {
            tracing::warn!("main window could not be shown: {error}");
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
fn session_set_routing_mode(
    mode: String,
    state: State<'_, AppState>,
) -> Result<SessionStatus, CommandError> {
    let mode = parse_routing_mode(&mode).map_err(|error| CommandError {
        code: "invalid_routing_mode",
        message: error.to_string(),
    })?;
    state
        .service()
        .set_routing_mode(mode)
        .map_err(|error| command_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State and deserialized arguments by value"
)]
fn session_set_route_settings(
    settings: RouteSettings,
    state: State<'_, AppState>,
) -> Result<SessionStatus, CommandError> {
    state
        .service()
        .set_route_settings(settings)
        .map_err(|error| command_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State and deserialized arguments by value"
)]
fn session_set_dns_settings(
    settings: DnsSettings,
    state: State<'_, AppState>,
) -> Result<SessionStatus, CommandError> {
    state
        .service()
        .set_dns_settings(settings)
        .map_err(|error| command_error(&error))
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
    reason = "Tauri commands receive State and deserialized arguments by value"
)]
fn session_import_nodes(
    content: String,
    state: State<'_, AppState>,
) -> Result<crate::session::BulkImportReport, CommandError> {
    state
        .service()
        .import_nodes(content.as_bytes())
        .map_err(|error| command_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State and deserialized arguments by value"
)]
fn session_create_node(
    draft: ManualNodeDraft,
    state: State<'_, AppState>,
) -> Result<SessionStatus, CommandError> {
    state
        .service()
        .create_node(draft)
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
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn session_node_groups(
    state: State<'_, AppState>,
) -> Result<Vec<crate::session::NodeGroupSummary>, CommandError> {
    state
        .service()
        .node_groups()
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

#[tauri::command]
async fn session_url_test(
    url: String,
    state: State<'_, AppState>,
) -> Result<NodeTestResult, CommandError> {
    let target = state
        .service()
        .url_test_target()
        .map_err(|error| command_error(&error))?;
    let proxy_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), target.http_port);
    let probe = probe_url(&url, proxy_address, URL_TEST_TIMEOUT).await;
    if let Err(error) = &probe {
        tracing::debug!("URL test did not succeed: {}", describe(error));
    }
    let result = url_test_result(target.node_id, &probe)?;
    state
        .service()
        .record_node_latency(target.node_id, result.latency_ms, current_timestamp())
        .map_err(|error| command_error(&error))?;
    Ok(result)
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn session_traffic(state: State<'_, AppState>) -> TrafficSnapshot {
    state.traffic().snapshot()
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
fn session_edit_node(
    id: String,
    name: String,
    server: String,
    port: u32,
    state: State<'_, AppState>,
) -> Result<SessionStatus, CommandError> {
    let id = parse_node_id(&id)?;
    state
        .service()
        .edit_node(id, name, server, port)
        .map_err(|error| command_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State and deserialized arguments by value"
)]
fn session_move_node(
    id: String,
    direction: NodeMoveDirection,
    state: State<'_, AppState>,
) -> Result<Vec<crate::session::NodeSummary>, CommandError> {
    let id = parse_node_id(&id)?;
    state
        .service()
        .move_node(id, direction)
        .map_err(|error| command_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State and deserialized arguments by value"
)]
fn session_set_node_group(
    id: String,
    group_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::session::NodeSummary>, CommandError> {
    let id = parse_node_id(&id)?;
    state
        .service()
        .set_node_group(id, group_name.as_deref())
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
fn session_node_traffic(state: State<'_, AppState>) -> HashMap<String, NodeTraffic> {
    lock(&state.traffic)
        .node_totals()
        .into_iter()
        .map(|(id, totals)| (id.to_string(), totals))
        .collect()
}

/// Reads a sharing link out of a QR code image the user picked.
///
/// Takes the bytes rather than a path: the webview reads the file the user chose
/// through the picker, and handing the shell a path would let it read anything.
#[tauri::command]
fn session_read_qr_code(image: &[u8]) -> Result<String, CommandError> {
    ShareLinkQrScanner::read(image).map_err(|error| CommandError {
        code: match error {
            ShareLinkQrScanError::UnreadableImage { .. } => "qr_image_unreadable",
            ShareLinkQrScanError::NoCodeFound => "qr_code_not_found",
        },
        message: describe(&error),
    })
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn session_node_qr_code(id: String, state: State<'_, AppState>) -> Result<String, CommandError> {
    let id = parse_node_id(&id)?;
    state
        .service()
        .node_qr_code(id)
        .map_err(|error| command_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn session_clone_node(
    id: String,
    state: State<'_, AppState>,
) -> Result<Vec<NodeSummary>, CommandError> {
    let id = parse_node_id(&id)?;
    state
        .service()
        .clone_node(id)
        .map_err(|error| command_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn session_remove_duplicate_nodes(state: State<'_, AppState>) -> Result<usize, CommandError> {
    state
        .service()
        .remove_duplicate_nodes()
        .map_err(|error| command_error(&error))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn session_export_node_link(
    id: String,
    state: State<'_, AppState>,
) -> Result<String, CommandError> {
    let id = parse_node_id(&id)?;
    state
        .service()
        .export_node_link(id)
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
    let mut service = state.service();
    let status = service.connect().map_err(|error| command_error(&error))?;
    if let Some(output) = service.take_core_output() {
        spawn_core_log_reader(output, state.logs.clone());
    }
    drop(service);
    tracing::info!(
        node = status.node.as_ref().map_or("", |node| &node.name),
        "session connected"
    );
    Ok(status)
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn app_settings(state: State<'_, AppState>) -> AppSettings {
    *lock(&state.settings)
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State and deserialized arguments by value"
)]
fn set_app_settings(
    app: AppHandle,
    settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<AppSettings, CommandError> {
    // Applied before saving: a login item the OS refused must not be recorded
    // as enabled, or the switch would lie after the next launch.
    apply_launch_at_login(&app, settings.launch_at_login)?;
    lock(&state.settings_store)
        .save(settings)
        .map_err(|error| CommandError {
            code: "app_settings_store_failed",
            message: describe(&error),
        })?;
    *lock(&state.settings) = settings;
    let pac_url = apply_pac_mode(&state, settings)?;
    // The running service keeps its own copy so status and connect do not have
    // to reach back into the settings on every call.
    {
        let mut service = state.service();
        service.set_core_preference(settings.core_preference.preference());
        service.set_tun_enabled(settings.tun_enabled);
        service.set_system_proxy_mode(settings.system_proxy_mode.mode(pac_url.as_deref()));
    }
    tracing::info!("application settings updated");
    Ok(settings)
}

/// Starts or stops the PAC server to match the selected mode.
///
/// Returns the URL to point the host at, or `None` when PAC is not selected.
fn apply_pac_mode(state: &AppState, settings: AppSettings) -> Result<Option<String>, CommandError> {
    let mut server = lock(&state.pac);
    if settings.system_proxy_mode != SystemProxyModeSetting::Pac {
        // Dropping it releases the port and joins the accept thread.
        *server = None;
        return Ok(None);
    }
    if let Some(running) = server.as_ref() {
        return Ok(Some(running.url()));
    }

    let defaults = state.service().defaults().clone();
    let started = PacServer::start(&PacScript::global(
        defaults.socks.port().get(),
        defaults.http.port().get(),
    ))
    .map_err(|error| CommandError {
        code: "pac_server_failed",
        message: describe(&error),
    })?;
    let url = started.url();
    tracing::info!("serving the proxy auto-configuration script at {url}");
    *server = Some(started);
    Ok(Some(url))
}

/// Registers or removes the OS login item.
fn apply_launch_at_login(app: &AppHandle, enabled: bool) -> Result<(), CommandError> {
    use tauri_plugin_autostart::ManagerExt;

    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    result.map_err(|error| CommandError {
        code: "launch_at_login_failed",
        message: describe(&error),
    })
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State and deserialized arguments by value"
)]
fn session_logs(
    level: Option<LogLevel>,
    source: Option<LogSource>,
    state: State<'_, AppState>,
) -> Vec<LogEntry> {
    state.logs.snapshot(level.unwrap_or(LogLevel::Info), source)
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn session_clear_logs(state: State<'_, AppState>) {
    state.logs.clear();
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive State by value"
)]
fn session_disconnect(state: State<'_, AppState>) -> Result<SessionStatus, CommandError> {
    let status = state
        .service()
        .disconnect()
        .map_err(|error| command_error(&error))?;
    tracing::info!("session disconnected");
    Ok(status)
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

fn handle_run_event(app: &AppHandle, event: tauri::RunEvent) {
    if let tauri::RunEvent::ExitRequested { api, .. } = event {
        let state = app.state::<AppState>();
        if !state.allow_exit.load(Ordering::Acquire) {
            api.prevent_exit();
            request_app_exit(app);
        }
    }
}

/// Opens the on-disk stores, wires the session service into Tauri state, and
/// starts the background loops.
fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let logs = Arc::new(LogBuffer::default());
    install_log_subscriber(logs.clone());
    let data_directory = app.path().app_data_dir()?;
    let runtime_directory = data_directory.join("runtime");
    std::fs::create_dir_all(&runtime_directory)?;
    let defaults = SessionDefaults::v01();
    let health_address = SocketAddr::from(([127, 0, 0, 1], defaults.socks.port().get()));
    let system_proxy =
        PlatformProxyControl::for_host(data_directory.join("system-proxy-recovery.json"));
    let session = DesktopSession::new(
        PlatformSecretStore,
        HostCoreControl::from_env(health_address, HEALTH_TIMEOUT),
        system_proxy.clone(),
        runtime_directory,
    );
    let node_database = data_directory.join("nodes.sqlite");
    let nodes = SqliteManualNodeStore::open(&node_database)?;
    let subscriptions = Arc::new(DesktopSubscriptionController::new(
        SqliteSubscriptionStore::open(&node_database)?,
        PlatformSecretStore,
    ));
    let traffic = Arc::new(Mutex::new(open_traffic_counter(&node_database)?));
    let service = Arc::new(Mutex::new(SessionService::new(
        session,
        defaults,
        NodeStores::new(
            nodes,
            SqliteSubscriptionStore::open(&node_database)?,
            SqliteNodeOrderStore::open(&node_database)?,
            SqliteNodeGroupStore::open(&node_database)?,
        ),
        SqliteRoutingModeStore::open(&node_database)?,
        SqliteRouteSettingsStore::open(&node_database)?,
        SqliteDnsSettingsStore::open(&node_database)?,
    )?));
    let settings_store = SqliteAppSettingsStore::open(&node_database)?;
    let settings = settings_store.load()?;
    // Restored before the service reads the mode, so a session started on launch
    // points at a script that is already being served.
    let pac = if settings.system_proxy_mode == SystemProxyModeSetting::Pac {
        let defaults = lock(&service).defaults().clone();
        Some(PacServer::start(&PacScript::global(
            defaults.socks.port().get(),
            defaults.http.port().get(),
        ))?)
    } else {
        None
    };
    {
        let mut service = lock(&service);
        service.set_core_preference(settings.core_preference.preference());
        service.set_tun_enabled(settings.tun_enabled);
        service.set_system_proxy_mode(
            settings
                .system_proxy_mode
                .mode(pac.as_ref().map(PacServer::url).as_deref()),
        );
    }
    let initial_tray_model = {
        let (service, traffic) = (lock(&service), lock(&traffic).snapshot());
        menu_model(&service.status(), &service.nodes()?, traffic)
    };
    let tray = TrayUi::install(app, initial_tray_model, handle_tray_action)?;
    app.manage(AppState {
        service: service.clone(),
        subscriptions: subscriptions.clone(),
        traffic: traffic.clone(),
        system_proxy,
        logs: logs.clone(),
        settings_store: Mutex::new(settings_store),
        settings: Mutex::new(settings),
        pac: Mutex::new(pac),
        export_directory: data_directory,
        tray,
        allow_exit: AtomicBool::new(false),
        exit_in_progress: AtomicBool::new(false),
    });
    if settings.connect_on_launch {
        connect_on_launch(app.handle());
    }
    let probe = TcpHealthProbe::new(health_address, PROBE_TIMEOUT);
    spawn_recovery_loop(service.clone(), probe);
    spawn_subscription_update_loop(subscriptions.clone(), service.clone());
    spawn_traffic_loop(service.clone(), traffic);
    spawn_tray_refresh_loop(app.handle().clone());
    Ok(())
}

/// Starts a session on launch without blocking the setup hook.
///
/// A failure here is logged rather than surfaced: the user did not press
/// anything, so an error dialog on startup would be noise. The panel and the
/// tray both show the session stayed down.
fn connect_on_launch(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        let state = app.state::<AppState>();
        let startup_status = match state.system_proxy.startup_status() {
            Ok(status) => status,
            Err(error) => {
                tracing::warn!("connect on launch skipped: {}", describe(&error));
                return;
            }
        };
        if !matches!(startup_status, SystemProxyStartupStatus::Clean) {
            tracing::warn!("connect on launch skipped: System Proxy needs recovery first");
            return;
        }

        let mut service = state.service();
        match service.connect() {
            Ok(status) => {
                if let Some(output) = service.take_core_output() {
                    spawn_core_log_reader(output, state.logs.clone());
                }
                drop(service);
                tracing::info!(
                    node = status.node.as_ref().map_or("", |node| &node.name),
                    "connected on launch"
                );
            }
            Err(error) => {
                drop(service);
                tracing::warn!("connect on launch failed: {}", describe(&error));
            }
        }
    });
}

/// Routes `tracing` events into the buffer the log panel reads.
///
/// A failure here means another subscriber is already installed, which only
/// happens in a test harness; the app keeps running without in-app app logs
/// rather than refusing to start.
fn install_log_subscriber(logs: Arc<LogBuffer>) {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    if tracing_subscriber::registry()
        .with(LogBufferLayer::new(logs))
        .try_init()
        .is_err()
    {
        eprintln!("log subscriber was already installed; in-app application logs are disabled");
    }
}

/// Starts the desktop application event loop.
///
/// # Panics
///
/// Panics when Tauri cannot initialize or run the desktop shell.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(setup_app)
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    // With close-to-tray off, the close button really quits, so
                    // the default handling is left alone and the app exits
                    // through the same cleanup path as the tray's Quit item.
                    if lock(&window.state::<AppState>().settings).close_to_tray {
                        api.prevent_close();
                        if let Err(error) = window.hide() {
                            tracing::warn!("main window could not be hidden: {error}");
                        }
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            platform_summary,
            session_status,
            session_set_routing_mode,
            session_set_route_settings,
            session_set_dns_settings,
            session_import_node,
            session_import_nodes,
            session_create_node,
            session_nodes,
            session_node_groups,
            session_test_node,
            session_url_test,
            session_traffic,
            session_select_node,
            session_edit_node,
            session_move_node,
            session_set_node_group,
            session_delete_node,
            session_export_node_link,
            session_clone_node,
            session_node_qr_code,
            session_read_qr_code,
            session_node_traffic,
            session_remove_duplicate_nodes,
            session_connect,
            session_disconnect,
            session_logs,
            session_clear_logs,
            app_settings,
            set_app_settings,
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
        .run(handle_run_event);
}

#[cfg(test)]
mod tests {
    use super::{
        NodeTestStatus, ensure_subscription_mutation_ready, ensure_system_proxy_ready,
        node_test_result, parse_node_id, parse_subscription_id, platform_summary,
        traffic_counter_command_error, url_test_result,
    };
    use crate::node_latency::TcpLatencyError;
    use crate::platform_proxy::SystemProxyStartupStatus;
    use crate::traffic::TrafficCounterError;
    use crate::url_test::UrlTestError;
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
    fn url_test_results_reject_settings_and_classify_network_outcomes() {
        let id = Uuid::nil();

        assert_eq!(
            url_test_result(id, &Ok(35)).unwrap().status,
            NodeTestStatus::Success
        );
        assert_eq!(
            url_test_result(id, &Err(UrlTestError::TimedOut))
                .unwrap()
                .status,
            NodeTestStatus::Timeout
        );
        assert_eq!(
            url_test_result(id, &Err(UrlTestError::HttpStatus { status: 503 }))
                .unwrap()
                .status,
            NodeTestStatus::Failed
        );
        assert_eq!(
            url_test_result(
                id,
                &Err(UrlTestError::UnsupportedScheme {
                    scheme: "file".to_owned(),
                }),
            )
            .unwrap_err()
            .code,
            "invalid_url_test"
        );
    }

    #[test]
    fn traffic_counter_failures_have_a_stable_command_code() {
        assert_eq!(
            traffic_counter_command_error(&TrafficCounterError::CounterOverflow).code,
            "traffic_counter_overflow"
        );
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
