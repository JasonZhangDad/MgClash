//! Proxy core process lifecycle.

mod adapter;
mod binary;
mod health;
mod local_proxy_ports;
mod output;
mod recovery;
mod sing_box;
mod xray;

use std::error::Error;
use std::ffi::OsString;
use std::fmt::{Display, Formatter};
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

pub use binary::{
    CoreBinaryError, CoreBinaryFormat, CoreBinaryRequirement, Sha256Hash, ValidatedCoreBinary,
    locate_core_binary,
};
pub use health::{CoreHealth, CoreHealthError};
pub use local_proxy_ports::{LocalProxyPortChecker, LocalProxyPortError, LocalProxyPortKind};
pub use output::{CoreOutput, CoreOutputEvent, CoreOutputStream};
pub use recovery::{
    CoreRecovery, CoreRecoveryError, CoreRecoveryFailure, MAX_CRASH_RECOVERY_ATTEMPTS,
};
pub use sing_box::{
    SingBoxAdapter, SingBoxAdapterError, SingBoxOperation, SingBoxVersion, ValidatedSingBoxConfig,
};
pub use xray::{ValidatedXrayConfig, XrayAdapter, XrayAdapterError, XrayOperation, XrayVersion};

const POLL_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreProcessSpec {
    binary: ValidatedCoreBinary,
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
            binary: binary.clone(),
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
    Failed { attempts: u8 },
}

#[derive(Debug)]
pub enum CoreRuntimeError {
    AlreadyRunning,
    NotRunning,
    BinaryValidationFailed(CoreBinaryError),
    SpawnFailed {
        executable: PathBuf,
        source: io::Error,
    },
    OutputPipeUnavailable(CoreOutputStream),
    OutputReaderSpawnFailed {
        stream: CoreOutputStream,
        source: io::Error,
    },
    OutputReaderPanicked(CoreOutputStream),
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
            Self::BinaryValidationFailed(source) => {
                write!(formatter, "Core binary failed launch validation: {source}")
            }
            Self::SpawnFailed { executable, source } => {
                write!(
                    formatter,
                    "failed to start core process {}: {source}",
                    executable.display()
                )
            }
            Self::OutputPipeUnavailable(stream) => {
                write!(formatter, "Core {stream:?} pipe is unavailable")
            }
            Self::OutputReaderSpawnFailed { stream, source } => {
                write!(
                    formatter,
                    "failed to start Core {stream:?} reader: {source}"
                )
            }
            Self::OutputReaderPanicked(stream) => {
                write!(formatter, "Core {stream:?} reader panicked")
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
            Self::BinaryValidationFailed(source) => Some(source),
            Self::SpawnFailed { source, .. }
            | Self::OutputReaderSpawnFailed { source, .. }
            | Self::PollFailed(source)
            | Self::TerminateFailed(source)
            | Self::WaitFailed(source) => Some(source),
            Self::AlreadyRunning
            | Self::NotRunning
            | Self::OutputPipeUnavailable(_)
            | Self::OutputReaderPanicked(_)
            | Self::WaitTimedOut { .. } => None,
        }
    }
}

pub struct CoreRuntime {
    child: Option<Child>,
    output_readers: Vec<output::CoreOutputReader>,
    recovery_attempts: u8,
    state: CoreState,
}

impl Default for CoreRuntime {
    fn default() -> Self {
        Self {
            child: None,
            output_readers: Vec::new(),
            recovery_attempts: 0,
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
    /// cannot be polled, the validated binary changed, or the executable cannot
    /// be started.
    pub fn start(&mut self, spec: &CoreProcessSpec) -> Result<CoreOutput, CoreRuntimeError> {
        self.recovery_attempts = 0;
        self.start_process(spec)
    }

    fn start_process(&mut self, spec: &CoreProcessSpec) -> Result<CoreOutput, CoreRuntimeError> {
        if self.poll()? == CoreState::Running {
            return Err(CoreRuntimeError::AlreadyRunning);
        }

        let binary = spec
            .binary
            .revalidate()
            .map_err(CoreRuntimeError::BinaryValidationFailed)?;
        let mut child = Command::new(binary.path())
            .args(&spec.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| CoreRuntimeError::SpawnFailed {
                executable: binary.path().to_path_buf(),
                source,
            })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            cleanup_started_child(&mut child, Vec::new());
            CoreRuntimeError::OutputPipeUnavailable(CoreOutputStream::Stdout)
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            cleanup_started_child(&mut child, Vec::new());
            CoreRuntimeError::OutputPipeUnavailable(CoreOutputStream::Stderr)
        })?;
        let (sender, output) = output::output_channel();
        let stdout_reader =
            output::spawn_output_reader(CoreOutputStream::Stdout, stdout, sender.clone()).map_err(
                |source| {
                    cleanup_started_child(&mut child, Vec::new());
                    CoreRuntimeError::OutputReaderSpawnFailed {
                        stream: CoreOutputStream::Stdout,
                        source,
                    }
                },
            )?;
        let stderr_reader =
            match output::spawn_output_reader(CoreOutputStream::Stderr, stderr, sender) {
                Ok(reader) => reader,
                Err(source) => {
                    cleanup_started_child(&mut child, vec![stdout_reader]);
                    return Err(CoreRuntimeError::OutputReaderSpawnFailed {
                        stream: CoreOutputStream::Stderr,
                        source,
                    });
                }
            };
        self.child = Some(child);
        self.output_readers = vec![stdout_reader, stderr_reader];
        self.state = CoreState::Running;

        Ok(output)
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
                self.join_output_readers()?;
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
                CoreState::Stopped | CoreState::Failed { .. } => {
                    return Err(CoreRuntimeError::NotRunning);
                }
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
        self.stop_process()?;
        self.recovery_attempts = 0;

        Ok(())
    }

    fn stop_process(&mut self) -> Result<(), CoreRuntimeError> {
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
        self.join_output_readers()?;

        Ok(())
    }

    /// Returns the operating-system ID of the currently retained child.
    #[must_use]
    pub fn process_id(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    fn join_output_readers(&mut self) -> Result<(), CoreRuntimeError> {
        let mut failed_stream = None;
        for reader in std::mem::take(&mut self.output_readers) {
            let result = reader.join();
            if failed_stream.is_none() {
                failed_stream = result.err();
            }
        }
        failed_stream.map_or(Ok(()), |stream| {
            Err(CoreRuntimeError::OutputReaderPanicked(stream))
        })
    }
}

impl Drop for CoreRuntime {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if let Err(error) = child.kill() {
                report_drop_error("terminate", child.id(), &error);
            }
            if let Err(error) = child.wait() {
                report_drop_error("reap", child.id(), &error);
            }
        }
        for reader in std::mem::take(&mut self.output_readers) {
            if let Err(stream) = reader.join() {
                eprintln!("Core {stream:?} reader panicked during cleanup");
            }
        }
    }
}

fn cleanup_started_child(child: &mut Child, readers: Vec<output::CoreOutputReader>) {
    if let Err(error) = child.kill() {
        report_drop_error("terminate after output setup failure", child.id(), &error);
    }
    if let Err(error) = child.wait() {
        report_drop_error("reap after output setup failure", child.id(), &error);
    }
    for reader in readers {
        if let Err(stream) = reader.join() {
            eprintln!("Core {stream:?} reader panicked during failed output setup cleanup");
        }
    }
}

fn report_drop_error(action: &str, process_id: u32, error: &io::Error) {
    eprintln!("failed to {action} Core process {process_id} during cleanup: {error}");
}
