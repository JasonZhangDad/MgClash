use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::system_proxy::{PacSetting, ProxyEndpoint, ProxySetting, SystemProxyState};

const PROXY_TYPE_DIRECT: u32 = 1;
const PROXY_TYPE_PROXY: u32 = 2;
const PROXY_TYPE_AUTO_PROXY_URL: u32 = 4;

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct WindowsSystemProxySnapshot {
    flags: u32,
    proxy_server: Option<String>,
    proxy_bypass: Option<String>,
    auto_config_url: Option<String>,
}

impl WindowsSystemProxySnapshot {
    fn from_json(json: &str) -> Result<Self, WindowsSystemProxyError> {
        serde_json::from_str(json)
            .map_err(|source| WindowsSystemProxyError::InvalidSnapshotJson { source })
    }

    fn to_json(&self) -> Result<String, WindowsSystemProxyError> {
        serde_json::to_string(self)
            .map_err(|source| WindowsSystemProxyError::InvalidSnapshotJson { source })
    }

    /// Converts raw `WinINet` settings into the shared proxy state model.
    ///
    /// # Errors
    ///
    /// Returns an error when a configured endpoint or active PAC setting is
    /// malformed.
    pub fn to_system_proxy_state(&self) -> Result<SystemProxyState, WindowsSystemProxyError> {
        let endpoints = parse_proxy_server(self.proxy_server.as_deref())?;
        let proxy_enabled = self.flags & PROXY_TYPE_PROXY != 0;
        let http = proxy_setting(proxy_enabled, endpoints.http)?;
        let https = proxy_setting(proxy_enabled, endpoints.https)?;
        let socks = proxy_setting(proxy_enabled, endpoints.socks)?;
        let pac = PacSetting::new(
            self.flags & PROXY_TYPE_AUTO_PROXY_URL != 0,
            self.auto_config_url.clone(),
        )
        .map_err(|_| WindowsSystemProxyError::MalformedSnapshot)?;

        Ok(SystemProxyState::new(http, https, socks, pac))
    }

    fn with_state(&self, state: &SystemProxyState) -> Result<Self, WindowsSystemProxyError> {
        let settings = [
            ("http", state.http()),
            ("https", state.https()),
            ("socks", state.socks()),
        ];
        let proxy_enabled = settings.iter().any(|(_, setting)| setting.enabled());
        if proxy_enabled
            && settings
                .iter()
                .any(|(_, setting)| !setting.enabled() && setting.endpoint().is_some())
        {
            return Err(WindowsSystemProxyError::UnrepresentableProxyState);
        }

        let proxy_server = settings
            .iter()
            .filter_map(|(protocol, setting)| {
                setting
                    .endpoint()
                    .map(|endpoint| format_windows_endpoint(protocol, endpoint))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let proxy_server = (!proxy_server.is_empty()).then(|| proxy_server.join(";"));
        let mut flags = PROXY_TYPE_DIRECT;
        if proxy_enabled {
            flags |= PROXY_TYPE_PROXY;
        }
        if state.pac().enabled() {
            flags |= PROXY_TYPE_AUTO_PROXY_URL;
        }

        Ok(Self {
            flags,
            proxy_server,
            proxy_bypass: self.proxy_bypass.clone(),
            auto_config_url: state.pac().url().map(str::to_owned),
        })
    }

    #[must_use]
    pub const fn flags(&self) -> u32 {
        self.flags
    }

    #[must_use]
    pub fn proxy_bypass(&self) -> Option<&str> {
        self.proxy_bypass.as_deref()
    }
}

impl Debug for WindowsSystemProxySnapshot {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WindowsSystemProxySnapshot")
            .field("flags", &self.flags)
            .field(
                "proxy_server",
                &self.proxy_server.as_ref().map(|_| "configured"),
            )
            .field("proxy_bypass", &self.proxy_bypass)
            .field(
                "auto_config_url",
                &self.auto_config_url.as_ref().map(|_| "configured"),
            )
            .finish()
    }
}

pub struct WindowsSystemProxyAdapter {
    executor: Arc<dyn CommandExecutor>,
}

impl WindowsSystemProxyAdapter {
    #[must_use]
    pub fn new() -> Self {
        Self::with_executor(Arc::new(ProcessCommandExecutor))
    }

    fn with_executor(executor: Arc<dyn CommandExecutor>) -> Self {
        Self { executor }
    }

    /// Reads the exact `WinINet` LAN proxy values used for later restoration.
    ///
    /// # Errors
    ///
    /// Returns a typed process or JSON error without including process output.
    pub fn read_snapshot(&self) -> Result<WindowsSystemProxySnapshot, WindowsSystemProxyError> {
        let json = self.run(WindowsProxyCommand::Read, "")?;
        WindowsSystemProxySnapshot::from_json(&json)
    }

    /// Restores an exact `WinINet` LAN proxy snapshot and refreshes `WinINet`.
    ///
    /// # Errors
    ///
    /// Returns a typed serialization or process error.
    pub fn apply_snapshot(
        &self,
        snapshot: &WindowsSystemProxySnapshot,
    ) -> Result<(), WindowsSystemProxyError> {
        let json = snapshot.to_json()?;
        self.run(WindowsProxyCommand::Write, &json)?;
        Ok(())
    }

    /// Reads the current Windows proxy settings into the shared state model.
    ///
    /// # Errors
    ///
    /// Returns a typed snapshot, process, or mapping error.
    pub fn read(&self) -> Result<SystemProxyState, WindowsSystemProxyError> {
        self.read_snapshot()?.to_system_proxy_state()
    }

    /// Applies shared proxy state while preserving the current bypass list.
    ///
    /// # Errors
    ///
    /// Returns an error for process failures or state Windows cannot represent.
    pub fn apply(&self, state: &SystemProxyState) -> Result<(), WindowsSystemProxyError> {
        let current = self.read_snapshot()?;
        let updated = current.with_state(state)?;
        self.apply_snapshot(&updated)
    }

    fn run(
        &self,
        command: WindowsProxyCommand,
        input: &str,
    ) -> Result<String, WindowsSystemProxyError> {
        let operation = command.operation();
        let output = self
            .executor
            .execute(command, input)
            .map_err(|source| WindowsSystemProxyError::PowerShellIo { operation, source })?;
        if output.status != Some(0) {
            return Err(WindowsSystemProxyError::CommandFailed {
                operation,
                status: output.status,
            });
        }
        Ok(output.stdout)
    }
}

impl Default for WindowsSystemProxyAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
struct ProxyEndpoints {
    http: Option<ProxyEndpoint>,
    https: Option<ProxyEndpoint>,
    socks: Option<ProxyEndpoint>,
}

fn parse_proxy_server(value: Option<&str>) -> Result<ProxyEndpoints, WindowsSystemProxyError> {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return Ok(ProxyEndpoints::default());
    };
    if !value.contains('=') {
        let endpoint = parse_windows_endpoint(value)?;
        return Ok(ProxyEndpoints {
            http: Some(endpoint.clone()),
            https: Some(endpoint.clone()),
            socks: Some(endpoint),
        });
    }

    let mut endpoints = ProxyEndpoints::default();
    for entry in value.split(';').filter(|entry| !entry.trim().is_empty()) {
        let (protocol, endpoint) = entry
            .split_once('=')
            .ok_or(WindowsSystemProxyError::MalformedSnapshot)?;
        let target = match protocol.trim().to_ascii_lowercase().as_str() {
            "http" => &mut endpoints.http,
            "https" => &mut endpoints.https,
            "socks" => &mut endpoints.socks,
            _ => continue,
        };
        if target.is_some() {
            return Err(WindowsSystemProxyError::MalformedSnapshot);
        }
        *target = Some(parse_windows_endpoint(endpoint)?);
    }
    Ok(endpoints)
}

fn parse_windows_endpoint(value: &str) -> Result<ProxyEndpoint, WindowsSystemProxyError> {
    let value = value.trim();
    if value.contains('@') || value.contains('/') || value.contains(';') {
        return Err(WindowsSystemProxyError::MalformedSnapshot);
    }
    let (host, port) = value
        .rsplit_once(':')
        .ok_or(WindowsSystemProxyError::MalformedSnapshot)?;
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    let port = port
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or(WindowsSystemProxyError::MalformedSnapshot)?;
    ProxyEndpoint::new(host, port).map_err(|_| WindowsSystemProxyError::MalformedSnapshot)
}

fn proxy_setting(
    proxy_enabled: bool,
    endpoint: Option<ProxyEndpoint>,
) -> Result<ProxySetting, WindowsSystemProxyError> {
    ProxySetting::new(proxy_enabled && endpoint.is_some(), endpoint)
        .map_err(|_| WindowsSystemProxyError::MalformedSnapshot)
}

fn format_windows_endpoint(
    protocol: &str,
    endpoint: &ProxyEndpoint,
) -> Result<String, WindowsSystemProxyError> {
    let host = endpoint.host();
    if host.contains('@')
        || host.contains('/')
        || host.contains(';')
        || host.contains('=')
        || host.chars().any(char::is_whitespace)
    {
        return Err(WindowsSystemProxyError::UnrepresentableProxyState);
    }
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    Ok(format!("{protocol}={host}:{}", endpoint.port()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowsProxyCommand {
    Read,
    Write,
}

impl WindowsProxyCommand {
    const fn operation(self) -> WindowsProxyOperation {
        match self {
            Self::Read => WindowsProxyOperation::Read,
            Self::Write => WindowsProxyOperation::Write,
        }
    }
}

trait CommandExecutor: Send + Sync {
    fn execute(&self, command: WindowsProxyCommand, input: &str) -> io::Result<CommandOutput>;
}

struct ProcessCommandExecutor;

impl CommandExecutor for ProcessCommandExecutor {
    fn execute(&self, command: WindowsProxyCommand, input: &str) -> io::Result<CommandOutput> {
        let script = match command {
            WindowsProxyCommand::Read => format!("{WININET_TYPE_DEFINITION}\n{READ_SCRIPT}"),
            WindowsProxyCommand::Write => format!("{WININET_TYPE_DEFINITION}\n{WRITE_SCRIPT}"),
        };
        let mut child = Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
            ])
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let write_result = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("PowerShell stdin was unavailable"))?
            .write_all(input.as_bytes());
        if let Err(error) = write_result {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        let output = child.wait_with_output()?;
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
pub enum WindowsProxyOperation {
    Read,
    Write,
}

impl Display for WindowsProxyOperation {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => formatter.write_str("read Windows System Proxy"),
            Self::Write => formatter.write_str("write Windows System Proxy"),
        }
    }
}

#[derive(Debug)]
pub enum WindowsSystemProxyError {
    InvalidSnapshotJson {
        source: serde_json::Error,
    },
    MalformedSnapshot,
    UnrepresentableProxyState,
    PowerShellIo {
        operation: WindowsProxyOperation,
        source: io::Error,
    },
    CommandFailed {
        operation: WindowsProxyOperation,
        status: Option<i32>,
    },
}

impl Display for WindowsSystemProxyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSnapshotJson { source } => {
                write!(
                    formatter,
                    "invalid Windows System Proxy snapshot JSON: {source}"
                )
            }
            Self::MalformedSnapshot => {
                formatter.write_str("Windows System Proxy snapshot is malformed")
            }
            Self::UnrepresentableProxyState => {
                formatter.write_str("Windows cannot represent the requested System Proxy state")
            }
            Self::PowerShellIo { operation, source } => {
                write!(formatter, "PowerShell failed to {operation}: {source}")
            }
            Self::CommandFailed { operation, status } => {
                write!(
                    formatter,
                    "PowerShell failed to {operation} with status {status:?}"
                )
            }
        }
    }
}

impl Error for WindowsSystemProxyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidSnapshotJson { source } => Some(source),
            Self::PowerShellIo { source, .. } => Some(source),
            Self::MalformedSnapshot
            | Self::UnrepresentableProxyState
            | Self::CommandFailed { .. } => None,
        }
    }
}

const WININET_TYPE_DEFINITION: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;

namespace MgClash {
    public sealed class ProxySnapshot {
        public uint Flags;
        public string ProxyServer;
        public string ProxyBypass;
        public string AutoConfigUrl;
    }

    public static class WinInetProxy {
        const int INTERNET_OPTION_REFRESH = 37;
        const int INTERNET_OPTION_PER_CONNECTION_OPTION = 75;
        const int INTERNET_OPTION_PROXY_SETTINGS_CHANGED = 95;
        const int INTERNET_PER_CONN_FLAGS = 1;
        const int INTERNET_PER_CONN_PROXY_SERVER = 2;
        const int INTERNET_PER_CONN_PROXY_BYPASS = 3;
        const int INTERNET_PER_CONN_AUTOCONFIG_URL = 4;
        const int INTERNET_PER_CONN_FLAGS_UI = 10;

        [StructLayout(LayoutKind.Explicit)]
        struct OptionValue {
            [FieldOffset(0)] public uint Dword;
            [FieldOffset(0)] public IntPtr String;
        }

        [StructLayout(LayoutKind.Sequential)]
        struct PerConnectionOption {
            public int Option;
            public OptionValue Value;
        }

        [StructLayout(LayoutKind.Sequential)]
        struct PerConnectionOptionList {
            public int Size;
            public IntPtr Connection;
            public int OptionCount;
            public int OptionError;
            public IntPtr Options;
        }

        [DllImport("wininet.dll", EntryPoint = "InternetQueryOptionA", SetLastError = true)]
        static extern bool InternetQueryOption(
            IntPtr internet, int option, ref PerConnectionOptionList buffer, ref int length);

        [DllImport("wininet.dll", EntryPoint = "InternetSetOptionA", SetLastError = true)]
        static extern bool InternetSetOption(
            IntPtr internet, int option, IntPtr buffer, int length);

        [DllImport("kernel32.dll")]
        static extern IntPtr GlobalFree(IntPtr memory);

        public static ProxySnapshot Read() {
            try {
                return ReadWithFlagsOption(INTERNET_PER_CONN_FLAGS_UI);
            } catch (Win32Exception) {
                return ReadWithFlagsOption(INTERNET_PER_CONN_FLAGS);
            }
        }

        static ProxySnapshot ReadWithFlagsOption(int flagsOption) {
            PerConnectionOption[] options = new PerConnectionOption[4];
            options[0].Option = flagsOption;
            options[1].Option = INTERNET_PER_CONN_PROXY_SERVER;
            options[2].Option = INTERNET_PER_CONN_PROXY_BYPASS;
            options[3].Option = INTERNET_PER_CONN_AUTOCONFIG_URL;
            int optionSize = Marshal.SizeOf(typeof(PerConnectionOption));
            IntPtr optionBuffer = Marshal.AllocHGlobal(optionSize * options.Length);
            try {
                for (int index = 0; index < options.Length; index++) {
                    Marshal.StructureToPtr(options[index], IntPtr.Add(optionBuffer, index * optionSize), false);
                }
                PerConnectionOptionList list = new PerConnectionOptionList {
                    Size = Marshal.SizeOf(typeof(PerConnectionOptionList)),
                    Connection = IntPtr.Zero,
                    OptionCount = options.Length,
                    OptionError = 0,
                    Options = optionBuffer,
                };
                int listSize = list.Size;
                if (!InternetQueryOption(IntPtr.Zero, INTERNET_OPTION_PER_CONNECTION_OPTION, ref list, ref listSize)) {
                    throw new Win32Exception(Marshal.GetLastWin32Error());
                }
                for (int index = 0; index < options.Length; index++) {
                    options[index] = (PerConnectionOption)Marshal.PtrToStructure(
                        IntPtr.Add(optionBuffer, index * optionSize), typeof(PerConnectionOption));
                }
                return new ProxySnapshot {
                    Flags = options[0].Value.Dword,
                    ProxyServer = ReadString(options[1].Value.String),
                    ProxyBypass = ReadString(options[2].Value.String),
                    AutoConfigUrl = ReadString(options[3].Value.String),
                };
            } finally {
                for (int index = 1; index < options.Length; index++) {
                    options[index] = (PerConnectionOption)Marshal.PtrToStructure(
                        IntPtr.Add(optionBuffer, index * optionSize), typeof(PerConnectionOption));
                    if (options[index].Value.String != IntPtr.Zero) {
                        GlobalFree(options[index].Value.String);
                    }
                }
                Marshal.FreeHGlobal(optionBuffer);
            }
        }

        public static void Write(uint flags, string proxyServer, string proxyBypass, string autoConfigUrl) {
            string[] strings = new string[] { proxyServer, proxyBypass, autoConfigUrl };
            IntPtr[] allocated = new IntPtr[strings.Length];
            PerConnectionOption[] options = new PerConnectionOption[4];
            options[0].Option = INTERNET_PER_CONN_FLAGS;
            options[0].Value.Dword = flags;
            int optionSize = Marshal.SizeOf(typeof(PerConnectionOption));
            IntPtr optionBuffer = Marshal.AllocHGlobal(optionSize * options.Length);
            try {
                for (int index = 0; index < strings.Length; index++) {
                    if (strings[index] != null) {
                        allocated[index] = Marshal.StringToHGlobalAnsi(strings[index]);
                    }
                    options[index + 1].Option = index + INTERNET_PER_CONN_PROXY_SERVER;
                    options[index + 1].Value.String = allocated[index];
                }
                for (int index = 0; index < options.Length; index++) {
                    Marshal.StructureToPtr(options[index], IntPtr.Add(optionBuffer, index * optionSize), false);
                }
                PerConnectionOptionList list = new PerConnectionOptionList {
                    Size = Marshal.SizeOf(typeof(PerConnectionOptionList)),
                    Connection = IntPtr.Zero,
                    OptionCount = options.Length,
                    OptionError = 0,
                    Options = optionBuffer,
                };
                IntPtr listBuffer = Marshal.AllocHGlobal(list.Size);
                try {
                    Marshal.StructureToPtr(list, listBuffer, false);
                    if (!InternetSetOption(
                        IntPtr.Zero,
                        INTERNET_OPTION_PER_CONNECTION_OPTION,
                        listBuffer,
                        list.Size)) {
                        throw new Win32Exception(Marshal.GetLastWin32Error());
                    }
                } finally {
                    Marshal.FreeHGlobal(listBuffer);
                }
            } finally {
                Marshal.FreeHGlobal(optionBuffer);
                foreach (IntPtr value in allocated) {
                    if (value != IntPtr.Zero) Marshal.FreeHGlobal(value);
                }
            }
            Notify(INTERNET_OPTION_PROXY_SETTINGS_CHANGED);
            Notify(INTERNET_OPTION_REFRESH);
        }

        static void Notify(int option) {
            if (!InternetSetOption(IntPtr.Zero, option, IntPtr.Zero, 0)) {
                throw new Win32Exception(Marshal.GetLastWin32Error());
            }
        }

        static string ReadString(IntPtr value) {
            return value == IntPtr.Zero ? null : Marshal.PtrToStringAnsi(value);
        }
    }
}
'@
"#;

const READ_SCRIPT: &str = r"
$snapshot = [MgClash.WinInetProxy]::Read()
[ordered]@{
    flags = [uint32]$snapshot.Flags
    proxy_server = $snapshot.ProxyServer
    proxy_bypass = $snapshot.ProxyBypass
    auto_config_url = $snapshot.AutoConfigUrl
} | ConvertTo-Json -Compress
";

const WRITE_SCRIPT: &str = r"
$snapshot = [Console]::In.ReadToEnd() | ConvertFrom-Json
$proxyServer = if ($null -eq $snapshot.proxy_server) { $null } else { [string]$snapshot.proxy_server }
$proxyBypass = if ($null -eq $snapshot.proxy_bypass) { $null } else { [string]$snapshot.proxy_bypass }
$autoConfigUrl = if ($null -eq $snapshot.auto_config_url) { $null } else { [string]$snapshot.auto_config_url }
[MgClash.WinInetProxy]::Write(
    [uint32]$snapshot.flags,
    $proxyServer,
    $proxyBypass,
    $autoConfigUrl)
";

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::io;
    use std::sync::{Arc, Mutex};

    use serde_json::Value;

    use super::*;
    use crate::system_proxy::{PacSetting, ProxyEndpoint, ProxySetting, SystemProxyState};

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct RecordedCall {
        command: WindowsProxyCommand,
        input: String,
    }

    struct RecordingExecutor {
        calls: Mutex<Vec<RecordedCall>>,
        outputs: Mutex<VecDeque<Result<CommandOutput, io::Error>>>,
    }

    impl RecordingExecutor {
        fn returning(outputs: impl IntoIterator<Item = CommandOutput>) -> Arc<Self> {
            Arc::new(Self {
                calls: Mutex::new(Vec::new()),
                outputs: Mutex::new(outputs.into_iter().map(Ok).collect()),
            })
        }

        fn calls(&self) -> Vec<RecordedCall> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl CommandExecutor for RecordingExecutor {
        fn execute(&self, command: WindowsProxyCommand, input: &str) -> io::Result<CommandOutput> {
            self.calls.lock().unwrap().push(RecordedCall {
                command,
                input: input.to_owned(),
            });
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
    fn reads_protocol_specific_proxy_and_pac_state() {
        let executor = RecordingExecutor::returning([successful(
            r#"{"flags":7,"proxy_server":"http=127.0.0.1:10809;https=127.0.0.1:10809;socks=127.0.0.1:10808","proxy_bypass":"<local>","auto_config_url":"https://user:secret@example.test/proxy.pac"}"#,
        )]);
        let adapter = WindowsSystemProxyAdapter::with_executor(executor.clone());

        let state = adapter.read().unwrap();

        assert!(state.http().enabled());
        assert_eq!(state.http().endpoint().unwrap().port(), 10809);
        assert!(state.https().enabled());
        assert!(state.socks().enabled());
        assert_eq!(state.socks().endpoint().unwrap().port(), 10808);
        assert!(state.pac().enabled());
        assert!(!format!("{state:?}").contains("secret"));
        assert_eq!(
            executor.calls(),
            [RecordedCall {
                command: WindowsProxyCommand::Read,
                input: String::new(),
            }]
        );
    }

    #[test]
    fn simple_proxy_endpoint_applies_to_all_protocols() {
        let snapshot = WindowsSystemProxySnapshot::from_json(
            r#"{"flags":3,"proxy_server":"[::1]:1080","proxy_bypass":null,"auto_config_url":null}"#,
        )
        .unwrap();

        let state = snapshot.to_system_proxy_state().unwrap();

        for setting in [state.http(), state.https(), state.socks()] {
            assert!(setting.enabled());
            assert_eq!(setting.endpoint().unwrap().host(), "::1");
            assert_eq!(setting.endpoint().unwrap().port(), 1080);
        }
    }

    #[test]
    fn disabled_snapshot_preserves_configured_values() {
        let snapshot = WindowsSystemProxySnapshot::from_json(
            r#"{"flags":1,"proxy_server":"http=old.proxy:8080","proxy_bypass":"localhost","auto_config_url":"https://example.test/proxy.pac"}"#,
        )
        .unwrap();

        let state = snapshot.to_system_proxy_state().unwrap();

        assert!(!state.http().enabled());
        assert_eq!(state.http().endpoint().unwrap().host(), "old.proxy");
        assert!(!state.https().enabled());
        assert!(state.https().endpoint().is_none());
        assert!(!state.pac().enabled());
        assert!(state.pac().url().is_some());
    }

    #[test]
    fn malformed_proxy_strings_are_rejected_without_echoing_them() {
        let snapshot = WindowsSystemProxySnapshot::from_json(
            r#"{"flags":3,"proxy_server":"http=user:secret@proxy","proxy_bypass":null,"auto_config_url":null}"#,
        )
        .unwrap();

        let error = snapshot.to_system_proxy_state().unwrap_err();

        assert!(matches!(error, WindowsSystemProxyError::MalformedSnapshot));
        assert!(!format!("{error:?}").contains("secret"));
    }

    #[test]
    fn snapshot_debug_redacts_proxy_and_pac_values() {
        let snapshot = WindowsSystemProxySnapshot::from_json(
            r#"{"flags":7,"proxy_server":"user:proxy-secret@example.test:8080","proxy_bypass":"<local>","auto_config_url":"https://pac-secret@example.test/proxy.pac"}"#,
        )
        .unwrap();

        let debug = format!("{snapshot:?}");

        assert!(debug.contains("configured"));
        assert!(debug.contains("<local>"));
        assert!(!debug.contains("proxy-secret"));
        assert!(!debug.contains("pac-secret"));
    }

    #[test]
    fn apply_preserves_bypass_and_writes_state_through_stdin() {
        let executor = RecordingExecutor::returning([
            successful(
                r#"{"flags":13,"proxy_server":"old.proxy:8080","proxy_bypass":"localhost;<local>","auto_config_url":"https://old.example/proxy.pac"}"#,
            ),
            successful(""),
        ]);
        let adapter = WindowsSystemProxyAdapter::with_executor(executor.clone());
        let state = SystemProxyState::new(
            ProxySetting::new(true, Some(ProxyEndpoint::new("127.0.0.1", 10809).unwrap())).unwrap(),
            ProxySetting::new(true, Some(ProxyEndpoint::new("127.0.0.1", 10809).unwrap())).unwrap(),
            ProxySetting::disabled(),
            PacSetting::disabled(),
        );

        adapter.apply(&state).unwrap();

        let calls = executor.calls();
        assert_eq!(calls[0].command, WindowsProxyCommand::Read);
        assert_eq!(calls[1].command, WindowsProxyCommand::Write);
        let written: Value = serde_json::from_str(&calls[1].input).unwrap();
        assert_eq!(written["flags"], 3);
        assert_eq!(
            written["proxy_server"],
            "http=127.0.0.1:10809;https=127.0.0.1:10809"
        );
        assert_eq!(written["proxy_bypass"], "localhost;<local>");
        assert!(written["auto_config_url"].is_null());
    }

    #[test]
    fn apply_rejects_disabled_configured_proxy_mixed_with_enabled_proxy() {
        let executor = RecordingExecutor::returning([successful(
            r#"{"flags":1,"proxy_server":null,"proxy_bypass":null,"auto_config_url":null}"#,
        )]);
        let adapter = WindowsSystemProxyAdapter::with_executor(executor.clone());
        let state = SystemProxyState::new(
            ProxySetting::new(true, Some(ProxyEndpoint::new("127.0.0.1", 10809).unwrap())).unwrap(),
            ProxySetting::new(false, Some(ProxyEndpoint::new("old.proxy", 8080).unwrap())).unwrap(),
            ProxySetting::disabled(),
            PacSetting::disabled(),
        );

        assert!(matches!(
            adapter.apply(&state),
            Err(WindowsSystemProxyError::UnrepresentableProxyState)
        ));
        assert_eq!(executor.calls().len(), 1);
    }

    #[test]
    fn command_failure_does_not_expose_powershell_output() {
        let executor = RecordingExecutor::returning([CommandOutput {
            status: Some(1),
            stdout: "https://user:secret@example.test/proxy.pac".to_owned(),
        }]);
        let adapter = WindowsSystemProxyAdapter::with_executor(executor);

        let error = adapter.read().unwrap_err();

        assert!(matches!(
            error,
            WindowsSystemProxyError::CommandFailed {
                operation: WindowsProxyOperation::Read,
                status: Some(1),
            }
        ));
        assert!(!format!("{error:?}").contains("secret"));
    }
}
