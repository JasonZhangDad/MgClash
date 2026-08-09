use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::{CoreExit, CoreRuntime, CoreRuntimeError, CoreState};

const CONNECT_ATTEMPT_TIMEOUT: Duration = Duration::from_millis(25);
const RETRY_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreHealth {
    pub ready_after: Duration,
}

#[derive(Debug)]
pub enum CoreHealthError {
    NotRunning,
    RecoveryFailed {
        attempts: u8,
    },
    ProcessExited(CoreExit),
    Runtime(CoreRuntimeError),
    TimedOut {
        address: SocketAddr,
        timeout: Duration,
        last_error: Option<io::Error>,
    },
}

impl Display for CoreHealthError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRunning => formatter.write_str("cannot check health without a running Core"),
            Self::RecoveryFailed { attempts } => write!(
                formatter,
                "cannot check health after Core recovery failed in {attempts} attempts"
            ),
            Self::ProcessExited(exit) => write!(
                formatter,
                "Core exited before becoming healthy (success: {}, code: {:?})",
                exit.success, exit.code
            ),
            Self::Runtime(source) => write!(formatter, "failed to inspect Core health: {source}"),
            Self::TimedOut {
                address,
                timeout,
                last_error: Some(source),
            } => write!(
                formatter,
                "Core did not open {address} within {timeout:?}: {source}"
            ),
            Self::TimedOut {
                address,
                timeout,
                last_error: None,
            } => write!(formatter, "Core did not open {address} within {timeout:?}"),
        }
    }
}

impl Error for CoreHealthError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Runtime(source) => Some(source),
            Self::TimedOut {
                last_error: Some(source),
                ..
            } => Some(source),
            Self::NotRunning
            | Self::RecoveryFailed { .. }
            | Self::ProcessExited(_)
            | Self::TimedOut { .. } => None,
        }
    }
}

impl CoreRuntime {
    /// Waits until the running Core accepts a TCP connection at `address`.
    ///
    /// # Errors
    ///
    /// Returns a typed error when no Core is running, the Core exits, process
    /// polling fails, or the listener does not become ready before `timeout`.
    pub fn wait_for_tcp_health(
        &mut self,
        address: SocketAddr,
        timeout: Duration,
    ) -> Result<CoreHealth, CoreHealthError> {
        let started_at = Instant::now();
        let mut last_error = None;

        loop {
            match self.poll().map_err(CoreHealthError::Runtime)? {
                CoreState::Stopped => return Err(CoreHealthError::NotRunning),
                CoreState::Failed { attempts } => {
                    return Err(CoreHealthError::RecoveryFailed { attempts });
                }
                CoreState::Exited(exit) => return Err(CoreHealthError::ProcessExited(exit)),
                CoreState::Running => {}
            }

            let remaining = timeout.saturating_sub(started_at.elapsed());
            if remaining.is_zero() {
                return Err(CoreHealthError::TimedOut {
                    address,
                    timeout,
                    last_error,
                });
            }

            match TcpStream::connect_timeout(&address, CONNECT_ATTEMPT_TIMEOUT.min(remaining)) {
                Ok(_) => {
                    return Ok(CoreHealth {
                        ready_after: started_at.elapsed(),
                    });
                }
                Err(source) => last_error = Some(source),
            }

            let remaining = timeout.saturating_sub(started_at.elapsed());
            sleep(RETRY_INTERVAL.min(remaining));
        }
    }
}
