use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io;
use std::process::Command;
use std::sync::Arc;

use crate::system_proxy::{PacSetting, ProxyEndpoint, ProxySetting, SystemProxyState};

const NETWORK_SETUP: &str = "networksetup";

pub struct MacOsSystemProxyAdapter {
    network_service: String,
    executor: Arc<dyn CommandExecutor>,
}

impl MacOsSystemProxyAdapter {
    /// Creates an adapter for one macOS network service, such as `Wi-Fi`.
    ///
    /// # Errors
    ///
    /// Returns an error when the network service name is empty.
    pub fn new(network_service: &str) -> Result<Self, MacOsSystemProxyError> {
        Self::with_executor(network_service, Arc::new(ProcessCommandExecutor))
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
            network_service: network_service.to_owned(),
            executor,
        })
    }

    /// Reads HTTP, HTTPS, SOCKS, and PAC settings for the configured service.
    ///
    /// # Errors
    ///
    /// Returns a typed command or parse error without including command output.
    pub fn read(&self) -> Result<SystemProxyState, MacOsSystemProxyError> {
        let http = self.read_proxy("-getwebproxy", MacOsProxyOperation::ReadHttp)?;
        let https = self.read_proxy("-getsecurewebproxy", MacOsProxyOperation::ReadHttps)?;
        let socks = self.read_proxy("-getsocksfirewallproxy", MacOsProxyOperation::ReadSocks)?;
        let pac_output = self.run(
            MacOsProxyOperation::ReadPac,
            &["-getautoproxyurl", &self.network_service],
        )?;
        let pac = parse_pac_setting(&pac_output, MacOsProxyOperation::ReadPac)?;

        Ok(SystemProxyState::new(http, https, socks, pac))
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
        self.apply_proxy(
            state.http(),
            "-setwebproxy",
            "-setwebproxystate",
            MacOsProxyOperation::WriteHttp,
        )?;
        self.apply_proxy(
            state.https(),
            "-setsecurewebproxy",
            "-setsecurewebproxystate",
            MacOsProxyOperation::WriteHttps,
        )?;
        self.apply_proxy(
            state.socks(),
            "-setsocksfirewallproxy",
            "-setsocksfirewallproxystate",
            MacOsProxyOperation::WriteSocks,
        )?;
        self.apply_pac(state.pac())
    }

    fn read_proxy(
        &self,
        command: &str,
        operation: MacOsProxyOperation,
    ) -> Result<ProxySetting, MacOsSystemProxyError> {
        let output = self.run(operation, &[command, &self.network_service])?;
        parse_proxy_setting(&output, operation)
    }

    fn apply_proxy(
        &self,
        setting: &ProxySetting,
        configure_command: &str,
        state_command: &str,
        operation: MacOsProxyOperation,
    ) -> Result<(), MacOsSystemProxyError> {
        if let Some(endpoint) = setting.endpoint() {
            let port = endpoint.port().to_string();
            self.run(
                operation,
                &[
                    configure_command,
                    &self.network_service,
                    endpoint.host(),
                    &port,
                    "off",
                ],
            )?;
        }

        self.run(
            operation,
            &[
                state_command,
                &self.network_service,
                on_or_off(setting.enabled()),
            ],
        )?;
        Ok(())
    }

    fn apply_pac(&self, setting: &PacSetting) -> Result<(), MacOsSystemProxyError> {
        if let Some(url) = setting.url() {
            self.run(
                MacOsProxyOperation::WritePac,
                &["-setautoproxyurl", &self.network_service, url],
            )?;
        }

        self.run(
            MacOsProxyOperation::WritePac,
            &[
                "-setautoproxystate",
                &self.network_service,
                on_or_off(setting.enabled()),
            ],
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
        }
    }
}

impl Error for MacOsSystemProxyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CommandStartFailed { source, .. } => Some(source),
            Self::EmptyNetworkService
            | Self::CommandFailed { .. }
            | Self::MalformedOutput { .. } => None,
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
