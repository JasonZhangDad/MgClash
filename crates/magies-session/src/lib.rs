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
    RuntimeConfigFile, RuntimeConfigFileError, SingBoxAdapter, SingBoxAdapterError,
};
use magies_domain::ProxyNode;
use magies_platform::system_proxy::{PacSetting, ProxyEndpoint, ProxySetting, SystemProxyState};
use magies_platform::system_proxy_recovery::{
    RecoveryStore, SystemProxyControl, SystemProxyRecoveryError, SystemProxyRecoveryManager,
};
use magies_profiles::{
    CredentialCodec, CredentialCodecError, DnsProfile, LocalHttpProfile, LocalSocksProfile,
    RuntimeConfigError, SingBoxRuntimeConfigGenerator, SingBoxRuntimeProfile, TunProfile,
};
use magies_routing::RouteProfile;
use magies_storage::{SecretStore, SecretStoreError};
use thiserror::Error;
use uuid::Uuid;

pub trait CoreSessionControl {
    type Error: Error + 'static;
    type Output;

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
pub struct DesktopSessionProfile {
    node: ProxyNode,
    dns: DnsProfile,
    route: RouteProfile,
    socks: LocalSocksProfile,
    http: LocalHttpProfile,
    clash_api_port: Option<NonZeroU16>,
    tun: Option<TunProfile>,
    dns_hijack: bool,
    system_proxy: bool,
}

impl DesktopSessionProfile {
    #[must_use]
    pub fn new(node: ProxyNode, dns: DnsProfile, route: RouteProfile) -> Self {
        Self {
            node,
            dns,
            route,
            socks: LocalSocksProfile::default(),
            http: LocalHttpProfile::default(),
            clash_api_port: None,
            tun: None,
            dns_hijack: false,
            system_proxy: false,
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

    #[must_use]
    pub const fn with_system_proxy(mut self, enabled: bool) -> Self {
        self.system_proxy = enabled;
        self
    }

    #[must_use]
    pub const fn node(&self) -> &ProxyNode {
        &self.node
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
        if profile.system_proxy && profile.tun.is_some() {
            return Err(DesktopSessionError::ConflictingNetworkModes);
        }

        let payload = self
            .secret_store
            .get(&profile.node.credential_ref)
            .map_err(|source| DesktopSessionError::Secret { source })?;
        let credential = CredentialCodec::decode(&payload)
            .map_err(|source| DesktopSessionError::Credential { source })?;
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
        if let Some(tun) = profile.tun.as_ref() {
            runtime_profile = runtime_profile.with_tun(tun, profile.dns_hijack);
        }
        let generated = SingBoxRuntimeConfigGenerator::generate(&runtime_profile)
            .map_err(|source| DesktopSessionError::Config { source })?;
        let bytes = serde_json::to_vec(generated.json())
            .map_err(|source| DesktopSessionError::Serialize { source })?;
        let path = self
            .runtime_directory
            .join(format!("session-{}.json", Uuid::new_v4()));
        let runtime_config = AtomicRuntimeConfig::write(path, &bytes)
            .map_err(|source| DesktopSessionError::RuntimeConfig { source })?;
        let output = self
            .core
            .start(runtime_config.path())
            .map_err(|source| DesktopSessionError::CoreStart { source })?;

        if profile.system_proxy {
            let managed_state = managed_system_proxy(profile.socks, profile.http);
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
            system_proxy: profile.system_proxy,
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
