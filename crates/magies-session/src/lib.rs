//! Desktop proxy session orchestration for `MgClash`.

mod network_recovery;
mod network_watcher;

pub use network_recovery::{
    DEFAULT_DEBOUNCE, MAX_RECOVERY_ATTEMPTS, NetworkEvent, NetworkRecoveryPolicy, RecoveryError,
    RecoveryOutcome, SessionHealthProbe, TcpHealthProbe,
};
pub use network_watcher::NetworkWatcher;

use std::error::Error;
use std::net::SocketAddr;
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};
use std::time::Duration;

use magies_core_runtime::{
    AtomicRuntimeConfig, CoreHealthError, CoreOutput, CoreRuntime, CoreRuntimeError, CoreState,
    RuntimeConfigFile, RuntimeConfigFileError, SingBoxAdapter, SingBoxAdapterError, XrayAdapter,
    XrayAdapterError,
};
use magies_domain::{CoreType, ProxyNode};
use magies_platform::system_proxy::{
    PacSetting, ProxyEndpoint, ProxySetting, SystemProxyModelError, SystemProxyState,
};
use magies_platform::system_proxy_recovery::{
    RecoveryStore, SystemProxyControl, SystemProxyRecoveryError, SystemProxyRecoveryManager,
};
use magies_profiles::{
    CredentialCodec, CredentialCodecError, DnsProfile, LocalHttpProfile, LocalSocksProfile,
    RuntimeConfigError, SingBoxRuntimeConfigGenerator, SingBoxRuntimeProfile, StoredNodeCredential,
    TunProfile, XrayRuntimeConfigError, XrayRuntimeConfigGenerator, XrayRuntimeProfile,
};
use magies_routing::RouteProfile;
use magies_storage::{SecretStore, SecretStoreError};
use thiserror::Error;
use uuid::Uuid;

/// Parses a custom node's full Core JSON into a runtime document.
fn parse_custom_document<C, P>(
    document: &str,
) -> Result<serde_json::Value, DesktopSessionError<C, P>>
where
    C: Error + 'static,
    P: Error + 'static,
{
    let value: serde_json::Value = serde_json::from_str(document.trim())
        .map_err(|source| DesktopSessionError::InvalidCustomDocument { source })?;
    if !value.is_object() {
        return Err(DesktopSessionError::InvalidCustomDocumentNotObject);
    }
    Ok(value)
}

/// Builds the sing-box document for one profile.
fn generate_sing_box<C, P>(
    profile: &DesktopSessionProfile,
    credential: &StoredNodeCredential,
) -> Result<serde_json::Value, DesktopSessionError<C, P>>
where
    C: Error + 'static,
    P: Error + 'static,
{
    let mut runtime_profile = SingBoxRuntimeProfile::new(
        &profile.node,
        credential.as_node_credential(),
        &profile.dns,
        &profile.route,
    )
    .with_local_proxies(profile.socks, profile.http)
    .map_err(|source| DesktopSessionError::Config { source })?;
    if let Some(port) = profile.clash_api_port {
        runtime_profile = runtime_profile
            .with_clash_api_port(port)
            .map_err(|source| DesktopSessionError::Config { source })?;
    }
    if profile.mux_enabled {
        runtime_profile = runtime_profile.with_mux(true);
    }
    if profile.fragment_enabled {
        runtime_profile = runtime_profile.with_fragment(true);
    }
    if let Some(tun) = profile.tun.as_ref() {
        runtime_profile = runtime_profile.with_tun(tun, profile.dns_hijack);
    }
    Ok(SingBoxRuntimeConfigGenerator::generate(&runtime_profile)
        .map_err(|source| DesktopSessionError::Config { source })?
        .json()
        .clone())
}

/// Builds the Xray document for one profile.
///
/// TUN never reaches here in practice — the capability matrix keeps TUN
/// sessions on sing-box — but the refusal is explicit rather than a silently
/// dropped setting.
fn generate_xray<C, P>(
    profile: &DesktopSessionProfile,
    credential: &StoredNodeCredential,
) -> Result<serde_json::Value, DesktopSessionError<C, P>>
where
    C: Error + 'static,
    P: Error + 'static,
{
    if profile.tun.is_some() {
        return Err(DesktopSessionError::TunUnsupportedByCore {
            core: CoreType::Xray,
        });
    }
    let mut runtime_profile = XrayRuntimeProfile::new(
        &profile.node,
        credential.as_node_credential(),
        &profile.dns,
        &profile.route,
    )
    .with_local_proxies(profile.socks, profile.http)
    .map_err(|source| DesktopSessionError::XrayConfig { source })?;
    if let Some(port) = profile.clash_api_port {
        runtime_profile = runtime_profile.with_api_port(port);
    }
    if profile.mux_enabled {
        runtime_profile = runtime_profile.with_mux(true);
    }
    if profile.fragment_enabled {
        runtime_profile = runtime_profile.with_fragment(true);
    }
    if profile.udp_noise_enabled {
        runtime_profile = runtime_profile.with_udp_noise(true);
    }
    Ok(XrayRuntimeConfigGenerator::generate(&runtime_profile)
        .map_err(|source| DesktopSessionError::XrayConfig { source })?
        .json()
        .clone())
}

pub trait CoreSessionControl {
    type Error: Error + 'static;
    type Output;

    /// Tells the control which Core the next start will use.
    ///
    /// Defaults to doing nothing: an implementation that drives one Core has
    /// nothing to choose. A host that can run either overrides this.
    fn select_core(&mut self, _core: CoreType) {}

    /// Starts a validated Core with the generated configuration.
    ///
    /// # Errors
    ///
    /// Returns the Core adapter's typed startup error.
    fn start(&mut self, config_path: &Path) -> Result<Self::Output, Self::Error>;

    /// Stops the Core or accepts that it already exited.
    ///
    /// # Errors
    ///
    /// Returns the Core adapter's typed shutdown error.
    fn stop(&mut self) -> Result<(), Self::Error>;
}

pub trait SystemProxySessionControl {
    type Error: Error + 'static;

    /// Captures and applies the managed System Proxy state.
    ///
    /// # Errors
    ///
    /// Returns the platform adapter or recovery store error.
    fn enable(&mut self, state: &SystemProxyState) -> Result<(), Self::Error>;

    /// Restores the captured System Proxy state.
    ///
    /// # Errors
    ///
    /// Returns the platform adapter or recovery store error.
    fn stop(&mut self) -> Result<(), Self::Error>;
}

impl<C, S> SystemProxySessionControl for SystemProxyRecoveryManager<C, S>
where
    C: SystemProxyControl,
    S: RecoveryStore<C::Snapshot>,
    C::Error: 'static,
    S::Error: 'static,
{
    type Error = SystemProxyRecoveryError<C::Error, S::Error>;

    fn enable(&mut self, state: &SystemProxyState) -> Result<(), Self::Error> {
        SystemProxyRecoveryManager::enable(self, state)
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        SystemProxyRecoveryManager::stop(self)
    }
}

#[derive(Clone, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent session preference; collapsing them would hide meaning"
)]
pub struct DesktopSessionProfile {
    node: ProxyNode,
    core: CoreType,
    dns: DnsProfile,
    route: RouteProfile,
    socks: LocalSocksProfile,
    http: LocalHttpProfile,
    clash_api_port: Option<NonZeroU16>,
    tun: Option<TunProfile>,
    dns_hijack: bool,
    system_proxy: SystemProxyMode,
    mux_enabled: bool,
    fragment_enabled: bool,
    udp_noise_enabled: bool,
}

impl DesktopSessionProfile {
    #[must_use]
    pub fn new(node: ProxyNode, dns: DnsProfile, route: RouteProfile) -> Self {
        Self {
            node,
            // sing-box carries the general case; a caller that wants Xray says
            // so explicitly with `with_core`.
            core: CoreType::SingBox,
            dns,
            route,
            socks: LocalSocksProfile::default(),
            http: LocalHttpProfile::default(),
            clash_api_port: None,
            tun: None,
            dns_hijack: false,
            system_proxy: SystemProxyMode::Unchanged,
            mux_enabled: false,
            fragment_enabled: false,
            udp_noise_enabled: false,
        }
    }

    #[must_use]
    pub const fn with_local_proxies(
        mut self,
        socks: LocalSocksProfile,
        http: LocalHttpProfile,
    ) -> Self {
        self.socks = socks;
        self.http = http;
        self
    }

    #[must_use]
    pub fn with_tun(mut self, tun: TunProfile, dns_hijack: bool) -> Self {
        self.tun = Some(tun);
        self.dns_hijack = dns_hijack;
        self
    }

    #[must_use]
    pub const fn with_clash_api_port(mut self, port: NonZeroU16) -> Self {
        self.clash_api_port = Some(port);
        self
    }

    /// Turns on Core multiplex / mux for this session.
    #[must_use]
    pub const fn with_mux(mut self, enabled: bool) -> Self {
        self.mux_enabled = enabled;
        self
    }

    /// Turns on TLS `ClientHello` fragmentation (v2rayN's Fragment toggle) for
    /// this session.
    #[must_use]
    pub const fn with_fragment(mut self, enabled: bool) -> Self {
        self.fragment_enabled = enabled;
        self
    }

    /// Turns on Xray UDP noise (v2rayN-style freedom `noises`) for this session.
    #[must_use]
    pub const fn with_udp_noise(mut self, enabled: bool) -> Self {
        self.udp_noise_enabled = enabled;
        self
    }

    /// Chooses the Core this session runs.
    #[must_use]
    pub const fn with_core(mut self, core: CoreType) -> Self {
        self.core = core;
        self
    }

    /// Keeps the two-state spelling: enabled means the managed proxy, disabled
    /// means leaving the host's settings alone.
    #[must_use]
    pub fn with_system_proxy(mut self, enabled: bool) -> Self {
        self.system_proxy = if enabled {
            SystemProxyMode::Managed
        } else {
            SystemProxyMode::Unchanged
        };
        self
    }

    #[must_use]
    pub fn with_system_proxy_mode(mut self, mode: SystemProxyMode) -> Self {
        self.system_proxy = mode;
        self
    }

    #[must_use]
    pub const fn node(&self) -> &ProxyNode {
        &self.node
    }
}

/// What a session does to the host's System Proxy.
///
/// v2rayN offers the same three choices plus PAC, and the distinction between
/// the last two matters: leaving a proxy alone and clearing it are different
/// requests, which a single boolean could not express.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SystemProxyMode {
    /// Point the host at this session's local proxies.
    #[default]
    Managed,
    /// Point the host at a proxy auto-configuration file served at this URL.
    ///
    /// The URL is carried here rather than derived, because the server handing
    /// out the script lives in the shell and picks its own loopback port.
    Pac(String),
    /// Clear the host's proxy for the duration of the session.
    Cleared,
    /// Touch nothing, leaving whatever the user configured in place.
    Unchanged,
}

impl SystemProxyMode {
    /// Whether the host's settings are replaced, and therefore snapshotted and
    /// restored on stop.
    const fn writes_host_settings(&self) -> bool {
        matches!(self, Self::Managed | Self::Cleared | Self::Pac(_))
    }
}

struct ActiveSession {
    runtime_config: RuntimeConfigFile,
    system_proxy: bool,
    profile: DesktopSessionProfile,
}

pub struct DesktopSession<S, C, P> {
    secret_store: S,
    core: C,
    system_proxy: P,
    runtime_directory: PathBuf,
    active: Option<ActiveSession>,
}

impl<S, C, P> DesktopSession<S, C, P>
where
    S: SecretStore,
    C: CoreSessionControl,
    P: SystemProxySessionControl,
{
    #[must_use]
    pub fn new(
        secret_store: S,
        core: C,
        system_proxy: P,
        runtime_directory: impl Into<PathBuf>,
    ) -> Self {
        Self {
            secret_store,
            core,
            system_proxy,
            runtime_directory: runtime_directory.into(),
            active: None,
        }
    }

    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.active.is_some()
    }

    /// Borrows the injected store so callers can save a node's credential
    /// before starting a session with it.
    #[must_use]
    pub const fn secret_store(&self) -> &S {
        &self.secret_store
    }

    #[must_use]
    pub fn config_path(&self) -> Option<&Path> {
        self.active
            .as_ref()
            .map(|active| active.runtime_config.path())
    }

    /// Borrows the profile the running session started from, so a recovery can
    /// restart it without the caller re-assembling it.
    #[must_use]
    pub fn active_profile(&self) -> Option<&DesktopSessionProfile> {
        self.active.as_ref().map(|active| &active.profile)
    }

    /// Loads the node secret, generates and writes the configuration, starts
    /// the Core, and only then enables System Proxy.
    ///
    /// # Errors
    ///
    /// Returns a typed preparation, Core, or System Proxy error. Failed System
    /// Proxy activation stops the Core; a failed rollback remains stoppable.
    pub fn start(
        &mut self,
        profile: &DesktopSessionProfile,
    ) -> Result<C::Output, DesktopSessionError<C::Error, P::Error>> {
        if self.active.is_some() {
            return Err(DesktopSessionError::AlreadyRunning);
        }
        // Only the managed mode conflicts with TUN: clearing the host's proxy
        // while TUN carries the traffic is a coherent request, not a conflict.
        if matches!(
            profile.system_proxy,
            SystemProxyMode::Managed | SystemProxyMode::Pac(_)
        ) && profile.tun.is_some()
        {
            return Err(DesktopSessionError::ConflictingNetworkModes);
        }

        let payload = self
            .secret_store
            .get(&profile.node.credential_ref)
            .map_err(|source| DesktopSessionError::Secret { source })?;
        let credential = CredentialCodec::decode(&payload)
            .map_err(|source| DesktopSessionError::Credential { source })?;
        let generated = match &credential {
            StoredNodeCredential::Custom(custom) => {
                if profile.core != custom.core() {
                    return Err(DesktopSessionError::CustomCoreMismatch {
                        profile: profile.core,
                        required: custom.core(),
                    });
                }
                parse_custom_document(custom.document())?
            }
            _ => match profile.core {
                CoreType::SingBox => generate_sing_box(profile, &credential)?,
                CoreType::Xray => generate_xray(profile, &credential)?,
            },
        };
        let bytes = serde_json::to_vec(&generated)
            .map_err(|source| DesktopSessionError::Serialize { source })?;
        let path = self
            .runtime_directory
            .join(format!("session-{}.json", Uuid::new_v4()));
        let runtime_config = AtomicRuntimeConfig::write(path, &bytes)
            .map_err(|source| DesktopSessionError::RuntimeConfig { source })?;
        // Announced before the start so a host driving both Cores can pick the
        // right binary; the start order itself is unchanged.
        self.core.select_core(profile.core);
        let output = self
            .core
            .start(runtime_config.path())
            .map_err(|source| DesktopSessionError::CoreStart { source })?;

        if profile.system_proxy.writes_host_settings() {
            // Clearing goes through the same path as managing, so the user's own
            // settings are snapshotted first and restored on stop either way.
            let managed_state = match &profile.system_proxy {
                SystemProxyMode::Cleared => cleared_system_proxy(),
                SystemProxyMode::Pac(url) => pac_system_proxy(url)
                    .map_err(|source| DesktopSessionError::InvalidPacUrl { source })?,
                SystemProxyMode::Managed | SystemProxyMode::Unchanged => {
                    managed_system_proxy(profile.socks, profile.http)
                }
            };
            if let Err(proxy) = self.system_proxy.enable(&managed_state) {
                return match self.core.stop() {
                    Ok(()) => Err(DesktopSessionError::ProxyEnable { source: proxy }),
                    Err(core) => {
                        self.active = Some(ActiveSession {
                            runtime_config,
                            system_proxy: true,
                            profile: profile.clone(),
                        });
                        Err(DesktopSessionError::ProxyEnableAndCoreRollback { proxy, core })
                    }
                };
            }
        }

        self.active = Some(ActiveSession {
            runtime_config,
            system_proxy: profile.system_proxy.writes_host_settings(),
            profile: profile.clone(),
        });
        Ok(output)
    }

    /// Restores System Proxy first, then stops Core and deletes its config.
    ///
    /// # Errors
    ///
    /// Returns a typed error and retains the active session for a retry when
    /// either restoration or Core shutdown fails.
    pub fn stop(&mut self) -> Result<(), DesktopSessionError<C::Error, P::Error>> {
        let active = self
            .active
            .as_mut()
            .ok_or(DesktopSessionError::NotRunning)?;
        if active.system_proxy {
            self.system_proxy
                .stop()
                .map_err(|source| DesktopSessionError::ProxyStop { source })?;
            active.system_proxy = false;
        }
        self.core
            .stop()
            .map_err(|source| DesktopSessionError::CoreStop { source })?;
        self.active.take();
        Ok(())
    }
}

/// A System Proxy state naming a PAC URL and nothing else.
///
/// The fixed endpoints stay off: a host given both a PAC file and a fixed proxy
/// applies them in an order that differs per platform, so naming one is the only
/// way the result is predictable.
fn pac_system_proxy(url: &str) -> Result<SystemProxyState, SystemProxyModelError> {
    Ok(SystemProxyState::new(
        ProxySetting::disabled(),
        ProxySetting::disabled(),
        ProxySetting::disabled(),
        PacSetting::new(true, Some(url.to_owned()))?,
    ))
}

/// A System Proxy state with every protocol and PAC switched off.
fn cleared_system_proxy() -> SystemProxyState {
    SystemProxyState::new(
        ProxySetting::disabled(),
        ProxySetting::disabled(),
        ProxySetting::disabled(),
        PacSetting::disabled(),
    )
}

fn managed_system_proxy(socks: LocalSocksProfile, http: LocalHttpProfile) -> SystemProxyState {
    let http_endpoint = ProxyEndpoint::new("127.0.0.1", http.port().get())
        .expect("local HTTP profile always has a valid endpoint");
    let socks_endpoint = ProxyEndpoint::new("127.0.0.1", socks.port().get())
        .expect("local SOCKS profile always has a valid endpoint");
    let http_setting = ProxySetting::new(true, Some(http_endpoint))
        .expect("managed HTTP proxy always has an endpoint");
    let socks_setting = ProxySetting::new(true, Some(socks_endpoint))
        .expect("managed SOCKS proxy always has an endpoint");
    SystemProxyState::new(
        http_setting.clone(),
        http_setting,
        socks_setting,
        PacSetting::disabled(),
    )
}

#[derive(Debug, Error)]
pub enum DesktopSessionError<C, P>
where
    C: Error + 'static,
    P: Error + 'static,
{
    #[error("desktop proxy session is already running")]
    AlreadyRunning,
    #[error("the proxy auto-configuration URL is unusable")]
    InvalidPacUrl {
        #[source]
        source: SystemProxyModelError,
    },
    #[error("desktop proxy session is not running")]
    NotRunning,
    #[error("TUN and System Proxy cannot be enabled together")]
    ConflictingNetworkModes,
    #[error("failed to load node credential")]
    Secret {
        #[source]
        source: SecretStoreError,
    },
    #[error("failed to decode node credential")]
    Credential {
        #[source]
        source: CredentialCodecError,
    },
    #[error("custom node requires {required:?}, not {profile:?}")]
    CustomCoreMismatch {
        profile: CoreType,
        required: CoreType,
    },
    #[error("custom Core JSON is malformed")]
    InvalidCustomDocument {
        #[source]
        source: serde_json::Error,
    },
    #[error("custom Core JSON must be a JSON object")]
    InvalidCustomDocumentNotObject,
    #[error("{core:?} cannot provide TUN mode")]
    TunUnsupportedByCore { core: CoreType },
    #[error("failed to generate the Xray runtime configuration")]
    XrayConfig {
        #[source]
        source: XrayRuntimeConfigError,
    },
    #[error("failed to generate runtime configuration")]
    Config {
        #[source]
        source: RuntimeConfigError,
    },
    #[error("failed to serialize runtime configuration")]
    Serialize {
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to write runtime configuration")]
    RuntimeConfig {
        #[source]
        source: RuntimeConfigFileError,
    },
    #[error("failed to start Core")]
    CoreStart {
        #[source]
        source: C,
    },
    #[error("failed to enable System Proxy")]
    ProxyEnable {
        #[source]
        source: P,
    },
    #[error("failed to enable System Proxy and stop Core")]
    ProxyEnableAndCoreRollback { proxy: P, core: C },
    #[error("failed to restore System Proxy")]
    ProxyStop {
        #[source]
        source: P,
    },
    #[error("failed to stop Core")]
    CoreStop {
        #[source]
        source: C,
    },
}

pub struct SingBoxCoreControl {
    adapter: SingBoxAdapter,
    runtime: CoreRuntime,
    health_address: SocketAddr,
    health_timeout: Duration,
}

impl SingBoxCoreControl {
    #[must_use]
    pub fn new(
        adapter: SingBoxAdapter,
        health_address: SocketAddr,
        health_timeout: Duration,
    ) -> Self {
        Self {
            adapter,
            runtime: CoreRuntime::default(),
            health_address,
            health_timeout,
        }
    }

    fn stop_running_core(&mut self) -> Result<(), CoreRuntimeError> {
        if self.runtime.poll()? == CoreState::Running {
            self.runtime.stop()?;
        }
        Ok(())
    }
}

impl CoreSessionControl for SingBoxCoreControl {
    type Error = SingBoxCoreSessionError;
    type Output = CoreOutput;

    fn start(&mut self, config_path: &Path) -> Result<Self::Output, Self::Error> {
        let config = self
            .adapter
            .validate_config(config_path)
            .map_err(SingBoxCoreSessionError::Validate)?;
        let output = self
            .runtime
            .start(&self.adapter.process_spec(&config))
            .map_err(SingBoxCoreSessionError::Start)?;
        if let Err(health) = self
            .runtime
            .wait_for_tcp_health(self.health_address, self.health_timeout)
        {
            return match self.stop_running_core() {
                Ok(()) => Err(SingBoxCoreSessionError::Health(health)),
                Err(rollback) => {
                    Err(SingBoxCoreSessionError::HealthAndRollback { health, rollback })
                }
            };
        }
        Ok(output)
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.stop_running_core()
            .map_err(SingBoxCoreSessionError::Stop)
    }
}

/// Drives an Xray process for one session, mirroring [`SingBoxCoreControl`].
pub struct XrayCoreControl {
    adapter: XrayAdapter,
    runtime: CoreRuntime,
    health_address: SocketAddr,
    health_timeout: Duration,
}

impl XrayCoreControl {
    #[must_use]
    pub fn new(adapter: XrayAdapter, health_address: SocketAddr, health_timeout: Duration) -> Self {
        Self {
            adapter,
            runtime: CoreRuntime::default(),
            health_address,
            health_timeout,
        }
    }

    fn stop_running_core(&mut self) -> Result<(), CoreRuntimeError> {
        if self.runtime.poll()? == CoreState::Running {
            self.runtime.stop()?;
        }
        Ok(())
    }
}

impl CoreSessionControl for XrayCoreControl {
    type Error = XrayCoreSessionError;
    type Output = CoreOutput;

    fn start(&mut self, config_path: &Path) -> Result<Self::Output, Self::Error> {
        let config = self
            .adapter
            .validate_config(config_path)
            .map_err(XrayCoreSessionError::Validate)?;
        let output = self
            .runtime
            .start(&self.adapter.process_spec(&config))
            .map_err(XrayCoreSessionError::Start)?;
        if let Err(health) = self
            .runtime
            .wait_for_tcp_health(self.health_address, self.health_timeout)
        {
            return match self.stop_running_core() {
                Ok(()) => Err(XrayCoreSessionError::Health(health)),
                Err(rollback) => Err(XrayCoreSessionError::HealthAndRollback { health, rollback }),
            };
        }
        Ok(output)
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.stop_running_core().map_err(XrayCoreSessionError::Stop)
    }
}

#[derive(Debug, Error)]
pub enum XrayCoreSessionError {
    #[error("Xray configuration validation failed")]
    Validate(#[source] XrayAdapterError),
    #[error("Xray process failed to start")]
    Start(#[source] CoreRuntimeError),
    #[error("Xray process failed its health check")]
    Health(#[source] CoreHealthError),
    #[error("Xray health check and process rollback both failed")]
    HealthAndRollback {
        #[source]
        health: CoreHealthError,
        rollback: CoreRuntimeError,
    },
    #[error("Xray process failed to stop")]
    Stop(#[source] CoreRuntimeError),
}

#[derive(Debug, Error)]
pub enum SingBoxCoreSessionError {
    #[error("sing-box configuration validation failed")]
    Validate(#[source] SingBoxAdapterError),
    #[error("sing-box process failed to start")]
    Start(#[source] CoreRuntimeError),
    #[error("sing-box process failed its health check")]
    Health(#[source] CoreHealthError),
    #[error("sing-box health check and process rollback both failed")]
    HealthAndRollback {
        health: CoreHealthError,
        rollback: CoreRuntimeError,
    },
    #[error("sing-box process failed to stop")]
    Stop(#[source] CoreRuntimeError),
}
