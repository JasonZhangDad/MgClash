//! Covers the persisted application settings: defaults on a fresh install,
//! round-tripping across restarts, and rejecting a corrupt stored level.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::id;
use std::sync::atomic::{AtomicUsize, Ordering};

use magies_desktop_lib::app_settings::{
    AppSettings, AppSettingsStoreError, CorePreferenceSetting, SqliteAppSettingsStore,
    log_level_name, parse_core_preference, parse_log_level,
};
use magies_desktop_lib::logs::LogLevel;

#[test]
fn a_fresh_install_uses_the_documented_defaults() {
    let store = SqliteAppSettingsStore::open_in_memory().unwrap();

    let settings = store.load().unwrap();

    assert_eq!(settings, AppSettings::default());
    // Connecting is an explicit action, so the app never starts a proxy on its
    // own unless the user asks for it.
    assert!(!settings.connect_on_launch);
    assert!(settings.close_to_tray);
    // Registering a login item without being asked is equally intrusive.
    assert!(!settings.launch_at_login);
    assert_eq!(settings.log_level, LogLevel::Info);
}

#[test]
fn saved_settings_survive_a_restart() {
    let database = TestDatabase::new("app-settings");
    let saved = AppSettings {
        connect_on_launch: true,
        close_to_tray: false,
        launch_at_login: true,
        core_preference: CorePreferenceSetting::Xray,
        log_level: LogLevel::Debug,
    };

    {
        let store = SqliteAppSettingsStore::open(database.path()).unwrap();
        store.save(saved).unwrap();
    }

    let store = SqliteAppSettingsStore::open(database.path()).unwrap();
    assert_eq!(store.load().unwrap(), saved);
}

#[test]
fn saving_twice_replaces_the_single_row() {
    let store = SqliteAppSettingsStore::open_in_memory().unwrap();

    store
        .save(AppSettings {
            connect_on_launch: true,
            ..AppSettings::default()
        })
        .unwrap();
    store
        .save(AppSettings {
            log_level: LogLevel::Error,
            ..AppSettings::default()
        })
        .unwrap();

    let settings = store.load().unwrap();
    assert!(!settings.connect_on_launch);
    assert_eq!(settings.log_level, LogLevel::Error);
}

#[test]
fn every_level_round_trips_through_storage() {
    let store = SqliteAppSettingsStore::open_in_memory().unwrap();

    for level in [
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
        LogLevel::Trace,
    ] {
        store
            .save(AppSettings {
                log_level: level,
                ..AppSettings::default()
            })
            .unwrap();

        assert_eq!(store.load().unwrap().log_level, level);
    }
}

#[test]
fn a_corrupt_stored_level_is_a_typed_error() {
    let database = TestDatabase::new("app-settings-corrupt");
    {
        let store = SqliteAppSettingsStore::open(database.path()).unwrap();
        store.save(AppSettings::default()).unwrap();
    }
    let connection = rusqlite::Connection::open(database.path()).unwrap();
    connection
        .execute(
            "UPDATE app_settings SET log_level = 'verbose' WHERE id = 1",
            [],
        )
        .unwrap();
    drop(connection);

    let store = SqliteAppSettingsStore::open(database.path()).unwrap();

    assert!(matches!(
        store.load().unwrap_err(),
        AppSettingsStoreError::InvalidStoredValue { value } if value == "verbose"
    ));
}

#[test]
fn the_default_core_preference_is_auto() {
    let store = SqliteAppSettingsStore::open_in_memory().unwrap();

    assert_eq!(
        store.load().unwrap().core_preference,
        CorePreferenceSetting::Auto
    );
}

#[test]
fn every_core_preference_round_trips_through_storage() {
    let store = SqliteAppSettingsStore::open_in_memory().unwrap();

    for preference in [
        CorePreferenceSetting::Auto,
        CorePreferenceSetting::SingBox,
        CorePreferenceSetting::Xray,
    ] {
        store
            .save(AppSettings {
                core_preference: preference,
                ..AppSettings::default()
            })
            .unwrap();

        assert_eq!(store.load().unwrap().core_preference, preference);
    }
}

#[test]
fn a_corrupt_stored_core_preference_is_a_typed_error() {
    let database = TestDatabase::new("app-settings-core");
    {
        let store = SqliteAppSettingsStore::open(database.path()).unwrap();
        store.save(AppSettings::default()).unwrap();
    }
    let connection = rusqlite::Connection::open(database.path()).unwrap();
    connection
        .execute(
            "UPDATE app_settings SET core_preference = 'clash' WHERE id = 1",
            [],
        )
        .unwrap();
    drop(connection);

    let store = SqliteAppSettingsStore::open(database.path()).unwrap();

    assert!(matches!(
        store.load().unwrap_err(),
        AppSettingsStoreError::InvalidStoredValue { value } if value == "clash"
    ));
}

#[test]
fn core_preference_names_are_stable_in_both_directions() {
    for preference in [
        CorePreferenceSetting::Auto,
        CorePreferenceSetting::SingBox,
        CorePreferenceSetting::Xray,
    ] {
        assert_eq!(parse_core_preference(preference.name()), Some(preference));
    }
    assert_eq!(parse_core_preference("clash"), None);
}

#[test]
fn level_names_are_stable_in_both_directions() {
    for level in [
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
        LogLevel::Trace,
    ] {
        assert_eq!(parse_log_level(log_level_name(level)), Some(level));
    }
    assert_eq!(parse_log_level("verbose"), None);
}

/// A uniquely named database file that removes itself.
struct TestDatabase {
    path: PathBuf,
}

impl TestDatabase {
    fn new(name: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mgclash-{name}-{}-{unique}.sqlite", id()));
        let _ = fs::remove_file(&path);
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
