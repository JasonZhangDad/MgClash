//! Lazily validated sing-box control for the desktop shell.
//!
//! Release artifacts are unsigned and ship the pinned Core next to the app, so
//! its location and SHA-256 come from configuration. Resolving that pin is
//! deferred to the first connect: an unconfigured or tampered Core then fails
//! with a typed error the dashboard can show, instead of preventing startup.

use std::error::Error;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(unix)]
use magies_core_runtime::elevated::ElevatedCore;
#[cfg(target_os = "macos")]
use magies_core_runtime::elevated::OsascriptLauncher as HostElevationLauncher;
#[cfg(all(unix, not(target_os = "macos")))]
use magies_core_runtime::elevated::PkexecLauncher as HostElevationLauncher;
use magies_core_runtime::{
    CoreBinaryError, CoreBinaryRequirement, CoreOutput, Sha256Hash, Sha256HashParseError,
    SingBoxAdapter, SingBoxAdapterError, ValidatedCoreBinary, XrayAdapter, XrayAdapterError,
    locate_core_binary,
};
use magies_domain::CoreType;
use magies_platform::{PlatformError, TargetPlatform};
use magies_session::{
    CoreSessionControl, SingBoxCoreControl, SingBoxCoreSessionError, XrayCoreControl,
    XrayCoreSessionError,
};
#[cfg(unix)]
use magies_session::{ElevatedSingBoxControl, ElevatedSingBoxSessionError};
use thiserror::Error;

pub const BINARY_PATH_VARIABLE: &str = "MAGIES_SING_BOX_BIN";
pub const SHA256_VARIABLE: &str = "MAGIES_SING_BOX_SHA256";

/// The Core file name an artifact ships, if it ships one.
const BUNDLED_CORE_STEM: &str = "sing-box";

/// The digest baked in when the release build set `MAGIES_SING_BOX_SHA256`.
///
/// Reading the pin from a file next to the Core would be no pin at all —
/// anything that can replace the binary can replace the file beside it. Baking
/// it into this executable at build time is what makes the check meaningful.
const BUILD_TIME_SHA256: Option<&str> = option_env!("MAGIES_SING_BOX_SHA256");

/// The pinned sing-box binary this build is allowed to run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreSettings {
    pub binary: PathBuf,
    pub sha256: Sha256Hash,
}

impl CoreSettings {
    /// Reads the pinned Core location and digest from the environment.
    ///
    /// # Errors
    ///
    /// Returns a typed error when either variable is missing or the digest is
    /// not 32 hex-encoded bytes.
    pub fn from_env() -> Result<Self, CoreSettingsError> {
        let executable_directory = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .unwrap_or_default();
        Self::resolve_from(
            std::env::var_os(BINARY_PATH_VARIABLE).map(PathBuf::from),
            std::env::var(SHA256_VARIABLE).ok(),
            &executable_directory,
            BUILD_TIME_SHA256,
        )
    }

    /// Resolves the Core from an explicit override, falling back to one shipped
    /// inside the artifact with a digest pinned at build time.
    ///
    /// The runtime override wins so a developer can point a packaged build at a
    /// different Core without rebuilding.
    ///
    /// # Errors
    ///
    /// Returns a typed error when no Core can be located, when a located Core
    /// has no digest to check it against, or when a digest is malformed.
    pub fn resolve_from(
        binary: Option<PathBuf>,
        sha256: Option<String>,
        executable_directory: &Path,
        build_time_sha256: Option<&str>,
    ) -> Result<Self, CoreSettingsError> {
        let binary = binary.or_else(|| bundled_core_in(executable_directory));
        let sha256 = sha256.or_else(|| build_time_sha256.map(str::to_owned));
        Self::from_values(binary, sha256)
    }

    /// Validates an already-read Core location and digest.
    ///
    /// # Errors
    ///
    /// Returns a typed error when either value is missing or the digest is not
    /// 32 hex-encoded bytes.
    pub fn from_values(
        binary: Option<PathBuf>,
        sha256: Option<String>,
    ) -> Result<Self, CoreSettingsError> {
        let binary = binary.ok_or(CoreSettingsError::MissingBinaryPath)?;
        let sha256 = sha256
            .ok_or(CoreSettingsError::MissingSha256)?
            .parse()
            .map_err(CoreSettingsError::InvalidSha256)?;
        Ok(Self { binary, sha256 })
    }
}

/// A [`CoreSessionControl`] that validates its binary on the first start.
pub struct LazySingBoxControl {
    settings: Result<CoreSettings, CoreSettingsError>,
    health_address: SocketAddr,
    health_timeout: Duration,
    control: Option<SingBoxCoreControl>,
}

impl LazySingBoxControl {
    #[must_use]
    pub const fn new(
        settings: Result<CoreSettings, CoreSettingsError>,
        health_address: SocketAddr,
        health_timeout: Duration,
    ) -> Self {
        Self {
            settings,
            health_address,
            health_timeout,
            control: None,
        }
    }

    pub fn replace_settings(&mut self, settings: Result<CoreSettings, CoreSettingsError>) {
        let _ = self.stop();
        self.settings = settings;
        self.control = None;
    }

    /// Reads the pinned Core from the environment, deferring failures to the
    /// first connect.
    #[must_use]
    pub fn from_env(health_address: SocketAddr, health_timeout: Duration) -> Self {
        Self::new(CoreSettings::from_env(), health_address, health_timeout)
    }

    /// The health port and deadline the plain control was built with, so a
    /// TUN start waits on the same readiness signal.
    #[cfg(unix)]
    pub(crate) const fn health_address(&self) -> SocketAddr {
        self.health_address
    }

    #[cfg(unix)]
    pub(crate) const fn health_timeout(&self) -> Duration {
        self.health_timeout
    }

    /// Locates and verifies the pinned sing-box binary.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the pin is unconfigured, the target is
    /// unsupported, or the binary does not match its digest.
    pub(crate) fn resolve_binary(&self) -> Result<ValidatedCoreBinary, LazySingBoxError> {
        let settings = self
            .settings
            .as_ref()
            .map_err(|source| LazySingBoxError::Settings(source.clone()))?;
        let target = TargetPlatform::parse(std::env::consts::OS, std::env::consts::ARCH)
            .map_err(LazySingBoxError::Target)?;
        locate_core_binary(
            &settings.binary,
            CoreBinaryRequirement::new(target.architecture(), settings.sha256),
        )
        .map_err(|source| LazySingBoxError::Binary(Box::new(source)))
    }

    fn resolve_control(&mut self) -> Result<(), LazySingBoxError> {
        if self.control.is_some() {
            return Ok(());
        }

        let binary = self.resolve_binary()?;
        self.control = Some(SingBoxCoreControl::new(
            SingBoxAdapter::new(binary),
            self.health_address,
            self.health_timeout,
        ));
        Ok(())
    }
}

impl CoreSessionControl for LazySingBoxControl {
    type Error = LazySingBoxError;
    type Output = CoreOutput;

    fn start(&mut self, config_path: &Path) -> Result<Self::Output, Self::Error> {
        self.resolve_control()?;
        self.control
            .as_mut()
            .expect("resolve_control creates the control whenever it is missing")
            .start(config_path)
            .map_err(|source| LazySingBoxError::Session(Box::new(source)))
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        match self.control.as_mut() {
            Some(control) => control
                .stop()
                .map_err(|source| LazySingBoxError::Session(Box::new(source))),
            None => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CoreSettingsError {
    #[error("{BINARY_PATH_VARIABLE} must point to the pinned sing-box binary")]
    MissingBinaryPath,
    #[error("{SHA256_VARIABLE} must contain the pinned sing-box SHA-256")]
    MissingSha256,
    #[error("{SHA256_VARIABLE} is not a SHA-256 digest")]
    InvalidSha256(#[source] Sha256HashParseError),
}

/// A [`CoreSessionControl`] that drives whichever Core the session selected.
///
/// Both Cores stay lazy: neither binary is located or verified until a session
/// actually starts on it, so a host with only one installed still works as long
/// as the user does not pick the other.
pub struct HostCoreControl {
    sing_box: LazySingBoxControl,
    xray: LazyXrayControl,
    current: CoreType,
    /// Whether the next start needs a TUN device. Only Unix acts on it: macOS
    /// and Linux can both raise privileges themselves, so there
    /// the device belongs to root, so that start is a different one.
    #[cfg(unix)]
    tun: bool,
    #[cfg(unix)]
    elevated: Option<ElevatedSingBoxControl<HostElevationLauncher>>,
    /// Where the elevated Core leaves its PID and log files.
    #[cfg(unix)]
    runtime_directory: PathBuf,
}

impl HostCoreControl {
    #[must_use]
    pub fn from_env(health_address: SocketAddr, health_timeout: Duration) -> Self {
        Self {
            sing_box: LazySingBoxControl::from_env(health_address, health_timeout),
            xray: LazyXrayControl::from_env(health_address, health_timeout),
            current: CoreType::SingBox,
            #[cfg(unix)]
            tun: false,
            #[cfg(unix)]
            elevated: None,
            #[cfg(unix)]
            runtime_directory: std::env::temp_dir(),
        }
    }

    /// Builds Core controls from an optional user install store.
    #[must_use]
    pub fn from_install(
        install: Option<&crate::core_install::CoreInstallStore>,
        health_address: SocketAddr,
        health_timeout: Duration,
    ) -> Self {
        Self {
            sing_box: LazySingBoxControl::new(
                crate::core_install::sing_box_settings_with_store(install),
                health_address,
                health_timeout,
            ),
            xray: LazyXrayControl::new(
                crate::core_install::xray_settings_with_store(install),
                health_address,
                health_timeout,
            ),
            current: CoreType::SingBox,
            #[cfg(unix)]
            tun: false,
            #[cfg(unix)]
            elevated: None,
            #[cfg(unix)]
            runtime_directory: std::env::temp_dir(),
        }
    }

    /// Points the elevated Core's PID and log files at the session runtime
    /// directory, where the app already writes the generated config.
    #[cfg(unix)]
    #[must_use]
    pub fn with_runtime_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.runtime_directory = directory.into();
        self
    }

    /// Accepted and ignored: only Unix starts a Core it does not own, so
    /// nowhere else is there a PID or log file to place.
    #[cfg(not(unix))]
    #[must_use]
    pub fn with_runtime_directory(self, _directory: impl Into<PathBuf>) -> Self {
        self
    }

    /// Reloads Core paths after a user-triggered install.
    pub fn apply_install_store(&mut self, install: &crate::core_install::CoreInstallStore) {
        self.sing_box
            .replace_settings(crate::core_install::sing_box_settings_with_store(Some(
                install,
            )));
        self.xray
            .replace_settings(crate::core_install::xray_settings_with_store(Some(install)));
    }

    /// Adopts an elevated Core left behind by an app that did not stop it.
    ///
    /// Called once at startup: a crash leaves a root Core still holding the TUN
    /// device, and the PID file beside the runtime config is the only record of
    /// it. Adopting it makes it stoppable again — the next TUN start or
    /// disconnect ends it, both of which the user asked for. Nothing is killed
    /// here, so launching the app never asks for a password on its own.
    #[cfg(unix)]
    pub fn reclaim_elevated_core(&mut self) -> Option<u32> {
        let mut core = ElevatedCore::new(
            HostElevationLauncher,
            self.runtime_directory.join("elevated-core.pid"),
            self.runtime_directory.join("elevated-core.log"),
        );
        let pid = core.reclaim()?;
        self.elevated = Some(ElevatedSingBoxControl::new(
            self.sing_box.resolve_binary().ok()?,
            core,
            self.sing_box.health_address(),
            self.sing_box.health_timeout(),
        ));
        Some(pid)
    }

    /// Whether the next start needs the authorization prompt.
    ///
    /// Only a sing-box TUN session does, and only where the app can raise
    /// privileges itself: the device belongs to root on macOS and needs
    /// `CAP_NET_ADMIN` on Linux. Xray has no TUN inbound to open.
    #[cfg(unix)]
    #[must_use]
    pub const fn selects_elevated_start(&self) -> bool {
        self.tun && matches!(self.current, CoreType::SingBox)
    }

    /// Starts sing-box behind the authorization prompt a `utun` needs.
    ///
    /// The binary is resolved and pinned exactly as the plain path resolves it
    /// — running an unverified Core would be worse as root, not better.
    #[cfg(unix)]
    fn start_elevated(&mut self, config_path: &Path) -> Result<CoreOutput, HostCoreError> {
        // The child-process control cannot own this session; stopping it here
        // keeps a Core from an earlier non-TUN session from lingering.
        self.sing_box.stop().map_err(HostCoreError::SingBox)?;
        // An adopted leftover, or the Core of a session that was not stopped:
        // replacing the control without stopping it would strand a root
        // process holding the device this start is about to ask for.
        if let Some(control) = self.elevated.as_mut() {
            control.stop().map_err(HostCoreError::Elevated)?;
        }
        let binary = self
            .sing_box
            .resolve_binary()
            .map_err(HostCoreError::SingBox)?;
        let control = self.elevated.insert(ElevatedSingBoxControl::new(
            binary,
            ElevatedCore::new(
                HostElevationLauncher,
                self.runtime_directory.join("elevated-core.pid"),
                self.runtime_directory.join("elevated-core.log"),
            ),
            self.sing_box.health_address(),
            self.sing_box.health_timeout(),
        ));
        control.start(config_path).map_err(HostCoreError::Elevated)
    }

    /// Points Xray at the desktop Geo assets directory.
    #[must_use]
    pub fn with_xray_asset_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.xray = self.xray.with_asset_directory(directory);
        self
    }
}

impl CoreSessionControl for HostCoreControl {
    type Error = HostCoreError;
    type Output = CoreOutput;

    fn select_core(&mut self, core: CoreType) {
        self.current = core;
    }

    #[cfg_attr(
        not(unix),
        expect(
            unused_variables,
            reason = "only Unix starts a TUN Core differently from a plain one"
        )
    )]
    fn select_network_mode(&mut self, tun: bool) {
        #[cfg(unix)]
        {
            self.tun = tun;
        }
    }

    fn start(&mut self, config_path: &Path) -> Result<Self::Output, Self::Error> {
        #[cfg(unix)]
        if self.selects_elevated_start() {
            return self.start_elevated(config_path);
        }
        match self.current {
            CoreType::SingBox => self
                .sing_box
                .start(config_path)
                .map_err(HostCoreError::SingBox),
            CoreType::Xray => self.xray.start(config_path).map_err(HostCoreError::Xray),
        }
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        // All of them are stopped: a Core switched away from mid-session — or a
        // TUN toggled off — would otherwise be left running.
        #[cfg(unix)]
        let elevated = match self.elevated.as_mut() {
            Some(control) => control.stop().map_err(HostCoreError::Elevated),
            None => Ok(()),
        };
        let sing_box = self.sing_box.stop().map_err(HostCoreError::SingBox);
        let xray = self.xray.stop().map_err(HostCoreError::Xray);
        #[cfg(unix)]
        return elevated.and(sing_box).and(xray);
        #[cfg(not(unix))]
        return sing_box.and(xray);
    }
}

#[derive(Debug, Error)]
pub enum HostCoreError {
    #[error(transparent)]
    SingBox(LazySingBoxError),
    #[error(transparent)]
    Xray(LazyXrayError),
    #[cfg(unix)]
    #[error(transparent)]
    Elevated(ElevatedSingBoxSessionError),
}

impl HostCoreError {
    /// The stable machine-readable code the UI branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            #[cfg(unix)]
            Self::Elevated(error) => error.code(),
            Self::SingBox(error) => error.code(),
            Self::Xray(error) => error.code(),
        }
    }
}

/// The Xray counterpart to [`LazySingBoxControl`].
pub struct LazyXrayControl {
    settings: Result<CoreSettings, CoreSettingsError>,
    health_address: SocketAddr,
    health_timeout: Duration,
    asset_directory: Option<PathBuf>,
    control: Option<XrayCoreControl>,
}

impl LazyXrayControl {
    #[must_use]
    pub const fn new(
        settings: Result<CoreSettings, CoreSettingsError>,
        health_address: SocketAddr,
        health_timeout: Duration,
    ) -> Self {
        Self {
            settings,
            health_address,
            health_timeout,
            asset_directory: None,
            control: None,
        }
    }

    /// Points future Xray sessions at a Geo assets directory.
    #[must_use]
    pub fn with_asset_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.asset_directory = Some(directory.into());
        self
    }

    pub fn replace_settings(&mut self, settings: Result<CoreSettings, CoreSettingsError>) {
        let _ = self.stop();
        self.settings = settings;
        self.control = None;
    }

    /// Reads the Xray binary and digest from the environment.
    ///
    /// Unlike sing-box there is no digest compiled in at build time: ADR 0003
    /// records that this repo has no verified official Xray digest, so a user
    /// choosing Xray has to supply both values.
    #[must_use]
    pub fn from_env(health_address: SocketAddr, health_timeout: Duration) -> Self {
        let settings = CoreSettings::from_values(
            std::env::var_os(XRAY_BINARY_VARIABLE).map(PathBuf::from),
            std::env::var(XRAY_SHA256_VARIABLE).ok(),
        );
        Self::new(settings, health_address, health_timeout)
    }

    fn resolve_control(&mut self) -> Result<(), LazyXrayError> {
        if self.control.is_some() {
            return Ok(());
        }

        let settings = self
            .settings
            .as_ref()
            .map_err(|source| LazyXrayError::Settings(source.clone()))?;
        let target = TargetPlatform::parse(std::env::consts::OS, std::env::consts::ARCH)
            .map_err(LazyXrayError::Target)?;
        let binary = locate_core_binary(
            &settings.binary,
            CoreBinaryRequirement::new(target.architecture(), settings.sha256),
        )
        .map_err(|source| LazyXrayError::Binary(Box::new(source)))?;
        let mut adapter = XrayAdapter::new(binary);
        if let Some(directory) = &self.asset_directory {
            adapter = adapter.with_asset_directory(directory.clone());
        }
        self.control = Some(XrayCoreControl::new(
            adapter,
            self.health_address,
            self.health_timeout,
        ));
        Ok(())
    }
}

impl CoreSessionControl for LazyXrayControl {
    type Error = LazyXrayError;
    type Output = CoreOutput;

    fn start(&mut self, config_path: &Path) -> Result<Self::Output, Self::Error> {
        self.resolve_control()?;
        self.control
            .as_mut()
            .expect("resolve_control creates the control whenever it is missing")
            .start(config_path)
            .map_err(|source| LazyXrayError::Session(Box::new(source)))
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        match self.control.as_mut() {
            Some(control) => control
                .stop()
                .map_err(|source| LazyXrayError::Session(Box::new(source))),
            None => Ok(()),
        }
    }
}

#[derive(Debug, Error)]
pub enum LazyXrayError {
    #[error("the Xray binary is not configured")]
    Settings(#[source] CoreSettingsError),
    #[error("this build runs on a target outside the V0.1 support matrix")]
    Target(#[source] PlatformError),
    #[error("the configured Xray binary does not match its pin")]
    Binary(#[source] Box<CoreBinaryError>),
    #[error("the Xray session failed")]
    Session(#[source] Box<XrayCoreSessionError>),
}

impl LazyXrayError {
    /// The stable machine-readable code the UI branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Settings(_) => "xray_unavailable",
            Self::Target(_) => "unsupported_target",
            Self::Binary(_) => "xray_binary_rejected",
            Self::Session(_) => "core_session_failed",
        }
    }
}

/// Where the user points the app at an Xray build.
pub const XRAY_BINARY_VARIABLE: &str = "MAGIES_XRAY_BIN";
/// The digest that build must match.
pub const XRAY_SHA256_VARIABLE: &str = "MAGIES_XRAY_SHA256";

#[derive(Debug, Error)]
pub enum LazySingBoxError {
    #[error("the pinned sing-box binary is not configured")]
    Settings(#[source] CoreSettingsError),
    #[error("this build runs on a target outside the V0.1 support matrix")]
    Target(#[source] PlatformError),
    #[error("the configured sing-box binary does not match its pin")]
    Binary(#[source] Box<CoreBinaryError>),
    #[error("the sing-box session failed")]
    Session(#[source] Box<SingBoxCoreSessionError>),
}

impl LazySingBoxError {
    /// The stable machine-readable code the UI branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Settings(_) => "core_not_configured",
            Self::Target(_) => "unsupported_target",
            Self::Binary(_) => "core_binary_rejected",
            Self::Session(_) => "core_session_failed",
        }
    }
}

/// Locates a Core shipped inside the artifact, relative to the executable.
///
/// Windows and Linux put it beside the executable; a macOS `.app` puts it in
/// `Contents/Resources`, which is `../Resources` from `Contents/MacOS`.
#[must_use]
pub fn bundled_core_in(executable_directory: &Path) -> Option<PathBuf> {
    let file_name = format!("{BUNDLED_CORE_STEM}{}", std::env::consts::EXE_SUFFIX);
    [
        executable_directory.join(&file_name),
        executable_directory
            .join("..")
            .join("Resources")
            .join(&file_name),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

/// Renders an error and every cause behind it for the developer-facing log.
pub fn describe(error: &dyn Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

/// Reads the installed sing-box version, preferring a user-installed binary.
///
/// # Errors
///
/// Returns a typed error when the Core settings cannot be resolved or the
/// binary does not report a version.
pub fn read_sing_box_version_from_settings_with_store(
    install: Option<&crate::core_install::CoreInstallStore>,
) -> Result<String, SingBoxVersionReadError> {
    let settings = crate::core_install::sing_box_settings_with_store(install)
        .map_err(SingBoxVersionReadError::Settings)?;
    read_version_from_settings(&settings)
}

/// Reads sing-box's version from the validated binary when it is configured.
///
/// # Errors
///
/// Returns a typed error when settings, the binary pin, or `--version` fails.
pub fn read_sing_box_version_from_settings() -> Result<String, SingBoxVersionReadError> {
    read_sing_box_version_from_settings_with_store(None)
}

fn read_version_from_settings(settings: &CoreSettings) -> Result<String, SingBoxVersionReadError> {
    let target = TargetPlatform::parse(std::env::consts::OS, std::env::consts::ARCH)
        .map_err(SingBoxVersionReadError::Target)?;
    let binary = locate_core_binary(
        &settings.binary,
        CoreBinaryRequirement::new(target.architecture(), settings.sha256),
    )
    .map_err(SingBoxVersionReadError::Binary)?;
    SingBoxAdapter::new(binary)
        .version()
        .map(|version| version.as_str().to_owned())
        .map_err(SingBoxVersionReadError::Version)
}

/// Reads Xray's version from the validated binary when it is configured.
///
/// # Errors
///
/// Returns a typed error when settings, the binary pin, or `version` fails.
pub fn read_xray_version_from_settings_with_store(
    install: Option<&crate::core_install::CoreInstallStore>,
) -> Result<String, XrayVersionReadError> {
    let settings = crate::core_install::xray_settings_with_store(install)
        .map_err(XrayVersionReadError::Settings)?;
    read_xray_version_from_core_settings(&settings)
}

/// Reads Xray's version from the validated binary when it is configured.
///
/// # Errors
///
/// Returns a typed error when settings, the binary pin, or `version` fails.
pub fn read_xray_version_from_settings() -> Result<String, XrayVersionReadError> {
    read_xray_version_from_settings_with_store(None)
}

fn read_xray_version_from_core_settings(
    settings: &CoreSettings,
) -> Result<String, XrayVersionReadError> {
    let target = TargetPlatform::parse(std::env::consts::OS, std::env::consts::ARCH)
        .map_err(XrayVersionReadError::Target)?;
    let binary = locate_core_binary(
        &settings.binary,
        CoreBinaryRequirement::new(target.architecture(), settings.sha256),
    )
    .map_err(XrayVersionReadError::Binary)?;
    XrayAdapter::new(binary)
        .version()
        .map(|version| version.as_str().to_owned())
        .map_err(XrayVersionReadError::Version)
}

#[derive(Debug, Error)]
pub enum SingBoxVersionReadError {
    #[error(transparent)]
    Settings(#[from] CoreSettingsError),
    #[error(transparent)]
    Target(#[from] PlatformError),
    #[error(transparent)]
    Binary(#[from] CoreBinaryError),
    #[error(transparent)]
    Version(#[from] SingBoxAdapterError),
}

#[derive(Debug, Error)]
pub enum XrayVersionReadError {
    #[error(transparent)]
    Settings(#[from] CoreSettingsError),
    #[error(transparent)]
    Target(#[from] PlatformError),
    #[error(transparent)]
    Binary(#[from] CoreBinaryError),
    #[error(transparent)]
    Version(#[from] XrayAdapterError),
}
