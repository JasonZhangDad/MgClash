//! Covers the persisted application settings: defaults on a fresh install,
//! round-tripping across restarts, and rejecting a corrupt stored level.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::id;
use std::sync::atomic::{AtomicUsize, Ordering};

use magies_desktop_lib::app_settings::{
    AppSettings, AppSettingsStoreError, CorePreferenceSetting, LocaleSetting,
    SqliteAppSettingsStore, SystemProxyModeSetting, log_level_name, parse_core_preference,
    parse_locale, parse_log_level, parse_system_proxy_mode,
};
use magies_desktop_lib::logs::LogLevel;
use magies_session::SystemProxyMode;

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
    // TUN needs elevation and takes over routing, so it is never on by default.
    assert!(!settings.tun_enabled);
    assert_eq!(settings.log_level, LogLevel::Info);
    assert_eq!(settings.socks_port, 10_808);
    assert_eq!(settings.http_port, 10_809);
    assert_eq!(settings.clash_api_port, 9_090);
    assert!(!settings.mux_enabled);
    assert!(!settings.auto_select_lowest_latency);
    assert_eq!(settings.url_test_address, "https://www.gstatic.com/generate_204");
    assert!(!settings.allow_lan);
    assert_eq!(
        settings.speed_test_url,
        "https://speed.cloudflare.com/__down?bytes=10000000"
    );
    assert!(settings.inbound_udp_enabled);
    assert!(!settings.def_allow_insecure);
    assert_eq!(settings.def_fingerprint, "");
    assert_eq!(settings.hotkey_connect, "Ctrl+Enter");
    assert_eq!(settings.hotkey_previous, "Ctrl+[");
    assert_eq!(settings.hotkey_next, "Ctrl+]");
    // Fragment changes the traffic shape, so it stays off until the user asks.
    assert!(!settings.fragment_enabled);
    assert!(!settings.udp_noise_enabled);
}

#[test]
fn saved_settings_survive_a_restart() {
    let database = TestDatabase::new("app-settings");
    let saved = AppSettings {
        connect_on_launch: true,
        close_to_tray: false,
        launch_at_login: true,
        core_preference: CorePreferenceSetting::Xray,
        tun_enabled: true,
        log_level: LogLevel::Debug,
        system_proxy_mode: SystemProxyModeSetting::Cleared,
        locale: LocaleSetting::Japanese,
        socks_port: 20_808,
        http_port: 20_809,
        clash_api_port: 19_090,
        mux_enabled: true,
        auto_select_lowest_latency: true,
        url_test_address: "https://www.google.com/generate_204".to_owned(),
        allow_lan: true,
        speed_test_url: "https://speed.cloudflare.com/__down?bytes=5000000".to_owned(),
        inbound_udp_enabled: false,
        def_allow_insecure: true,
        def_fingerprint: "chrome".to_owned(),
        hotkey_connect: "Ctrl+T".to_owned(),
        hotkey_previous: "Alt+[".to_owned(),
        hotkey_next: "Alt+]".to_owned(),
        fragment_enabled: true,
        udp_noise_enabled: true,
    };

    {
        let store = SqliteAppSettingsStore::open(database.path()).unwrap();
        store.save(&saved).unwrap();
    }

    let store = SqliteAppSettingsStore::open(database.path()).unwrap();
    assert_eq!(store.load().unwrap(), saved);
}

#[test]
fn saving_twice_replaces_the_single_row() {
    let store = SqliteAppSettingsStore::open_in_memory().unwrap();

    store
        .save(&AppSettings {
            connect_on_launch: true,
            ..AppSettings::default()
        })
        .unwrap();
    store
        .save(&AppSettings {
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
            .save(&AppSettings {
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
        store.save(&AppSettings::default()).unwrap();
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
            .save(&AppSettings {
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
        store.save(&AppSettings::default()).unwrap();
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

#[test]
fn the_default_system_proxy_mode_is_managed() {
    let store = SqliteAppSettingsStore::open_in_memory().unwrap();

    // The behaviour every existing install already has.
    assert_eq!(
        store.load().unwrap().system_proxy_mode,
        SystemProxyModeSetting::Managed
    );
}

#[test]
fn every_system_proxy_mode_round_trips_through_storage() {
    let store = SqliteAppSettingsStore::open_in_memory().unwrap();

    for mode in [
        SystemProxyModeSetting::Managed,
        SystemProxyModeSetting::Pac,
        SystemProxyModeSetting::Cleared,
        SystemProxyModeSetting::Unchanged,
    ] {
        store
            .save(&AppSettings {
                system_proxy_mode: mode,
                ..AppSettings::default()
            })
            .unwrap();

        assert_eq!(store.load().unwrap().system_proxy_mode, mode);
        assert_eq!(parse_system_proxy_mode(mode.name()), Some(mode));
    }
}

#[test]
fn a_database_written_before_the_mode_existed_still_opens() {
    let database = TestDatabase::new("app-settings-migration");
    {
        // The schema exactly as it was before the System Proxy mode was added.
        let connection = rusqlite::Connection::open(database.path()).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE app_settings (
                     id INTEGER PRIMARY KEY CHECK (id = 1),
                     connect_on_launch INTEGER NOT NULL,
                     close_to_tray INTEGER NOT NULL,
                     core_preference TEXT NOT NULL,
                     tun_enabled INTEGER NOT NULL,
                     launch_at_login INTEGER NOT NULL,
                     log_level TEXT NOT NULL
                 );
                 INSERT INTO app_settings VALUES (1, 1, 0, 'xray', 0, 1, 'debug');",
            )
            .unwrap();
    }

    let store = SqliteAppSettingsStore::open(database.path()).unwrap();
    let settings = store.load().unwrap();

    // The user's existing choices survive, and the new one takes the default
    // rather than failing to read the row.
    assert!(settings.connect_on_launch);
    assert!(!settings.close_to_tray);
    assert!(settings.launch_at_login);
    assert_eq!(settings.core_preference, CorePreferenceSetting::Xray);
    assert_eq!(settings.log_level, LogLevel::Debug);
    assert_eq!(settings.system_proxy_mode, SystemProxyModeSetting::Managed);
}

#[test]
fn opening_twice_does_not_repeat_the_migration() {
    let database = TestDatabase::new("app-settings-migration-twice");
    let first = SqliteAppSettingsStore::open(database.path()).unwrap();
    first
        .save(&AppSettings {
            system_proxy_mode: SystemProxyModeSetting::Cleared,
            ..AppSettings::default()
        })
        .unwrap();
    drop(first);

    // SQLite has no ADD COLUMN IF NOT EXISTS, so every launch after the first
    // hits a duplicate-column error that has to be tolerated, not propagated.
    let second = SqliteAppSettingsStore::open(database.path()).unwrap();

    assert_eq!(
        second.load().unwrap().system_proxy_mode,
        SystemProxyModeSetting::Cleared
    );
}

#[test]
fn pac_without_a_served_url_falls_back_to_the_managed_proxy() {
    // The URL belongs to the server handing out the script, which only the shell
    // knows. Pointing the host at nothing would leave it with no proxy at all
    // while the UI claimed PAC was on.
    assert_eq!(
        SystemProxyModeSetting::Pac.mode(None),
        SystemProxyMode::Managed
    );
    assert_eq!(
        SystemProxyModeSetting::Pac.mode(Some("http://127.0.0.1:1/proxy.pac")),
        SystemProxyMode::Pac("http://127.0.0.1:1/proxy.pac".to_owned())
    );
}

#[test]
fn every_language_round_trips_and_defaults_to_english() {
    let store = SqliteAppSettingsStore::open_in_memory().unwrap();

    // English is what the window opens in; Chinese is what the source strings
    // are written in, which is not the same thing.
    assert_eq!(store.load().unwrap().locale, LocaleSetting::English);

    for locale in LocaleSetting::ALL.iter().copied() {
        store
            .save(&AppSettings {
                locale,
                ..AppSettings::default()
            })
            .unwrap();

        assert_eq!(store.load().unwrap().locale, locale);
        // The stored value is a BCP 47 tag, so it means the same thing to
        // anything else that reads the database.
        assert_eq!(parse_locale(locale.name()), Some(locale));
    }
}

#[test]
fn fragment_enabled_round_trips_through_storage() {
    let store = SqliteAppSettingsStore::open_in_memory().unwrap();

    for enabled in [true, false] {
        store
            .save(&AppSettings {
                fragment_enabled: enabled,
                ..AppSettings::default()
            })
            .unwrap();

        assert_eq!(store.load().unwrap().fragment_enabled, enabled);
    }
}

#[test]
fn udp_noise_enabled_round_trips_through_storage() {
    let store = SqliteAppSettingsStore::open_in_memory().unwrap();

    for enabled in [true, false] {
        store
            .save(&AppSettings {
                udp_noise_enabled: enabled,
                ..AppSettings::default()
            })
            .unwrap();

        assert_eq!(store.load().unwrap().udp_noise_enabled, enabled);
    }
}

#[test]
fn a_database_written_before_fragment_still_opens() {
    let database = TestDatabase::new("app-settings-fragment-migration");
    {
        // The schema exactly as it was before the Fragment toggle was added.
        let connection = rusqlite::Connection::open(database.path()).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE app_settings (
                     id INTEGER PRIMARY KEY CHECK (id = 1),
                     connect_on_launch INTEGER NOT NULL,
                     close_to_tray INTEGER NOT NULL,
                     core_preference TEXT NOT NULL,
                     tun_enabled INTEGER NOT NULL,
                     launch_at_login INTEGER NOT NULL,
                     log_level TEXT NOT NULL
                 );
                 INSERT INTO app_settings VALUES (1, 1, 0, 'xray', 0, 1, 'debug');",
            )
            .unwrap();
    }

    let store = SqliteAppSettingsStore::open(database.path()).unwrap();
    let settings = store.load().unwrap();

    // An existing install must not gain Fragment for free: the traffic shape
    // it already had should not change without the user asking.
    assert!(!settings.fragment_enabled);
}

#[test]
fn an_unknown_language_is_a_typed_error() {
    let database = TestDatabase::new("app-settings-locale");
    {
        let store = SqliteAppSettingsStore::open(database.path()).unwrap();
        store.save(&AppSettings::default()).unwrap();
    }
    let connection = rusqlite::Connection::open(database.path()).unwrap();
    connection
        .execute("UPDATE app_settings SET locale = 'kl' WHERE id = 1", [])
        .unwrap();
    drop(connection);

    let store = SqliteAppSettingsStore::open(database.path()).unwrap();

    assert!(matches!(
        store.load(),
        Err(AppSettingsStoreError::InvalidStoredValue { .. })
    ));
}
