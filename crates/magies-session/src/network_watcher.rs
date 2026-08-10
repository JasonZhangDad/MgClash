//! Portable detection of the two events [`NetworkEvent`] describes.
//!
//! `NWPathMonitor` is macOS-only and needs FFI, which this workspace forbids
//! (`unsafe_code = "forbid"`). Instead the driver ticks this watcher on a timer
//! and hands it an opaque fingerprint of the host's current network path — a
//! string each platform produces however it likes. The watcher compares it with
//! the previous tick, and separately notices when far more wall-clock time
//! passed than the tick interval, which is what a sleep/wake cycle looks like
//! from a timer that was frozen along with the machine.

use std::time::{Duration, SystemTime};

use crate::NetworkEvent;

/// Detects path changes and sleep/wake from periodic ticks.
#[derive(Clone, Debug)]
pub struct NetworkWatcher {
    tick: Duration,
    sleep_threshold: Duration,
    last_tick: Option<SystemTime>,
    last_fingerprint: Option<String>,
}

impl NetworkWatcher {
    /// `sleep_threshold` is the wall-clock gap between two ticks that counts as
    /// a wake; it must be comfortably larger than `tick` so ordinary scheduling
    /// jitter is not mistaken for sleep.
    #[must_use]
    pub const fn new(tick: Duration, sleep_threshold: Duration) -> Self {
        Self {
            tick,
            sleep_threshold,
            last_tick: None,
            last_fingerprint: None,
        }
    }

    /// How long the driver should wait between [`Self::tick`] calls.
    #[must_use]
    pub const fn tick_interval(&self) -> Duration {
        self.tick
    }

    /// Feeds one tick and returns the event it implies, if any.
    ///
    /// `fingerprint` is `None` when the platform could not read the current
    /// path; that is treated as "no new information" rather than as a change,
    /// so a flaky read never restarts a healthy Core.
    pub fn tick(&mut self, now: SystemTime, fingerprint: Option<&str>) -> Option<NetworkEvent> {
        let woke = self
            .last_tick
            .is_some_and(|last| self.is_sleep_gap(last, now));
        self.last_tick = Some(now);

        let changed = match (fingerprint, self.last_fingerprint.as_deref()) {
            (Some(current), Some(previous)) => current != previous,
            // The first successful read only establishes the baseline.
            (Some(_), None) | (None, _) => false,
        };
        if let Some(current) = fingerprint {
            self.last_fingerprint = Some(current.to_owned());
        }

        if woke {
            Some(NetworkEvent::Woke)
        } else if changed {
            Some(NetworkEvent::PathChanged)
        } else {
            None
        }
    }

    /// A gap at or beyond the threshold means the timer stopped tracking real
    /// time. A clock that moved backwards means the same thing: either way the
    /// network has to be re-checked.
    fn is_sleep_gap(&self, last: SystemTime, now: SystemTime) -> bool {
        now.duration_since(last)
            .map_or(true, |gap| gap >= self.sleep_threshold)
    }
}
