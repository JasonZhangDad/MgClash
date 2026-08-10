use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::fs::{self, OpenOptions};
use std::io::{self, BufReader, BufWriter, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::system_proxy::SystemProxyState;

/// Platform operations required by the System Proxy recovery state machine.
pub trait SystemProxyControl {
    type Error: Error;
    type Snapshot: Clone;

    /// Captures the exact settings needed for restoration.
    ///
    /// # Errors
    ///
    /// Returns the platform adapter's read error.
    fn capture(&self) -> Result<Self::Snapshot, Self::Error>;

    /// Reads the current settings into the shared state model.
    ///
    /// # Errors
    ///
    /// Returns the platform adapter's read or mapping error.
    fn current(&self) -> Result<SystemProxyState, Self::Error>;

    /// Applies the settings managed by `MgClash`.
    ///
    /// # Errors
    ///
    /// Returns the platform adapter's write error.
    fn apply(&self, state: &SystemProxyState) -> Result<(), Self::Error>;

    /// Restores a previously captured exact snapshot.
    ///
    /// # Errors
    ///
    /// Returns the platform adapter's write error.
    fn restore(&self, snapshot: &Self::Snapshot) -> Result<(), Self::Error>;
}

/// Persistence boundary for a crash-recoverable System Proxy session.
pub trait RecoveryStore<S> {
    type Error: Error;

    /// Loads the active recovery record, if one exists.
    ///
    /// # Errors
    ///
    /// Returns an error when the record cannot be read or decoded.
    fn load(&self) -> Result<Option<RecoveryRecord<S>>, Self::Error>;

    /// Persists the record before System Proxy is changed.
    ///
    /// # Errors
    ///
    /// Returns an error when the record cannot be encoded or written.
    fn save(&self, record: &RecoveryRecord<S>) -> Result<(), Self::Error>;

    /// Removes the active recovery record.
    ///
    /// # Errors
    ///
    /// Returns an error when an existing record cannot be removed.
    fn clear(&self) -> Result<(), Self::Error>;
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecoveryRecord<S> {
    snapshot: S,
    managed_state: SystemProxyState,
}

impl<S> RecoveryRecord<S> {
    #[must_use]
    pub const fn new(snapshot: S, managed_state: SystemProxyState) -> Self {
        Self {
            snapshot,
            managed_state,
        }
    }

    #[must_use]
    pub const fn snapshot(&self) -> &S {
        &self.snapshot
    }

    #[must_use]
    pub const fn managed_state(&self) -> &SystemProxyState {
        &self.managed_state
    }
}

impl<S> Debug for RecoveryRecord<S> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecoveryRecord")
            .field("snapshot", &"[REDACTED]")
            .field("managed_state", &"configured")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupRecovery {
    Clean,
    ManagedCoreRunning,
    RestoreRequired,
}

pub struct SystemProxyRecoveryManager<C, S> {
    control: C,
    store: S,
}

impl<C, S> SystemProxyRecoveryManager<C, S> {
    #[must_use]
    pub const fn new(control: C, store: S) -> Self {
        Self { control, store }
    }
}

impl<C, S> SystemProxyRecoveryManager<C, S>
where
    C: SystemProxyControl,
    S: RecoveryStore<C::Snapshot>,
{
    /// Saves the original snapshot before applying the managed proxy state.
    ///
    /// # Errors
    ///
    /// Returns a typed control or persistence error. An apply failure is rolled
    /// back immediately; a failed rollback leaves the record for next startup.
    pub fn enable(
        &self,
        managed_state: &SystemProxyState,
    ) -> Result<(), SystemProxyRecoveryError<C::Error, S::Error>> {
        let snapshot = self
            .control
            .capture()
            .map_err(|source| control_error(ControlOperation::Capture, source))?;
        let record = RecoveryRecord::new(snapshot, managed_state.clone());
        self.store
            .save(&record)
            .map_err(|source| store_error(StoreOperation::Save, source))?;

        if let Err(apply) = self.control.apply(managed_state) {
            if let Err(rollback) = self.control.restore(record.snapshot()) {
                return Err(SystemProxyRecoveryError::ApplyAndRollback { apply, rollback });
            }
            self.store
                .clear()
                .map_err(|source| store_error(StoreOperation::Clear, source))?;
            return Err(control_error(ControlOperation::Apply, apply));
        }
        Ok(())
    }

    /// Restores an active session and removes its record after success.
    ///
    /// # Errors
    ///
    /// Returns a typed read, restore, or clear error. Restore failures retain
    /// the record so a later startup can retry.
    pub fn stop(&self) -> Result<(), SystemProxyRecoveryError<C::Error, S::Error>> {
        self.restore_record()
    }

    /// Applies the PRD's three-condition abnormal-exit check.
    ///
    /// A stale record is cleared when the current System Proxy no longer equals
    /// the state `MgClash` saved as managed.
    ///
    /// # Errors
    ///
    /// Returns a typed store or platform read error.
    pub fn inspect_startup(
        &self,
        core_running: bool,
    ) -> Result<StartupRecovery, SystemProxyRecoveryError<C::Error, S::Error>> {
        let Some(record) = self
            .store
            .load()
            .map_err(|source| store_error(StoreOperation::Load, source))?
        else {
            return Ok(StartupRecovery::Clean);
        };
        let current = self
            .control
            .current()
            .map_err(|source| control_error(ControlOperation::ReadCurrent, source))?;
        if current != *record.managed_state() {
            self.store
                .clear()
                .map_err(|source| store_error(StoreOperation::Clear, source))?;
            return Ok(StartupRecovery::Clean);
        }
        if core_running {
            Ok(StartupRecovery::ManagedCoreRunning)
        } else {
            Ok(StartupRecovery::RestoreRequired)
        }
    }

    /// Confirms abnormal-exit recovery.
    ///
    /// # Errors
    ///
    /// Returns a typed read, restore, or clear error.
    pub fn recover(&self) -> Result<(), SystemProxyRecoveryError<C::Error, S::Error>> {
        self.restore_record()
    }

    /// Declines abnormal-exit recovery and forgets the saved snapshot.
    ///
    /// # Errors
    ///
    /// Returns a typed persistence error.
    pub fn dismiss(&self) -> Result<(), SystemProxyRecoveryError<C::Error, S::Error>> {
        self.store
            .clear()
            .map_err(|source| store_error(StoreOperation::Clear, source))
    }

    fn restore_record(&self) -> Result<(), SystemProxyRecoveryError<C::Error, S::Error>> {
        let Some(record) = self
            .store
            .load()
            .map_err(|source| store_error(StoreOperation::Load, source))?
        else {
            return Ok(());
        };
        self.control
            .restore(record.snapshot())
            .map_err(|source| control_error(ControlOperation::Restore, source))?;
        self.store
            .clear()
            .map_err(|source| store_error(StoreOperation::Clear, source))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlOperation {
    Capture,
    ReadCurrent,
    Apply,
    Restore,
}

impl Display for ControlOperation {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Capture => "capture System Proxy snapshot",
            Self::ReadCurrent => "read current System Proxy",
            Self::Apply => "apply managed System Proxy",
            Self::Restore => "restore System Proxy snapshot",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreOperation {
    Load,
    Save,
    Clear,
}

impl Display for StoreOperation {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Load => "load System Proxy recovery record",
            Self::Save => "save System Proxy recovery record",
            Self::Clear => "clear System Proxy recovery record",
        })
    }
}

#[derive(Debug)]
pub enum SystemProxyRecoveryError<C, S> {
    Control {
        operation: ControlOperation,
        source: C,
    },
    Store {
        operation: StoreOperation,
        source: S,
    },
    ApplyAndRollback {
        apply: C,
        rollback: C,
    },
}

impl<C: Display, S: Display> Display for SystemProxyRecoveryError<C, S> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Control { operation, source } => {
                write!(formatter, "failed to {operation}: {source}")
            }
            Self::Store { operation, source } => {
                write!(formatter, "failed to {operation}: {source}")
            }
            Self::ApplyAndRollback { apply, rollback } => write!(
                formatter,
                "failed to apply managed System Proxy ({apply}) and rollback ({rollback})"
            ),
        }
    }
}

impl<C: Error + 'static, S: Error + 'static> Error for SystemProxyRecoveryError<C, S> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Control { source, .. } => Some(source),
            Self::Store { source, .. } => Some(source),
            Self::ApplyAndRollback { rollback, .. } => Some(rollback),
        }
    }
}

fn control_error<C, S>(operation: ControlOperation, source: C) -> SystemProxyRecoveryError<C, S> {
    SystemProxyRecoveryError::Control { operation, source }
}

fn store_error<C, S>(operation: StoreOperation, source: S) -> SystemProxyRecoveryError<C, S> {
    SystemProxyRecoveryError::Store { operation, source }
}

pub struct JsonRecoveryStore<S> {
    path: PathBuf,
    snapshot: PhantomData<fn() -> S>,
}

impl<S> JsonRecoveryStore<S> {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            snapshot: PhantomData,
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl<S> RecoveryStore<S> for JsonRecoveryStore<S>
where
    S: Clone + DeserializeOwned + Serialize,
{
    type Error = JsonRecoveryStoreError;

    fn load(&self) -> Result<Option<RecoveryRecord<S>>, Self::Error> {
        let file = match fs::File::open(&self.path) {
            Ok(file) => file,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(JsonRecoveryStoreError::Open { source }),
        };
        serde_json::from_reader(BufReader::new(file))
            .map(Some)
            .map_err(|source| JsonRecoveryStoreError::Deserialize { source })
    }

    fn save(&self, record: &RecoveryRecord<S>) -> Result<(), Self::Error> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|source| JsonRecoveryStoreError::CreateDirectory { source })?;
        }
        let file = restricted_file(&self.path)
            .map_err(|source| JsonRecoveryStoreError::OpenForWrite { source })?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, record)
            .map_err(|source| JsonRecoveryStoreError::Serialize { source })?;
        writer
            .flush()
            .map_err(|source| JsonRecoveryStoreError::Flush { source })?;
        writer
            .get_ref()
            .sync_all()
            .map_err(|source| JsonRecoveryStoreError::Sync { source })
    }

    fn clear(&self) -> Result<(), Self::Error> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(JsonRecoveryStoreError::Remove { source }),
        }
    }
}

fn restricted_file(path: &Path) -> io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.mode(0o600);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

#[derive(Debug)]
pub enum JsonRecoveryStoreError {
    CreateDirectory { source: io::Error },
    Open { source: io::Error },
    OpenForWrite { source: io::Error },
    Serialize { source: serde_json::Error },
    Flush { source: io::Error },
    Sync { source: io::Error },
    Deserialize { source: serde_json::Error },
    Remove { source: io::Error },
}

impl Display for JsonRecoveryStoreError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::CreateDirectory { .. } => "failed to create recovery record directory",
            Self::Open { .. } => "failed to open recovery record",
            Self::OpenForWrite { .. } => "failed to open recovery record for writing",
            Self::Serialize { .. } => "failed to serialize recovery record",
            Self::Flush { .. } => "failed to flush recovery record",
            Self::Sync { .. } => "failed to sync recovery record",
            Self::Deserialize { .. } => "failed to deserialize recovery record",
            Self::Remove { .. } => "failed to remove recovery record",
        })
    }
}

impl Error for JsonRecoveryStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::CreateDirectory { source }
            | Self::Open { source }
            | Self::OpenForWrite { source }
            | Self::Flush { source }
            | Self::Sync { source }
            | Self::Remove { source } => source,
            Self::Serialize { source } | Self::Deserialize { source } => source,
        })
    }
}
