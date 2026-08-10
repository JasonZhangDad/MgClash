//! Network-change and sleep/wake recovery for an active desktop session.
//!
//! PRD section 29 fixes the pipeline: a path change is debounced, the Core's
//! health is checked, and the session is reconnected *only when necessary* —
//! a minor path change must never restart a healthy Core. Recovery attempts are
//! bounded so a permanently broken network cannot restart the Core forever.

use std::error::Error;
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use magies_storage::SecretStore;
use thiserror::Error as ThisError;

use crate::{
    CoreSessionControl, DesktopSession, DesktopSessionError, DesktopSessionProfile,
    SystemProxySessionControl,
};

/// How many times one debounced event may restart the session before it gives
/// up and waits for the next event.
pub const MAX_RECOVERY_ATTEMPTS: u8 = 3;

/// The PRD's recommended debounce window is 500–1500 ms.
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_secs(1);

/// Why the session might need to be checked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkEvent {
    /// The default route, IP, or DNS configuration changed.
    PathChanged,
    /// The machine resumed from sleep.
    Woke,
}

/// What one recovery pass actually did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryOutcome {
    /// Nothing was owed: no event pending, the debounce has not elapsed, or no
    /// session is running.
    Idle,
    /// The Core still answered, so nothing was restarted.
    Healthy,
    /// The session was stopped and started again.
    Reconnected { attempts: u8 },
}

/// Answers whether the running Core still serves its local proxy port.
pub trait SessionHealthProbe {
    fn is_healthy(&self) -> bool;
}

/// The real probe: a short TCP connect to the Core's local proxy listener.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TcpHealthProbe {
    address: SocketAddr,
    timeout: Duration,
}

impl TcpHealthProbe {
    #[must_use]
    pub const fn new(address: SocketAddr, timeout: Duration) -> Self {
        Self { address, timeout }
    }
}

impl SessionHealthProbe for TcpHealthProbe {
    fn is_healthy(&self) -> bool {
        TcpStream::connect_timeout(&self.address, self.timeout).is_ok()
    }
}

/// Debounces network events and decides whether a session needs reconnecting.
#[derive(Clone, Debug)]
pub struct NetworkRecoveryPolicy {
    debounce: Duration,
    due_at: Option<Instant>,
    /// The profile of a session this policy took down and could not bring back.
    /// Retained so a later event can retry — waking with no network yet must
    /// not permanently kill the session. A user-requested stop never sets it,
    /// so recovery never resurrects a session the user disconnected.
    pending_restart: Option<DesktopSessionProfile>,
}

impl NetworkRecoveryPolicy {
    #[must_use]
    pub const fn new(debounce: Duration) -> Self {
        Self {
            debounce,
            due_at: None,
            pending_restart: None,
        }
    }

    /// Records an event. A second event inside the window pushes the deadline
    /// out so a burst of changes costs one recovery pass, not several.
    pub fn observe(&mut self, _event: NetworkEvent, now: Instant) {
        self.due_at = Some(now + self.debounce);
    }

    /// When the pending event becomes actionable, if one is pending.
    #[must_use]
    pub const fn due_at(&self) -> Option<Instant> {
        self.due_at
    }

    /// Runs one recovery pass for the debounced event, if it is due.
    ///
    /// Checks Core health first and restarts the session only when the probe
    /// fails. The pending event is consumed either way, so a failed burst never
    /// blocks the next event.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the session cannot be stopped, or when
    /// [`MAX_RECOVERY_ATTEMPTS`] restarts all failed.
    pub fn recover<S, C, P>(
        &mut self,
        now: Instant,
        session: &mut DesktopSession<S, C, P>,
        probe: &impl SessionHealthProbe,
    ) -> Result<RecoveryOutcome, RecoveryError<C::Error, P::Error>>
    where
        S: SecretStore,
        C: CoreSessionControl,
        P: SystemProxySessionControl,
    {
        let Some(due_at) = self.due_at else {
            return Ok(RecoveryOutcome::Idle);
        };
        if now < due_at {
            return Ok(RecoveryOutcome::Idle);
        }
        self.due_at = None;

        let profile = match session.active_profile().cloned() {
            Some(profile) => {
                if probe.is_healthy() {
                    self.pending_restart = None;
                    return Ok(RecoveryOutcome::Healthy);
                }
                session.stop().map_err(RecoveryError::Stop)?;
                profile
            }
            // Nothing is running. Only retry when an earlier recovery is what
            // took the session down.
            None => match self.pending_restart.clone() {
                Some(profile) => profile,
                None => return Ok(RecoveryOutcome::Idle),
            },
        };

        for attempt in 1..=MAX_RECOVERY_ATTEMPTS {
            match session.start(&profile) {
                Ok(_) => {
                    self.pending_restart = None;
                    return Ok(RecoveryOutcome::Reconnected { attempts: attempt });
                }
                Err(source) if attempt == MAX_RECOVERY_ATTEMPTS => {
                    self.pending_restart = Some(profile);
                    return Err(RecoveryError::AttemptsExhausted {
                        attempts: attempt,
                        source,
                    });
                }
                Err(_) => {}
            }
        }
        unreachable!("the loop returns on the final attempt")
    }
}

impl Default for NetworkRecoveryPolicy {
    fn default() -> Self {
        Self::new(DEFAULT_DEBOUNCE)
    }
}

#[derive(Debug, ThisError)]
pub enum RecoveryError<C, P>
where
    C: Error + 'static,
    P: Error + 'static,
{
    #[error("failed to stop the session before reconnecting")]
    Stop(#[source] DesktopSessionError<C, P>),
    #[error("failed to reconnect the session in {attempts} attempts")]
    AttemptsExhausted {
        attempts: u8,
        #[source]
        source: DesktopSessionError<C, P>,
    },
}
