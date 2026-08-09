//! Proxy core process lifecycle.

mod binary;

use std::error::Error;
use std::ffi::OsString;
use std::fmt::{Display, Formatter};
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus};
use std::thread::sleep;
use std::time::{Duration, Instant};

pub use binary::{
    CoreBinaryError, CoreBinaryFormat, CoreBinaryRequirement, Sha256Hash, ValidatedCoreBinary,
    locate_core_binary,
};

const POLL_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreProcessSpec {
    executable: PathBuf,
    arguments: Vec<OsString>,
}

impl CoreProcessSpec {
    #[must_use]
    pub fn new<I, A>(binary: &ValidatedCoreBinary, arguments: I) -> Self
    where
        I: IntoIterator<Item = A>,
        A: Into<OsString>,
    {
        Self {
            executable: binary.path().to_path_buf(),
            arguments: arguments.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreExit {
    pub success: bool,
    pub code: Option<i32>,
}

impl From<ExitStatus> for CoreExit {
    fn from(status: ExitStatus) -> Self {
        Self {
            success: status.success(),
            code: status.code(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreState {
    Stopped,
    Running,
    Exited(CoreExit),
}

#[derive(Debug)]
pub enum CoreRuntimeError {
    AlreadyRunning,
    NotRunning,
    SpawnFailed {
        executable: PathBuf,
        source: io::Error,
    },
    PollFailed(io::Error),
    TerminateFailed(io::Error),
    WaitFailed(io::Error),
    WaitTimedOut {
        timeout: Duration,
    },
}

impl Display for CoreRuntimeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRunning => formatter.write_str("core process is already running"),
            Self::NotRunning => formatter.write_str("core process is not running"),
            Self::SpawnFailed { executable, source } => {
                write!(
                    formatter,
                    "failed to start core process {}: {source}",
                    executable.display()
                )
            }
            Self::PollFailed(source) => write!(formatter, "failed to poll core process: {source}"),
            Self::TerminateFailed(source) => {
                write!(formatter, "failed to terminate core process: {source}")
            }
            Self::WaitFailed(source) => {
                write!(formatter, "failed to wait for core process: {source}")
            }
            Self::WaitTimedOut { timeout } => {
                write!(formatter, "core process did not exit within {timeout:?}")
            }
        }
    }
}

impl Error for CoreRuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SpawnFailed { source, .. }
            | Self::PollFailed(source)
            | Self::TerminateFailed(source)
            | Self::WaitFailed(source) => Some(source),
            Self::AlreadyRunning | Self::NotRunning | Self::WaitTimedOut { .. } => None,
        }
    }
}

pub struct CoreRuntime {
    child: Option<Child>,
    state: CoreState,
}

impl Default for CoreRuntime {
    fn default() -> Self {
        Self {
            child: None,
            state: CoreState::Stopped,
        }
    }
}

impl CoreRuntime {
    /// Starts the configured Core process.
    ///
    /// # Errors
    ///
    /// Returns a typed error when another Core is running, the previous process
    /// cannot be polled, or the executable cannot be started.
    pub fn start(&mut self, spec: &CoreProcessSpec) -> Result<(), CoreRuntimeError> {
        if self.poll()? == CoreState::Running {
            return Err(CoreRuntimeError::AlreadyRunning);
        }

        let child = Command::new(&spec.executable)
            .args(&spec.arguments)
            .spawn()
            .map_err(|source| CoreRuntimeError::SpawnFailed {
                executable: spec.executable.clone(),
                source,
            })?;
        self.child = Some(child);
        self.state = CoreState::Running;

        Ok(())
    }

    /// Returns the current process state without blocking.
    ///
    /// # Errors
    ///
    /// Returns [`CoreRuntimeError::PollFailed`] when the operating system cannot
    /// report the child process state.
    pub fn poll(&mut self) -> Result<CoreState, CoreRuntimeError> {
        let Some(child) = self.child.as_mut() else {
            return Ok(self.state);
        };

        match child.try_wait().map_err(CoreRuntimeError::PollFailed)? {
            None => self.state = CoreState::Running,
            Some(status) => {
                self.child = None;
                self.state = CoreState::Exited(status.into());
            }
        }

        Ok(self.state)
    }

    /// Waits up to `timeout` for a running Core to exit.
    ///
    /// # Errors
    ///
    /// Returns a typed error when no Core is running, polling fails, or the
    /// timeout expires before the process exits.
    pub fn wait_for_exit(&mut self, timeout: Duration) -> Result<CoreExit, CoreRuntimeError> {
        let started_at = Instant::now();

        loop {
            match self.poll()? {
                CoreState::Exited(exit) => return Ok(exit),
                CoreState::Stopped => return Err(CoreRuntimeError::NotRunning),
                CoreState::Running if started_at.elapsed() >= timeout => {
                    return Err(CoreRuntimeError::WaitTimedOut { timeout });
                }
                CoreState::Running => {
                    let remaining = timeout.saturating_sub(started_at.elapsed());
                    sleep(POLL_INTERVAL.min(remaining));
                }
            }
        }
    }

    /// Force-stops the running Core and waits for its process to be reaped.
    ///
    /// # Errors
    ///
    /// Returns a typed error when no Core is running, polling fails, or the
    /// operating system cannot terminate or reap the process.
    pub fn stop(&mut self) -> Result<(), CoreRuntimeError> {
        if self.poll()? != CoreState::Running {
            return Err(CoreRuntimeError::NotRunning);
        }

        let Some(mut child) = self.child.take() else {
            self.state = CoreState::Stopped;
            return Err(CoreRuntimeError::NotRunning);
        };
        if let Err(source) = child.kill() {
            self.child = Some(child);
            return Err(CoreRuntimeError::TerminateFailed(source));
        }
        if let Err(source) = child.wait() {
            self.child = Some(child);
            return Err(CoreRuntimeError::WaitFailed(source));
        }
        self.state = CoreState::Stopped;

        Ok(())
    }
}

impl Drop for CoreRuntime {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };

        if let Err(error) = child.kill() {
            report_drop_error("terminate", child.id(), &error);
        }
        if let Err(error) = child.wait() {
            report_drop_error("reap", child.id(), &error);
        }
    }
}

fn report_drop_error(action: &str, process_id: u32, error: &io::Error) {
    eprintln!("failed to {action} Core process {process_id} during cleanup: {error}");
}
