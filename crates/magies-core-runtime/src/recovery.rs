use std::error::Error;
use std::fmt::{Display, Formatter};
use std::net::SocketAddr;
use std::time::Duration;

use crate::{
    CoreHealth, CoreHealthError, CoreOutput, CoreProcessSpec, CoreRuntime, CoreRuntimeError,
    CoreState,
};

pub const MAX_CRASH_RECOVERY_ATTEMPTS: u8 = 3;

#[derive(Debug)]
pub struct CoreRecovery {
    pub attempts: u8,
    pub health: CoreHealth,
    pub output: CoreOutput,
}

#[derive(Debug)]
pub enum CoreRecoveryFailure {
    Start(CoreRuntimeError),
    Health(CoreHealthError),
}

impl Display for CoreRecoveryFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start(source) => write!(formatter, "Core recovery start failed: {source}"),
            Self::Health(source) => write!(formatter, "restarted Core is unhealthy: {source}"),
        }
    }
}

impl Error for CoreRecoveryFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Start(source) => Some(source),
            Self::Health(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub enum CoreRecoveryError {
    NotCrashed(CoreState),
    Runtime(CoreRuntimeError),
    CleanupFailed {
        attempt: u8,
        source: CoreRuntimeError,
    },
    AttemptsExhausted {
        attempts: u8,
        last_failure: CoreRecoveryFailure,
    },
    RetryLimitReached {
        attempts: u8,
    },
}

impl Display for CoreRecoveryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotCrashed(state) => {
                write!(
                    formatter,
                    "Core recovery requires an exited process, got {state:?}"
                )
            }
            Self::Runtime(source) => write!(formatter, "failed to inspect crashed Core: {source}"),
            Self::CleanupFailed { attempt, source } => write!(
                formatter,
                "failed to clean up unhealthy Core recovery attempt {attempt}: {source}"
            ),
            Self::AttemptsExhausted {
                attempts,
                last_failure,
            } => write!(
                formatter,
                "Core recovery failed after {attempts} attempts: {last_failure}"
            ),
            Self::RetryLimitReached { attempts } => write!(
                formatter,
                "Core recovery retry limit already reached after {attempts} attempts"
            ),
        }
    }
}

impl Error for CoreRecoveryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Runtime(source) | Self::CleanupFailed { source, .. } => Some(source),
            Self::AttemptsExhausted { last_failure, .. } => Some(last_failure),
            Self::NotCrashed(_) | Self::RetryLimitReached { .. } => None,
        }
    }
}

impl CoreRuntime {
    /// Restarts a Core that exited unexpectedly and waits for TCP readiness.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the Core did not crash, process inspection or
    /// cleanup fails, or three consecutive recovery attempts fail.
    pub fn recover_after_crash(
        &mut self,
        spec: &CoreProcessSpec,
        health_address: SocketAddr,
        health_timeout: Duration,
    ) -> Result<CoreRecovery, CoreRecoveryError> {
        match self.poll().map_err(CoreRecoveryError::Runtime)? {
            CoreState::Exited(_) => {}
            CoreState::Failed { attempts } => {
                return Err(CoreRecoveryError::RetryLimitReached { attempts });
            }
            state => return Err(CoreRecoveryError::NotCrashed(state)),
        }

        if self.recovery_attempts >= MAX_CRASH_RECOVERY_ATTEMPTS {
            return Err(CoreRecoveryError::RetryLimitReached {
                attempts: self.recovery_attempts,
            });
        }

        let last_failure = loop {
            self.recovery_attempts += 1;
            let attempt = self.recovery_attempts;
            let output = match self.start_process(spec) {
                Ok(output) => output,
                Err(source) => {
                    let failure = CoreRecoveryFailure::Start(source);
                    if attempt >= MAX_CRASH_RECOVERY_ATTEMPTS {
                        break failure;
                    }
                    continue;
                }
            };

            match self.wait_for_tcp_health(health_address, health_timeout) {
                Ok(health) => {
                    self.recovery_attempts = 0;
                    return Ok(CoreRecovery {
                        attempts: attempt,
                        health,
                        output,
                    });
                }
                Err(source) => {
                    let failure = CoreRecoveryFailure::Health(source);
                    drop(output);
                    self.cleanup_recovery_attempt(attempt)?;
                    if attempt >= MAX_CRASH_RECOVERY_ATTEMPTS {
                        break failure;
                    }
                }
            }
        };

        self.state = CoreState::Failed {
            attempts: self.recovery_attempts,
        };
        Err(CoreRecoveryError::AttemptsExhausted {
            attempts: self.recovery_attempts,
            last_failure,
        })
    }

    fn cleanup_recovery_attempt(&mut self, attempt: u8) -> Result<(), CoreRecoveryError> {
        match self
            .poll()
            .map_err(|source| CoreRecoveryError::CleanupFailed { attempt, source })?
        {
            CoreState::Running => {
                self.stop_process()
                    .map_err(|source| CoreRecoveryError::CleanupFailed { attempt, source })?;
            }
            CoreState::Stopped | CoreState::Exited(_) | CoreState::Failed { .. } => {}
        }

        Ok(())
    }
}
