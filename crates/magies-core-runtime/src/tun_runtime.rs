use std::error::Error;
use std::fmt::{Display, Formatter};
use std::net::SocketAddr;
use std::time::Duration;

use magies_platform::{OperatingSystem, TargetPlatform, TunAvailability};

use crate::{
    CoreExit, CoreHealth, CoreHealthError, CoreOutput, CoreProcessSpec, CoreRuntime,
    CoreRuntimeError, CoreState,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TunFailure {
    Start,
    Readiness,
    Cleanup,
    Inspection,
    Stop,
    CoreExited(CoreExit),
    CoreRecoveryFailed { attempts: u8 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TunState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed(TunFailure),
}

#[derive(Debug)]
pub struct TunStart {
    pub health: CoreHealth,
    pub output: CoreOutput,
}

#[derive(Debug)]
pub enum TunRuntimeError {
    UnsupportedPlatform(OperatingSystem),
    AlreadyRunning,
    Start(CoreRuntimeError),
    Readiness(CoreHealthError),
    ReadinessCleanup {
        readiness: CoreHealthError,
        cleanup: Box<CoreRuntimeError>,
    },
    Poll(CoreRuntimeError),
    Stop(CoreRuntimeError),
}

impl Display for TunRuntimeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform(platform) => {
                write!(
                    formatter,
                    "TUN is unavailable for unsigned {platform:?} builds"
                )
            }
            Self::AlreadyRunning => formatter.write_str("TUN is already running"),
            Self::Start(source) => write!(formatter, "failed to start TUN Core: {source}"),
            Self::Readiness(source) => write!(formatter, "TUN did not become ready: {source}"),
            Self::ReadinessCleanup { readiness, cleanup } => write!(
                formatter,
                "TUN did not become ready ({readiness}) and its Core could not be stopped: {cleanup}"
            ),
            Self::Poll(source) => write!(formatter, "failed to inspect TUN Core: {source}"),
            Self::Stop(source) => write!(formatter, "failed to stop TUN Core: {source}"),
        }
    }
}

impl Error for TunRuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Start(source) | Self::Poll(source) | Self::Stop(source) => Some(source),
            Self::Readiness(source)
            | Self::ReadinessCleanup {
                readiness: source, ..
            } => Some(source),
            Self::UnsupportedPlatform(_) | Self::AlreadyRunning => None,
        }
    }
}

pub struct TunRuntime {
    core: CoreRuntime,
    state: TunState,
}

impl Default for TunRuntime {
    fn default() -> Self {
        Self {
            core: CoreRuntime::default(),
            state: TunState::Stopped,
        }
    }
}

impl TunRuntime {
    #[must_use]
    pub const fn state(&self) -> TunState {
        self.state
    }

    #[must_use]
    pub fn process_id(&self) -> Option<u32> {
        self.core.process_id()
    }

    /// Starts a Windows or Linux TUN Core and waits for its local health port.
    ///
    /// # Errors
    ///
    /// Returns a typed error for unsigned macOS, an active TUN, Core startup,
    /// readiness, or failed readiness cleanup.
    pub fn start(
        &mut self,
        target: TargetPlatform,
        spec: &CoreProcessSpec,
        health_address: SocketAddr,
        health_timeout: Duration,
    ) -> Result<TunStart, TunRuntimeError> {
        if target.unsigned_tun_availability() == TunAvailability::UnavailableInUnsignedBuild {
            return Err(TunRuntimeError::UnsupportedPlatform(target.os()));
        }
        if matches!(
            self.state,
            TunState::Starting | TunState::Running | TunState::Stopping
        ) {
            return Err(TunRuntimeError::AlreadyRunning);
        }

        self.state = TunState::Starting;
        let output = match self.core.start(spec) {
            Ok(output) => output,
            Err(source) => {
                self.state = TunState::Failed(TunFailure::Start);
                return Err(TunRuntimeError::Start(source));
            }
        };
        match self
            .core
            .wait_for_tcp_health(health_address, health_timeout)
        {
            Ok(health) => {
                self.state = TunState::Running;
                Ok(TunStart { health, output })
            }
            Err(readiness) => {
                drop(output);
                let cleanup = self.cleanup_after_failed_start();
                if let Err(cleanup) = cleanup {
                    self.state = TunState::Failed(TunFailure::Cleanup);
                    return Err(TunRuntimeError::ReadinessCleanup {
                        readiness,
                        cleanup: Box::new(cleanup),
                    });
                }
                self.state = TunState::Failed(TunFailure::Readiness);
                Err(TunRuntimeError::Readiness(readiness))
            }
        }
    }

    /// Returns the current TUN state and records an unexpected Core exit.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the Core process cannot be inspected.
    pub fn poll(&mut self) -> Result<TunState, TunRuntimeError> {
        if self.state != TunState::Running {
            return Ok(self.state);
        }
        self.state = match self.core.poll() {
            Ok(CoreState::Stopped) => TunState::Stopped,
            Ok(CoreState::Running) => TunState::Running,
            Ok(CoreState::Exited(exit)) => TunState::Failed(TunFailure::CoreExited(exit)),
            Ok(CoreState::Failed { attempts }) => {
                TunState::Failed(TunFailure::CoreRecoveryFailed { attempts })
            }
            Err(source) => {
                self.state = TunState::Failed(TunFailure::Inspection);
                return Err(TunRuntimeError::Poll(source));
            }
        };
        Ok(self.state)
    }

    /// Stops the TUN Core. Stopping an inactive TUN succeeds without mutation.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the Core cannot be inspected or stopped.
    pub fn stop(&mut self) -> Result<(), TunRuntimeError> {
        if self.state == TunState::Stopped {
            return Ok(());
        }
        self.state = TunState::Stopping;
        let core_state = match self.core.poll() {
            Ok(state) => state,
            Err(source) => {
                self.state = TunState::Failed(TunFailure::Inspection);
                return Err(TunRuntimeError::Poll(source));
            }
        };
        match core_state {
            CoreState::Running => {
                if let Err(source) = self.core.stop() {
                    self.state = TunState::Failed(TunFailure::Stop);
                    return Err(TunRuntimeError::Stop(source));
                }
            }
            CoreState::Stopped | CoreState::Exited(_) | CoreState::Failed { .. } => {}
        }
        self.state = TunState::Stopped;
        Ok(())
    }

    fn cleanup_after_failed_start(&mut self) -> Result<(), CoreRuntimeError> {
        if self.core.poll()? == CoreState::Running {
            self.core.stop()?;
        }
        Ok(())
    }
}
