use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io;
use std::process::Command;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::system_proxy::{PacSetting, ProxyEndpoint, ProxySetting, SystemProxyState};

const NETWORK_SETUP: &str = "networksetup";
const ROUTE: &str = "route";

pub struct MacOsSystemProxyAdapter {
    service: NetworkServiceSource,
    executor: Arc<dyn CommandExecutor>,
}

/// Which macOS network service an adapter acts on.
enum NetworkServiceSource {
    /// A name chosen by the caller, used verbatim.
    Fixed(String),
    /// Whichever service owns the default route at the time of the operation.
    DefaultRoute,
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct MacOsSystemProxySnapshot {
    network_service: String,
    state: SystemProxyState,
}

impl MacOsSystemProxySnapshot {
    #[must_use]
    pub fn network_service(&self) -> &str {
        &self.network_service
    }

    #[must_use]
    pub const fn state(&self) -> &SystemProxyState {
        &self.state
    }
}

impl std::fmt::Debug for MacOsSystemProxySnapshot {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MacOsSystemProxySnapshot")
            .field("network_service", &self.network_service)
            .field("state", &"[REDACTED]")
            .finish()
    }
}

impl MacOsSystemProxyAdapter {
    /// Creates an adapter pinned to one macOS network service, such as `Wi-Fi`.
    ///
    /// # Errors
    ///
    /// Returns an error when the network service name is empty.
    pub fn new(network_service: &str) -> Result<Self, MacOsSystemProxyError> {
        Self::with_executor(network_service, Arc::new(ProcessCommandExecutor))
    }

    /// Creates an adapter that targets whichever network service currently owns
    /// the default route.
    ///
    /// The service is resolved on each operation rather than here, so a host
    /// that is offline at startup still works once it has a network, and moving
    /// from Wi-Fi to Ethernet mid-session configures the service actually in
    /// use. Recovery is unaffected: a snapshot records the service it captured
    /// and [`Self::apply_snapshot`] restores that one.
    #[must_use]
    pub fn for_default_route() -> Self {
        Self {
            service: NetworkServiceSource::DefaultRoute,
            executor: Arc::new(ProcessCommandExecutor),
        }
    }

    fn with_executor(
        network_service: &str,
        executor: Arc<dyn CommandExecutor>,
    ) -> Result<Self, MacOsSystemProxyError> {
        let network_service = network_service.trim();
        if network_service.is_empty() {
            return Err(MacOsSystemProxyError::EmptyNetworkService);
        }

        Ok(Self {
            service: NetworkServiceSource::Fixed(network_service.to_owned()),
            executor,
        })
    }

    /// Returns the network service this adapter should act on right now.
    fn resolve_service(&self) -> Result<String, MacOsSystemProxyError> {
        match &self.service {
            NetworkServiceSource::Fixed(name) => Ok(name.clone()),
            NetworkServiceSource::DefaultRoute => {
                let interface = self.default_route_interface()?;
                self.service_for_interface(&interface)
            }
        }
    }

    /// Reads the interface backing the default route, such as `en0`.
    fn default_route_interface(&self) -> Result<String, MacOsSystemProxyError> {
        let output = self
            .executor
            .execute(ROUTE, &["-n", "get", "default"])
            .map_err(|source| MacOsSystemProxyError::DefaultRouteCommandStartFailed { source })?;
        if output.status != Some(0) {
            return Err(MacOsSystemProxyError::DefaultRouteCommandFailed {
                status: output.status,
            });
        }
        parse_default_route_interface(&output.stdout)
            .ok_or(MacOsSystemProxyError::NoDefaultRouteInterface)
    }

    /// Maps a BSD interface name back to the network service that owns it.
    fn service_for_interface(&self, interface: &str) -> Result<String, MacOsSystemProxyError> {
        let output = self
            .executor
            .execute(NETWORK_SETUP, &["-listnetworkserviceorder"])
            .map_err(
                |source| MacOsSystemProxyError::NetworkServicesCommandStartFailed { source },
            )?;
        if output.status != Some(0) {
            return Err(MacOsSystemProxyError::NetworkServicesCommandFailed {
                status: output.status,
            });
        }
        parse_service_for_interface(&output.stdout, interface).ok_or_else(|| {
            MacOsSystemProxyError::NoServiceForInterface {
                interface: interface.to_owned(),
            }
        })
    }

    /// Reads HTTP, HTTPS, SOCKS, and PAC settings for the configured service.
    ///
    /// # Errors
    ///
    /// Returns a typed command or parse error without including command output.
    pub fn read(&self) -> Result<SystemProxyState, MacOsSystemProxyError> {
        self.read_service(&self.resolve_service()?)
    }

    fn read_service(&self, service: &str) -> Result<SystemProxyState, MacOsSystemProxyError> {
        let http = self.read_proxy(service, "-getwebproxy", MacOsProxyOperation::ReadHttp)?;
        let https = self.read_proxy(
            service,
            "-getsecurewebproxy",
            MacOsProxyOperation::ReadHttps,
        )?;
        let socks = self.read_proxy(
            service,
            "-getsocksfirewallproxy",
            MacOsProxyOperation::ReadSocks,
        )?;
        let pac_output = self.run(MacOsProxyOperation::ReadPac, &["-getautoproxyurl", service])?;
        let pac = parse_pac_setting(&pac_output, MacOsProxyOperation::ReadPac)?;

        Ok(SystemProxyState::new(http, https, socks, pac))
    }

    /// Captures the service in use right now and all its managed proxy values.
    ///
    /// The captured name is what recovery later restores, so it must be the
    /// service actually read here rather than whichever one is active later.
    ///
    /// # Errors
    ///
    /// Returns a typed command or parse error.
    pub fn read_snapshot(&self) -> Result<MacOsSystemProxySnapshot, MacOsSystemProxyError> {
        let service = self.resolve_service()?;
        let state = self.read_service(&service)?;
        Ok(MacOsSystemProxySnapshot {
            network_service: service,
            state,
        })
    }

    /// Applies HTTP, HTTPS, SOCKS, and PAC settings to the configured service.
    ///
    /// This operation is intentionally non-transactional. Callers that require
    /// rollback must snapshot with [`Self::read`] before applying a new state.
    ///
    /// # Errors
    ///
    /// Returns immediately when a `networksetup` command fails.
    pub fn apply(&self, state: &SystemProxyState) -> Result<(), MacOsSystemProxyError> {
        self.apply_service(&self.resolve_service()?, state)
    }

    fn apply_service(
        &self,
        service: &str,
        state: &SystemProxyState,
    ) -> Result<(), MacOsSystemProxyError> {
        self.apply_proxy(
            service,
            state.http(),
            "-setwebproxy",
            "-setwebproxystate",
            MacOsProxyOperation::WriteHttp,
        )?;
        self.apply_proxy(
            service,
            state.https(),
            "-setsecurewebproxy",
            "-setsecurewebproxystate",
            MacOsProxyOperation::WriteHttps,
        )?;
        self.apply_proxy(
            service,
            state.socks(),
            "-setsocksfirewallproxy",
            "-setsocksfirewallproxystate",
            MacOsProxyOperation::WriteSocks,
        )?;
        self.apply_pac(service, state.pac())
    }

    /// Restores the proxy values on the service captured in the snapshot.
    ///
    /// # Errors
    ///
    /// Returns immediately when a `networksetup` command fails.
    pub fn apply_snapshot(
        &self,
        snapshot: &MacOsSystemProxySnapshot,
    ) -> Result<(), MacOsSystemProxyError> {
        // Deliberately not resolved: recovery must undo the service that was
        // actually modified, even if the host has since moved to another one.
        self.apply_service(&snapshot.network_service, &snapshot.state)
    }

    fn read_proxy(
        &self,
        service: &str,
        command: &str,
        operation: MacOsProxyOperation,
    ) -> Result<ProxySetting, MacOsSystemProxyError> {
        let output = self.run(operation, &[command, service])?;
        parse_proxy_setting(&output, operation)
    }

    fn apply_proxy(
        &self,
        service: &str,
        setting: &ProxySetting,
        configure_command: &str,
        state_command: &str,
        operation: MacOsProxyOperation,
    ) -> Result<(), MacOsSystemProxyError> {
        if let Some(endpoint) = setting.endpoint() {
            let port = endpoint.port().to_string();
            self.run(
                operation,
                &[configure_command, service, endpoint.host(), &port, "off"],
            )?;
        }

        self.run(
            operation,
            &[state_command, service, on_or_off(setting.enabled())],
        )?;
        Ok(())
    }

    fn apply_pac(&self, service: &str, setting: &PacSetting) -> Result<(), MacOsSystemProxyError> {
        if let Some(url) = setting.url() {
            self.run(
                MacOsProxyOperation::WritePac,
                &["-setautoproxyurl", service, url],
            )?;
        }

        self.run(
            MacOsProxyOperation::WritePac,
            &["-setautoproxystate", service, on_or_off(setting.enabled())],
        )?;
        Ok(())
    }

    fn run(
        &self,
        operation: MacOsProxyOperation,
        arguments: &[&str],
    ) -> Result<String, MacOsSystemProxyError> {
        let output = self
            .executor
            .execute(NETWORK_SETUP, arguments)
            .map_err(|error| MacOsSystemProxyError::CommandStartFailed {
                operation,
                source: error,
            })?;
        if output.status != Some(0) {
            return Err(MacOsSystemProxyError::CommandFailed {
                operation,
                status: output.status,
            });
        }

        Ok(output.stdout)
    }
}

impl crate::system_proxy_recovery::SystemProxyControl for MacOsSystemProxyAdapter {
    type Error = MacOsSystemProxyError;
    type Snapshot = MacOsSystemProxySnapshot;

    fn capture(&self) -> Result<Self::Snapshot, Self::Error> {
        self.read_snapshot()
    }

    fn current(&self) -> Result<SystemProxyState, Self::Error> {
        Self::read(self)
    }

    fn apply(&self, state: &SystemProxyState) -> Result<(), Self::Error> {
        Self::apply(self, state)
    }

    fn restore(&self, snapshot: &Self::Snapshot) -> Result<(), Self::Error> {
        self.apply_snapshot(snapshot)
    }
}

fn on_or_off(enabled: bool) -> &'static str {
    if enabled { "on" } else { "off" }
}

fn parse_proxy_setting(
    output: &str,
    operation: MacOsProxyOperation,
) -> Result<ProxySetting, MacOsSystemProxyError> {
    let enabled = field(output, "Enabled")
        .and_then(parse_enabled)
        .ok_or(MacOsSystemProxyError::MalformedOutput { operation })?;
    let server = field(output, "Server").and_then(configured_value);
    let port = field(output, "Port")
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port != 0);
    let endpoint = match (server, port) {
        (Some(server), Some(port)) => Some(
            ProxyEndpoint::new(server, port)
                .map_err(|_| MacOsSystemProxyError::MalformedOutput { operation })?,
        ),
        (None, None) => None,
        (Some(_), None) | (None, Some(_)) => {
            return Err(MacOsSystemProxyError::MalformedOutput { operation });
        }
    };

    ProxySetting::new(enabled, endpoint)
        .map_err(|_| MacOsSystemProxyError::MalformedOutput { operation })
}

fn parse_pac_setting(
    output: &str,
    operation: MacOsProxyOperation,
) -> Result<PacSetting, MacOsSystemProxyError> {
    let enabled = field(output, "Enabled")
        .and_then(parse_enabled)
        .ok_or(MacOsSystemProxyError::MalformedOutput { operation })?;
    let url = field(output, "URL")
        .and_then(configured_value)
        .map(str::to_owned);

    PacSetting::new(enabled, url).map_err(|_| MacOsSystemProxyError::MalformedOutput { operation })
}

fn field<'a>(output: &'a str, expected_key: &str) -> Option<&'a str> {
    output.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == expected_key).then(|| value.trim())
    })
}

fn configured_value(value: &str) -> Option<&str> {
    (!value.is_empty() && value != "(null)").then_some(value)
}

/// Pulls the interface out of `route -n get default`, whose body is a list of
/// `  key: value` lines.
fn parse_default_route_interface(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == "interface").then(|| value.trim().to_owned())
    })
}

/// Finds the service owning `interface` in `networksetup
/// -listnetworkserviceorder`, whose entries look like:
///
/// ```text
/// (1) Wi-Fi
/// (Hardware Port: Wi-Fi, Device: en0)
/// ```
///
/// A leading `(*)` marks a disabled service; those are skipped, because a
/// disabled service cannot be the one carrying the default route.
fn parse_service_for_interface(output: &str, interface: &str) -> Option<String> {
    let mut pending: Option<String> = None;
    for line in output.lines() {
        let line = line.trim();
        if let Some(device) = parse_service_device(line) {
            if device == interface
                && let Some(service) = pending.take()
            {
                return Some(service);
            }
            pending = None;
        } else {
            pending = parse_service_heading(line);
        }
    }
    None
}

/// Reads `(1) Wi-Fi` as `Wi-Fi`, rejecting entries marked disabled by `(*)`.
fn parse_service_heading(line: &str) -> Option<String> {
    let rest = line.strip_prefix('(')?;
    let (marker, name) = rest.split_once(')')?;
    if marker == "*" || !marker.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// Reads the `Device:` field out of `(Hardware Port: Wi-Fi, Device: en0)`.
fn parse_service_device(line: &str) -> Option<&str> {
    let body = line.strip_prefix('(')?.strip_suffix(')')?;
    let device = body.split("Device:").nth(1)?.trim();
    (!device.is_empty()).then_some(device)
}

fn parse_enabled(value: &str) -> Option<bool> {
    match value {
        "Yes" => Some(true),
        "No" => Some(false),
        _ => None,
    }
}

trait CommandExecutor: Send + Sync {
    fn execute(&self, program: &str, arguments: &[&str]) -> io::Result<CommandOutput>;
}

struct ProcessCommandExecutor;

impl CommandExecutor for ProcessCommandExecutor {
    fn execute(&self, program: &str, arguments: &[&str]) -> io::Result<CommandOutput> {
        let output = Command::new(program).args(arguments).output()?;
        Ok(CommandOutput {
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        })
    }
}

struct CommandOutput {
    status: Option<i32>,
    stdout: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacOsProxyOperation {
    ReadHttp,
    ReadHttps,
    ReadSocks,
    ReadPac,
    WriteHttp,
    WriteHttps,
    WriteSocks,
    WritePac,
}

impl Display for MacOsProxyOperation {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::ReadHttp => "read HTTP proxy",
            Self::ReadHttps => "read HTTPS proxy",
            Self::ReadSocks => "read SOCKS proxy",
            Self::ReadPac => "read proxy auto-configuration",
            Self::WriteHttp => "write HTTP proxy",
            Self::WriteHttps => "write HTTPS proxy",
            Self::WriteSocks => "write SOCKS proxy",
            Self::WritePac => "write proxy auto-configuration",
        };
        formatter.write_str(name)
    }
}

#[derive(Debug)]
pub enum MacOsSystemProxyError {
    EmptyNetworkService,
    CommandStartFailed {
        operation: MacOsProxyOperation,
        source: io::Error,
    },
    CommandFailed {
        operation: MacOsProxyOperation,
        status: Option<i32>,
    },
    MalformedOutput {
        operation: MacOsProxyOperation,
    },
    DefaultRouteCommandStartFailed {
        source: io::Error,
    },
    DefaultRouteCommandFailed {
        status: Option<i32>,
    },
    NoDefaultRouteInterface,
    NetworkServicesCommandStartFailed {
        source: io::Error,
    },
    NetworkServicesCommandFailed {
        status: Option<i32>,
    },
    NoServiceForInterface {
        interface: String,
    },
}

impl Display for MacOsSystemProxyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyNetworkService => {
                formatter.write_str("macOS network service cannot be empty")
            }
            Self::CommandStartFailed { operation, source } => {
                write!(
                    formatter,
                    "failed to start networksetup to {operation}: {source}"
                )
            }
            Self::CommandFailed { operation, status } => {
                write!(
                    formatter,
                    "networksetup failed to {operation} with status {status:?}"
                )
            }
            Self::MalformedOutput { operation } => {
                write!(
                    formatter,
                    "networksetup returned malformed output while trying to {operation}"
                )
            }
            Self::DefaultRouteCommandStartFailed { source } => {
                write!(
                    formatter,
                    "failed to start route to read the default route: {source}"
                )
            }
            Self::DefaultRouteCommandFailed { status } => {
                write!(
                    formatter,
                    "route failed to read the default route with status {status:?}"
                )
            }
            Self::NoDefaultRouteInterface => formatter
                .write_str("the host has no default route to derive a network service from"),
            Self::NetworkServicesCommandStartFailed { source } => {
                write!(
                    formatter,
                    "failed to start networksetup to list network services: {source}"
                )
            }
            Self::NetworkServicesCommandFailed { status } => {
                write!(
                    formatter,
                    "networksetup failed to list network services with status {status:?}"
                )
            }
            Self::NoServiceForInterface { interface } => {
                write!(
                    formatter,
                    "no macOS network service uses interface {interface}"
                )
            }
        }
    }
}

impl Error for MacOsSystemProxyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CommandStartFailed { source, .. }
            | Self::DefaultRouteCommandStartFailed { source }
            | Self::NetworkServicesCommandStartFailed { source } => Some(source),
            Self::EmptyNetworkService
            | Self::CommandFailed { .. }
            | Self::MalformedOutput { .. }
            | Self::DefaultRouteCommandFailed { .. }
            | Self::NoDefaultRouteInterface
            | Self::NetworkServicesCommandFailed { .. }
            | Self::NoServiceForInterface { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::error::Error;
    use std::io;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::system_proxy::{PacSetting, ProxyEndpoint, ProxySetting, SystemProxyState};
    use crate::system_proxy_recovery::SystemProxyControl;

    #[test]
    fn adapter_satisfies_recovery_control_contract() {
        fn assert_control<T: SystemProxyControl<Snapshot = MacOsSystemProxySnapshot>>(
            _control: &T,
        ) {
        }

        let adapter = MacOsSystemProxyAdapter::new("Wi-Fi").unwrap();
        assert_control(&adapter);
    }

    #[test]
    fn recovery_snapshot_restores_the_original_network_service() {
        let executor = RecordingExecutor::returning([
            successful(""),
            successful(""),
            successful(""),
            successful(""),
        ]);
        let adapter = MacOsSystemProxyAdapter::with_executor("Ethernet", executor.clone()).unwrap();
        let snapshot = MacOsSystemProxySnapshot {
            network_service: "Wi-Fi".to_owned(),
            state: SystemProxyState::new(
                ProxySetting::disabled(),
                ProxySetting::disabled(),
                ProxySetting::disabled(),
                PacSetting::disabled(),
            ),
        };

        SystemProxyControl::restore(&adapter, &snapshot).unwrap();

        let commands = executor.calls();
        assert_eq!(commands.len(), 4);
        assert!(commands.iter().all(|(_, arguments)| {
            arguments.iter().any(|argument| argument == "Wi-Fi")
                && arguments.iter().all(|argument| argument != "Ethernet")
        }));
    }

    const SERVICE_ORDER: &str = "An asterisk (*) denotes that a network service is disabled.\n\
        (1) Wi-Fi\n\
        (Hardware Port: Wi-Fi, Device: en0)\n\
        \n\
        (2) Thunderbolt Ethernet\n\
        (Hardware Port: Thunderbolt Ethernet, Device: en5)\n\
        \n\
        (*) Bridge\n\
        (Hardware Port: Thunderbolt Bridge, Device: bridge0)\n";

    fn default_route(interface: &str) -> CommandOutput {
        successful(&format!(
            "   route to: default\ndestination: default\n    gateway: 192.168.1.1\n  interface: {interface}\n      flags: <UP,GATEWAY>\n"
        ))
    }

    /// The adapter resolves the service on every operation, so each test drives
    /// it with a route reply followed by the service list.
    fn detecting_adapter(
        outputs: impl IntoIterator<Item = CommandOutput>,
    ) -> MacOsSystemProxyAdapter {
        MacOsSystemProxyAdapter {
            service: NetworkServiceSource::DefaultRoute,
            executor: RecordingExecutor::returning(outputs),
        }
    }

    #[test]
    fn reads_the_service_that_owns_the_default_route() {
        let executor = RecordingExecutor::returning([
            default_route("en5"),
            successful(SERVICE_ORDER),
            successful("Enabled: No\nServer:\nPort: 0\n"),
            successful("Enabled: No\nServer:\nPort: 0\n"),
            successful("Enabled: No\nServer:\nPort: 0\n"),
            successful("Enabled: No\nURL: (null)\n"),
        ]);
        let adapter = MacOsSystemProxyAdapter {
            service: NetworkServiceSource::DefaultRoute,
            executor: executor.clone(),
        };

        adapter.read().unwrap();

        let calls = executor.calls();
        assert_eq!(calls[0].0, "route");
        assert_eq!(calls[1].1, vec!["-listnetworkserviceorder"]);
        // Every networksetup read targets the detected service, not Wi-Fi.
        assert!(
            calls[2..]
                .iter()
                .all(|(_, arguments)| arguments.contains(&"Thunderbolt Ethernet".to_owned()))
        );
    }

    #[test]
    fn a_snapshot_records_the_service_it_actually_captured() {
        let adapter = detecting_adapter([
            default_route("en5"),
            successful(SERVICE_ORDER),
            successful("Enabled: No\nServer:\nPort: 0\n"),
            successful("Enabled: No\nServer:\nPort: 0\n"),
            successful("Enabled: No\nServer:\nPort: 0\n"),
            successful("Enabled: No\nURL: (null)\n"),
        ]);

        let snapshot = adapter.read_snapshot().unwrap();

        assert_eq!(snapshot.network_service(), "Thunderbolt Ethernet");
    }

    #[test]
    fn restoring_a_snapshot_never_re_detects_the_service() {
        // Only four outputs: if restore tried to detect, it would run out.
        let executor = RecordingExecutor::returning([
            successful(""),
            successful(""),
            successful(""),
            successful(""),
        ]);
        let adapter = MacOsSystemProxyAdapter {
            service: NetworkServiceSource::DefaultRoute,
            executor: executor.clone(),
        };
        let snapshot = MacOsSystemProxySnapshot {
            network_service: "Wi-Fi".to_owned(),
            state: SystemProxyState::new(
                ProxySetting::disabled(),
                ProxySetting::disabled(),
                ProxySetting::disabled(),
                PacSetting::disabled(),
            ),
        };

        SystemProxyControl::restore(&adapter, &snapshot).unwrap();

        let calls = executor.calls();
        assert!(calls.iter().all(|(program, _)| program == NETWORK_SETUP));
        assert!(
            calls
                .iter()
                .all(|(_, arguments)| arguments.contains(&"Wi-Fi".to_owned()))
        );
    }

    #[test]
    fn an_explicit_service_is_used_without_reading_the_route() {
        let executor = RecordingExecutor::returning([
            successful("Enabled: No\nServer:\nPort: 0\n"),
            successful("Enabled: No\nServer:\nPort: 0\n"),
            successful("Enabled: No\nServer:\nPort: 0\n"),
            successful("Enabled: No\nURL: (null)\n"),
        ]);
        let adapter = MacOsSystemProxyAdapter::with_executor("Wi-Fi", executor.clone()).unwrap();

        adapter.read().unwrap();

        assert!(
            executor
                .calls()
                .iter()
                .all(|(program, _)| program == NETWORK_SETUP)
        );
    }

    #[test]
    fn reports_a_host_with_no_default_route() {
        let adapter = detecting_adapter([successful("   route to: default\n")]);

        assert!(matches!(
            adapter.read().unwrap_err(),
            MacOsSystemProxyError::NoDefaultRouteInterface
        ));
    }

    #[test]
    fn reports_an_interface_no_service_claims() {
        let adapter = detecting_adapter([default_route("utun7"), successful(SERVICE_ORDER)]);

        let error = adapter.read().unwrap_err();

        assert!(matches!(
            &error,
            MacOsSystemProxyError::NoServiceForInterface { interface } if interface == "utun7"
        ));
        assert_eq!(
            error.to_string(),
            "no macOS network service uses interface utun7"
        );
    }

    #[test]
    fn reports_a_failing_route_command() {
        let adapter = detecting_adapter([CommandOutput {
            status: Some(1),
            stdout: String::new(),
        }]);

        assert!(matches!(
            adapter.read().unwrap_err(),
            MacOsSystemProxyError::DefaultRouteCommandFailed { status: Some(1) }
        ));
    }

    #[test]
    fn skips_a_disabled_service_when_matching_an_interface() {
        // bridge0 belongs only to the disabled "(*) Bridge" entry.
        assert_eq!(parse_service_for_interface(SERVICE_ORDER, "bridge0"), None);
        assert_eq!(
            parse_service_for_interface(SERVICE_ORDER, "en0"),
            Some("Wi-Fi".to_owned())
        );
    }

    #[test]
    fn parses_the_interface_out_of_a_route_reply() {
        assert_eq!(
            parse_default_route_interface("  interface: en0\n  flags: <UP>\n"),
            Some("en0".to_owned())
        );
        assert_eq!(
            parse_default_route_interface("destination: default\n"),
            None
        );
    }

    struct RecordingExecutor {
        calls: Mutex<Vec<(String, Vec<String>)>>,
        outputs: Mutex<VecDeque<Result<CommandOutput, io::Error>>>,
    }

    impl RecordingExecutor {
        fn returning(outputs: impl IntoIterator<Item = CommandOutput>) -> Arc<Self> {
            Arc::new(Self {
                calls: Mutex::new(Vec::new()),
                outputs: Mutex::new(outputs.into_iter().map(Ok).collect()),
            })
        }

        fn failing(error: io::Error) -> Arc<Self> {
            Arc::new(Self {
                calls: Mutex::new(Vec::new()),
                outputs: Mutex::new([Err(error)].into_iter().collect()),
            })
        }

        fn calls(&self) -> Vec<(String, Vec<String>)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl CommandExecutor for RecordingExecutor {
        fn execute(&self, program: &str, arguments: &[&str]) -> io::Result<CommandOutput> {
            self.calls.lock().unwrap().push((
                program.to_owned(),
                arguments.iter().map(|value| (*value).to_owned()).collect(),
            ));
            self.outputs
                .lock()
                .unwrap()
                .pop_front()
                .expect("test supplied too few command outputs")
        }
    }

    fn successful(stdout: &str) -> CommandOutput {
        CommandOutput {
            status: Some(0),
            stdout: stdout.to_owned(),
        }
    }

    #[test]
    fn read_parses_all_proxy_kinds_and_preserves_disabled_values() {
        let executor = RecordingExecutor::returning([
            successful(
                "Enabled: Yes\nServer: 127.0.0.1\nPort: 10809\nAuthenticated Proxy Enabled: 0\n",
            ),
            successful(
                "Enabled: No\nServer: old.proxy.test\nPort: 8443\nAuthenticated Proxy Enabled: 0\n",
            ),
            successful(
                "Enabled: Yes\nServer: 127.0.0.1\nPort: 10808\nAuthenticated Proxy Enabled: 0\n",
            ),
            successful("URL: https://user:secret@example.test/proxy.pac\nEnabled: No\n"),
        ]);
        let adapter = MacOsSystemProxyAdapter::with_executor("Wi-Fi", executor.clone()).unwrap();

        let state = adapter.read().unwrap();

        assert!(state.http().enabled());
        assert_eq!(state.http().endpoint().unwrap().port(), 10809);
        assert!(!state.https().enabled());
        assert_eq!(state.https().endpoint().unwrap().host(), "old.proxy.test");
        assert!(state.socks().enabled());
        assert!(!state.pac().enabled());
        assert!(state.pac().url().is_some());
        assert!(!format!("{state:?}").contains("secret"));
        assert_eq!(
            executor.calls(),
            [
                (
                    "networksetup".to_owned(),
                    vec!["-getwebproxy".to_owned(), "Wi-Fi".to_owned()]
                ),
                (
                    "networksetup".to_owned(),
                    vec!["-getsecurewebproxy".to_owned(), "Wi-Fi".to_owned()]
                ),
                (
                    "networksetup".to_owned(),
                    vec!["-getsocksfirewallproxy".to_owned(), "Wi-Fi".to_owned()]
                ),
                (
                    "networksetup".to_owned(),
                    vec!["-getautoproxyurl".to_owned(), "Wi-Fi".to_owned()]
                ),
            ]
        );
    }

    #[test]
    fn read_rejects_enabled_proxy_without_a_valid_endpoint() {
        let executor = RecordingExecutor::returning([
            successful("Enabled: Yes\nServer: (null)\nPort: 0\n"),
            successful("Enabled: No\nServer: (null)\nPort: 0\n"),
            successful("Enabled: No\nServer: (null)\nPort: 0\n"),
            successful("URL: (null)\nEnabled: No\n"),
        ]);
        let adapter = MacOsSystemProxyAdapter::with_executor("Wi-Fi", executor).unwrap();

        assert!(matches!(
            adapter.read(),
            Err(MacOsSystemProxyError::MalformedOutput {
                operation: MacOsProxyOperation::ReadHttp,
            })
        ));
    }

    #[test]
    fn apply_emits_exact_commands_without_credentials() {
        let executor = RecordingExecutor::returning((0..8).map(|_| successful("")));
        let adapter =
            MacOsSystemProxyAdapter::with_executor("USB 10/100 LAN", executor.clone()).unwrap();
        let state = SystemProxyState::new(
            ProxySetting::new(true, Some(ProxyEndpoint::new("127.0.0.1", 10809).unwrap())).unwrap(),
            ProxySetting::new(
                false,
                Some(ProxyEndpoint::new("previous.proxy", 8443).unwrap()),
            )
            .unwrap(),
            ProxySetting::new(true, Some(ProxyEndpoint::new("127.0.0.1", 10808).unwrap())).unwrap(),
            PacSetting::new(true, Some("https://example.test/proxy.pac".to_owned())).unwrap(),
        );

        adapter.apply(&state).unwrap();

        assert_eq!(
            executor.calls(),
            [
                call(&[
                    "-setwebproxy",
                    "USB 10/100 LAN",
                    "127.0.0.1",
                    "10809",
                    "off"
                ]),
                call(&["-setwebproxystate", "USB 10/100 LAN", "on"]),
                call(&[
                    "-setsecurewebproxy",
                    "USB 10/100 LAN",
                    "previous.proxy",
                    "8443",
                    "off"
                ]),
                call(&["-setsecurewebproxystate", "USB 10/100 LAN", "off"]),
                call(&[
                    "-setsocksfirewallproxy",
                    "USB 10/100 LAN",
                    "127.0.0.1",
                    "10808",
                    "off"
                ]),
                call(&["-setsocksfirewallproxystate", "USB 10/100 LAN", "on"]),
                call(&[
                    "-setautoproxyurl",
                    "USB 10/100 LAN",
                    "https://example.test/proxy.pac"
                ]),
                call(&["-setautoproxystate", "USB 10/100 LAN", "on"]),
            ]
        );
    }

    #[test]
    fn apply_disables_unconfigured_proxy_kinds() {
        let executor = RecordingExecutor::returning((0..4).map(|_| successful("")));
        let adapter = MacOsSystemProxyAdapter::with_executor("Wi-Fi", executor.clone()).unwrap();
        let state = SystemProxyState::new(
            ProxySetting::disabled(),
            ProxySetting::disabled(),
            ProxySetting::disabled(),
            PacSetting::disabled(),
        );

        adapter.apply(&state).unwrap();

        assert_eq!(
            executor.calls(),
            [
                call(&["-setwebproxystate", "Wi-Fi", "off"]),
                call(&["-setsecurewebproxystate", "Wi-Fi", "off"]),
                call(&["-setsocksfirewallproxystate", "Wi-Fi", "off"]),
                call(&["-setautoproxystate", "Wi-Fi", "off"]),
            ]
        );
    }

    #[test]
    fn command_failure_reports_operation_without_leaking_output() {
        let executor = RecordingExecutor::returning([CommandOutput {
            status: Some(1),
            stdout: "https://user:secret@example.test/proxy.pac".to_owned(),
        }]);
        let adapter = MacOsSystemProxyAdapter::with_executor("Wi-Fi", executor).unwrap();

        let error = adapter.read().unwrap_err();

        assert!(matches!(
            &error,
            MacOsSystemProxyError::CommandFailed {
                operation: MacOsProxyOperation::ReadHttp,
                status: Some(1),
            }
        ));
        assert!(!format!("{error:?}").contains("secret"));
    }

    #[test]
    fn command_start_failure_preserves_its_io_source() {
        let executor = RecordingExecutor::failing(io::Error::new(
            io::ErrorKind::NotFound,
            "networksetup was not found",
        ));
        let adapter = MacOsSystemProxyAdapter::with_executor("Wi-Fi", executor).unwrap();

        let error = adapter.read().unwrap_err();

        assert!(matches!(
            &error,
            MacOsSystemProxyError::CommandStartFailed {
                operation: MacOsProxyOperation::ReadHttp,
                source,
            } if source.kind() == io::ErrorKind::NotFound
        ));
        assert_eq!(
            error.source().unwrap().to_string(),
            "networksetup was not found"
        );
    }

    #[test]
    fn adapter_requires_a_network_service() {
        assert!(matches!(
            MacOsSystemProxyAdapter::new("  "),
            Err(MacOsSystemProxyError::EmptyNetworkService)
        ));
    }

    fn call(arguments: &[&str]) -> (String, Vec<String>) {
        (
            "networksetup".to_owned(),
            arguments.iter().map(|value| (*value).to_owned()).collect(),
        )
    }
}
