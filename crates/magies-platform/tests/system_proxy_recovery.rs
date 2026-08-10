use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use magies_platform::system_proxy::{PacSetting, ProxyEndpoint, ProxySetting, SystemProxyState};
use magies_platform::system_proxy_recovery::{
    JsonRecoveryStore, RecoveryStore, StartupRecovery, SystemProxyControl,
    SystemProxyRecoveryError, SystemProxyRecoveryManager,
};

#[test]
fn enable_saves_the_original_snapshot_before_applying_managed_state() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let control = FakeControl::new(events.clone());
    let store = FakeStore::new(events.clone());
    let manager = SystemProxyRecoveryManager::new(control, store);

    manager.enable(&managed_state()).unwrap();

    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["capture", "save", "apply"]
    );
}

#[test]
fn enable_does_not_touch_system_proxy_when_snapshot_persistence_fails() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let control = FakeControl::new(events.clone());
    let mut store = FakeStore::new(events.clone());
    store.fail_save = true;
    let manager = SystemProxyRecoveryManager::new(control, store);

    assert!(matches!(
        manager.enable(&managed_state()),
        Err(SystemProxyRecoveryError::Store { .. })
    ));
    assert_eq!(events.lock().unwrap().as_slice(), ["capture", "save"]);
}

#[test]
fn failed_apply_rolls_back_and_clears_the_recovery_record() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut control = FakeControl::new(events.clone());
    control.fail_apply = true;
    let store = FakeStore::new(events.clone());
    let manager = SystemProxyRecoveryManager::new(control, store.clone());

    assert!(matches!(
        manager.enable(&managed_state()),
        Err(SystemProxyRecoveryError::Control { .. })
    ));
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["capture", "save", "apply", "restore", "clear"]
    );
    assert!(store.record.lock().unwrap().is_none());
}

#[test]
fn failed_apply_and_rollback_preserve_the_record_for_next_startup() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut control = FakeControl::new(events.clone());
    control.fail_apply = true;
    control.fail_restore = true;
    let store = FakeStore::new(events.clone());
    let manager = SystemProxyRecoveryManager::new(control, store.clone());

    assert!(matches!(
        manager.enable(&managed_state()),
        Err(SystemProxyRecoveryError::ApplyAndRollback { .. })
    ));
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["capture", "save", "apply", "restore"]
    );
    assert!(store.record.lock().unwrap().is_some());
}

#[test]
fn stop_restores_before_clearing_and_keeps_record_when_restore_fails() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut control = FakeControl::new(events.clone());
    control.fail_restore = true;
    let store = FakeStore::new(events.clone());
    store.seed("original", managed_state());
    let manager = SystemProxyRecoveryManager::new(control, store.clone());

    assert!(matches!(
        manager.stop(),
        Err(SystemProxyRecoveryError::Control { .. })
    ));
    assert_eq!(events.lock().unwrap().as_slice(), ["load", "restore"]);
    assert!(store.record.lock().unwrap().is_some());
}

#[test]
fn startup_only_prompts_for_managed_proxy_without_a_running_core() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let control = FakeControl::new(events.clone());
    control.set_current(managed_state());
    let store = FakeStore::new(events);
    store.seed("original", managed_state());
    let manager = SystemProxyRecoveryManager::new(control, store);

    assert_eq!(
        manager.inspect_startup(false).unwrap(),
        StartupRecovery::RestoreRequired
    );
    assert_eq!(
        manager.inspect_startup(true).unwrap(),
        StartupRecovery::ManagedCoreRunning
    );
}

#[test]
fn startup_discards_stale_record_when_proxy_no_longer_matches() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let control = FakeControl::new(events.clone());
    control.set_current(SystemProxyState::new(
        ProxySetting::disabled(),
        ProxySetting::disabled(),
        ProxySetting::disabled(),
        PacSetting::disabled(),
    ));
    let store = FakeStore::new(events.clone());
    store.seed("original", managed_state());
    let manager = SystemProxyRecoveryManager::new(control, store.clone());

    assert_eq!(
        manager.inspect_startup(false).unwrap(),
        StartupRecovery::Clean
    );
    assert!(store.record.lock().unwrap().is_none());
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["load", "current", "clear"]
    );
}

#[test]
fn confirmed_recovery_restores_snapshot_and_decline_only_clears_record() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let control = FakeControl::new(events.clone());
    let store = FakeStore::new(events.clone());
    store.seed("original", managed_state());
    let manager = SystemProxyRecoveryManager::new(control, store.clone());

    manager.recover().unwrap();
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["load", "restore", "clear"]
    );

    store.seed("original", managed_state());
    events.lock().unwrap().clear();
    manager.dismiss().unwrap();
    assert_eq!(events.lock().unwrap().as_slice(), ["clear"]);
}

#[test]
fn json_store_round_trips_and_redacts_snapshot_debug_output() {
    let path = recovery_path("roundtrip");
    let store = JsonRecoveryStore::<String>::new(path.clone());
    let record = magies_platform::system_proxy_recovery::RecoveryRecord::new(
        "pac-secret".to_owned(),
        managed_state(),
    );

    store.save(&record).unwrap();
    assert_eq!(store.load().unwrap(), Some(record.clone()));
    assert!(!format!("{record:?}").contains("pac-secret"));
    store.clear().unwrap();
    assert_eq!(store.load().unwrap(), None);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        store.save(&record).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        store.clear().unwrap();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FakeError(&'static str);

impl Display for FakeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for FakeError {}

#[derive(Clone)]
struct FakeControl {
    events: Arc<Mutex<Vec<&'static str>>>,
    current: Arc<Mutex<SystemProxyState>>,
    fail_apply: bool,
    fail_restore: bool,
}

impl FakeControl {
    fn new(events: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self {
            events,
            current: Arc::new(Mutex::new(managed_state())),
            fail_apply: false,
            fail_restore: false,
        }
    }

    fn set_current(&self, state: SystemProxyState) {
        *self.current.lock().unwrap() = state;
    }
}

impl SystemProxyControl for FakeControl {
    type Error = FakeError;
    type Snapshot = String;

    fn capture(&self) -> Result<Self::Snapshot, Self::Error> {
        self.events.lock().unwrap().push("capture");
        Ok("original".to_owned())
    }

    fn current(&self) -> Result<SystemProxyState, Self::Error> {
        self.events.lock().unwrap().push("current");
        Ok(self.current.lock().unwrap().clone())
    }

    fn apply(&self, _state: &SystemProxyState) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push("apply");
        if self.fail_apply {
            Err(FakeError("apply"))
        } else {
            Ok(())
        }
    }

    fn restore(&self, _snapshot: &Self::Snapshot) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push("restore");
        if self.fail_restore {
            Err(FakeError("restore"))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone)]
struct FakeStore {
    events: Arc<Mutex<Vec<&'static str>>>,
    record: Arc<Mutex<Option<magies_platform::system_proxy_recovery::RecoveryRecord<String>>>>,
    fail_save: bool,
}

impl FakeStore {
    fn new(events: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self {
            events,
            record: Arc::new(Mutex::new(None)),
            fail_save: false,
        }
    }

    fn seed(&self, snapshot: &str, state: SystemProxyState) {
        *self.record.lock().unwrap() = Some(
            magies_platform::system_proxy_recovery::RecoveryRecord::new(snapshot.to_owned(), state),
        );
    }
}

impl RecoveryStore<String> for FakeStore {
    type Error = FakeError;

    fn load(
        &self,
    ) -> Result<Option<magies_platform::system_proxy_recovery::RecoveryRecord<String>>, Self::Error>
    {
        self.events.lock().unwrap().push("load");
        Ok(self.record.lock().unwrap().clone())
    }

    fn save(
        &self,
        record: &magies_platform::system_proxy_recovery::RecoveryRecord<String>,
    ) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push("save");
        if self.fail_save {
            return Err(FakeError("save"));
        }
        *self.record.lock().unwrap() = Some(record.clone());
        Ok(())
    }

    fn clear(&self) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push("clear");
        *self.record.lock().unwrap() = None;
        Ok(())
    }
}

fn managed_state() -> SystemProxyState {
    SystemProxyState::new(
        enabled("127.0.0.1", 10809),
        enabled("127.0.0.1", 10809),
        enabled("127.0.0.1", 10808),
        PacSetting::disabled(),
    )
}

fn enabled(host: &str, port: u16) -> ProxySetting {
    ProxySetting::new(true, Some(ProxyEndpoint::new(host, port).unwrap())).unwrap()
}

fn recovery_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "magies-system-proxy-recovery-{name}-{}-{}.json",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ))
}
