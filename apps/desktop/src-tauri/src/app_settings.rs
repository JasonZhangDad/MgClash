//! Persisted desktop application settings.
//!
//! These are the preferences that shape how the shell behaves rather than how
//! traffic is routed: whether closing the window quits, whether a session starts
//! by itself, and how much of the log the panel keeps showing.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use magies_domain::CoreType;
use magies_profiles::CorePreference;
use magies_session::SystemProxyMode;

use crate::logs::LogLevel;

const CREATE_APP_SETTINGS_TABLE: &str = "
    CREATE TABLE IF NOT EXISTS app_settings (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        connect_on_launch INTEGER NOT NULL,
        close_to_tray INTEGER NOT NULL,
        core_preference TEXT NOT NULL,
        tun_enabled INTEGER NOT NULL,
        launch_at_login INTEGER NOT NULL,
        log_level TEXT NOT NULL
    );
";

/// Adds the System Proxy mode to a table created before it existed.
///
/// `SQLite` has no `ADD COLUMN IF NOT EXISTS`, so the duplicate-column error is
/// the expected outcome on every launch after the first and is discarded. The
/// default keeps existing installs on the behaviour they already had.
const ADD_SYSTEM_PROXY_MODE: &str = "
    ALTER TABLE app_settings ADD COLUMN system_proxy_mode TEXT NOT NULL
        DEFAULT 'managed';
";

/// Adds the interface language, on the same terms as the mode above.
const ADD_LOCALE: &str = "
    ALTER TABLE app_settings ADD COLUMN locale TEXT NOT NULL DEFAULT 'en';
";

/// Adds the editable local inbound ports (v2rayN-style SOCKS/HTTP).
const ADD_SOCKS_PORT: &str = "
    ALTER TABLE app_settings ADD COLUMN socks_port INTEGER NOT NULL DEFAULT 10808;
";

const ADD_HTTP_PORT: &str = "
    ALTER TABLE app_settings ADD COLUMN http_port INTEGER NOT NULL DEFAULT 10809;
";

const ADD_CLASH_API_PORT: &str = "
    ALTER TABLE app_settings ADD COLUMN clash_api_port INTEGER NOT NULL DEFAULT 9090;
";

const ADD_MUX_ENABLED: &str = "
    ALTER TABLE app_settings ADD COLUMN mux_enabled INTEGER NOT NULL DEFAULT 0;
";

const ADD_AUTO_SELECT_LOWEST_LATENCY: &str = "
    ALTER TABLE app_settings ADD COLUMN auto_select_lowest_latency INTEGER NOT NULL DEFAULT 0;
";

const ADD_URL_TEST_ADDRESS: &str = "
    ALTER TABLE app_settings ADD COLUMN url_test_address TEXT NOT NULL DEFAULT 'https://www.gstatic.com/generate_204';
";

const ADD_ALLOW_LAN: &str = "
    ALTER TABLE app_settings ADD COLUMN allow_lan INTEGER NOT NULL DEFAULT 0;
";

const ADD_SPEED_TEST_URL: &str = "
    ALTER TABLE app_settings ADD COLUMN speed_test_url TEXT NOT NULL DEFAULT 'https://speed.cloudflare.com/__down?bytes=10000000';
";

const ADD_INBOUND_UDP_ENABLED: &str = "
    ALTER TABLE app_settings ADD COLUMN inbound_udp_enabled INTEGER NOT NULL DEFAULT 1;
";

const ADD_DEF_ALLOW_INSECURE: &str = "
    ALTER TABLE app_settings ADD COLUMN def_allow_insecure INTEGER NOT NULL DEFAULT 0;
";

const ADD_DEF_FINGERPRINT: &str = "
    ALTER TABLE app_settings ADD COLUMN def_fingerprint TEXT NOT NULL DEFAULT '';
";

const ADD_HOTKEY_CONNECT: &str = "
    ALTER TABLE app_settings ADD COLUMN hotkey_connect TEXT NOT NULL DEFAULT 'Ctrl+Enter';
";

const ADD_HOTKEY_PREVIOUS: &str = "
    ALTER TABLE app_settings ADD COLUMN hotkey_previous TEXT NOT NULL DEFAULT 'Ctrl+[';
";

const ADD_HOTKEY_NEXT: &str = "
    ALTER TABLE app_settings ADD COLUMN hotkey_next TEXT NOT NULL DEFAULT 'Ctrl+]';
";

/// Default URL used when measuring latency through the connected node.
pub const DEFAULT_URL_TEST_ADDRESS: &str = "https://www.gstatic.com/generate_204";

/// Default download used when measuring throughput through the connected node.
pub const DEFAULT_SPEED_TEST_URL: &str = "https://speed.cloudflare.com/__down?bytes=10000000";

/// What the shell does outside of proxying, as the settings panel edits it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent user preference; a bitset would make the settings harder to read, not easier"
)]
pub struct AppSettings {
    /// Connect the last selected node as soon as the app starts.
    pub connect_on_launch: bool,
    /// Closing the window hides it instead of quitting.
    pub close_to_tray: bool,
    /// Start the app when the user logs in.
    pub launch_at_login: bool,
    /// Which Core to run, or `auto` to let the capability matrix decide.
    pub core_preference: CorePreferenceSetting,
    /// Route through a TUN device instead of System Proxy. The two are mutually
    /// exclusive, so enabling this turns System Proxy off for the session.
    pub tun_enabled: bool,
    /// The minimum level the log panel shows on launch.
    pub log_level: LogLevel,
    /// What connecting does to the host's System Proxy.
    pub system_proxy_mode: SystemProxyModeSetting,
    /// The language the window renders in.
    pub locale: LocaleSetting,
    /// Local SOCKS inbound port for the next session.
    pub socks_port: u16,
    /// Local HTTP inbound port for the next session.
    pub http_port: u16,
    /// Loopback Clash API port used for traffic sampling.
    pub clash_api_port: u16,
    /// Ask the Core for multiplex / mux on the next connect.
    pub mux_enabled: bool,
    /// After a batch latency test, select the fastest successful node.
    pub auto_select_lowest_latency: bool,
    /// HTTP(S) address used by the connected-node URL test.
    pub url_test_address: String,
    /// Bind local SOCKS/HTTP inbounds on all interfaces for LAN clients.
    pub allow_lan: bool,
    /// HTTP(S) address used by the connected-node download speed test.
    pub speed_test_url: String,
    /// Enable SOCKS UDP associate on the next Xray connect.
    pub inbound_udp_enabled: bool,
    /// Default "allow insecure" when creating a TLS node.
    pub def_allow_insecure: bool,
    /// Default TLS fingerprint when creating a TLS node (empty = none).
    pub def_fingerprint: String,
    /// Window hotkey that toggles connect / disconnect (empty disables).
    pub hotkey_connect: String,
    /// Window hotkey that selects the previous enabled node (empty disables).
    pub hotkey_previous: String,
    /// Window hotkey that selects the next enabled node (empty disables).
    pub hotkey_next: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // Off by default: starting a proxy without being asked is a
            // surprise, and the PRD treats connecting as an explicit action.
            connect_on_launch: false,
            close_to_tray: true,
            launch_at_login: false,
            core_preference: CorePreferenceSetting::Auto,
            system_proxy_mode: SystemProxyModeSetting::Managed,
            locale: LocaleSetting::English,
            // Off by default: TUN needs elevation and takes over the whole
            // routing table, which is not something to switch on unasked.
            tun_enabled: false,
            log_level: LogLevel::Info,
            socks_port: 10_808,
            http_port: 10_809,
            clash_api_port: 9_090,
            mux_enabled: false,
            auto_select_lowest_latency: false,
            url_test_address: DEFAULT_URL_TEST_ADDRESS.to_owned(),
            allow_lan: false,
            speed_test_url: DEFAULT_SPEED_TEST_URL.to_owned(),
            inbound_udp_enabled: true,
            def_allow_insecure: false,
            def_fingerprint: String::new(),
            hotkey_connect: "Ctrl+Enter".to_owned(),
            hotkey_previous: "Ctrl+[".to_owned(),
            hotkey_next: "Ctrl+]".to_owned(),
        }
    }
}

#[derive(Debug, Error)]
pub enum AppSettingsStoreError {
    #[error("failed to access the application settings database")]
    Database(#[from] rusqlite::Error),
    #[error("the application settings database contains unsupported log level `{value}`")]
    InvalidStoredValue { value: String },
}

pub struct SqliteAppSettingsStore {
    connection: Connection,
}

impl SqliteAppSettingsStore {
    /// Opens or creates the settings table in `path`.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when `SQLite` cannot be opened or migrated.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppSettingsStoreError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Opens an isolated store for tests.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when the in-memory schema cannot be created.
    pub fn open_in_memory() -> Result<Self, AppSettingsStoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self, AppSettingsStoreError> {
        connection.execute_batch(CREATE_APP_SETTINGS_TABLE)?;
        // Only a duplicate column is tolerated; anything else is a real failure.
        for migration in [
            ADD_SYSTEM_PROXY_MODE,
            ADD_LOCALE,
            ADD_SOCKS_PORT,
            ADD_HTTP_PORT,
            ADD_CLASH_API_PORT,
            ADD_MUX_ENABLED,
            ADD_AUTO_SELECT_LOWEST_LATENCY,
            ADD_URL_TEST_ADDRESS,
            ADD_ALLOW_LAN,
            ADD_SPEED_TEST_URL,
            ADD_INBOUND_UDP_ENABLED,
            ADD_DEF_ALLOW_INSECURE,
            ADD_DEF_FINGERPRINT,
            ADD_HOTKEY_CONNECT,
            ADD_HOTKEY_PREVIOUS,
            ADD_HOTKEY_NEXT,
        ] {
            if let Err(error) = connection.execute_batch(migration)
                && !error.to_string().contains("duplicate column")
            {
                return Err(error.into());
            }
        }
        Ok(Self { connection })
    }

    /// Loads the saved settings, falling back to the defaults on first launch.
    ///
    /// # Errors
    ///
    /// Returns a typed database or invalid-stored-value error.
    pub fn load(&self) -> Result<AppSettings, AppSettingsStoreError> {
        let row = self
            .connection
            .query_row(
                "SELECT connect_on_launch, close_to_tray, launch_at_login, core_preference, tun_enabled, log_level, system_proxy_mode, locale, socks_port, http_port, clash_api_port, mux_enabled, auto_select_lowest_latency, url_test_address, allow_lan, speed_test_url, inbound_udp_enabled, def_allow_insecure, def_fingerprint, hotkey_connect, hotkey_previous, hotkey_next
                 FROM app_settings WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, i64>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, i64>(11)?,
                        row.get::<_, i64>(12)?,
                        row.get::<_, String>(13)?,
                        row.get::<_, i64>(14)?,
                        row.get::<_, String>(15)?,
                        row.get::<_, i64>(16)?,
                        row.get::<_, i64>(17)?,
                        row.get::<_, String>(18)?,
                        row.get::<_, String>(19)?,
                        row.get::<_, String>(20)?,
                        row.get::<_, String>(21)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            connect_on_launch,
            close_to_tray,
            launch_at_login,
            core_preference,
            tun_enabled,
            log_level,
            system_proxy_mode,
            locale,
            socks_port,
            http_port,
            clash_api_port,
            mux_enabled,
            auto_select_lowest_latency,
            url_test_address,
            allow_lan,
            speed_test_url,
            inbound_udp_enabled,
            def_allow_insecure,
            def_fingerprint,
            hotkey_connect,
            hotkey_previous,
            hotkey_next,
        )) = row
        else {
            return Ok(AppSettings::default());
        };

        Ok(AppSettings {
            connect_on_launch: connect_on_launch != 0,
            close_to_tray: close_to_tray != 0,
            launch_at_login: launch_at_login != 0,
            core_preference: parse_core_preference(&core_preference).ok_or(
                AppSettingsStoreError::InvalidStoredValue {
                    value: core_preference,
                },
            )?,
            tun_enabled: tun_enabled != 0,
            log_level: parse_log_level(&log_level)
                .ok_or(AppSettingsStoreError::InvalidStoredValue { value: log_level })?,
            system_proxy_mode: parse_system_proxy_mode(&system_proxy_mode).ok_or(
                AppSettingsStoreError::InvalidStoredValue {
                    value: system_proxy_mode,
                },
            )?,
            locale: parse_locale(&locale)
                .ok_or(AppSettingsStoreError::InvalidStoredValue { value: locale })?,
            socks_port: parse_stored_port(socks_port)?,
            http_port: parse_stored_port(http_port)?,
            clash_api_port: parse_stored_port(clash_api_port)?,
            mux_enabled: mux_enabled != 0,
            auto_select_lowest_latency: auto_select_lowest_latency != 0,
            url_test_address: normalize_url_test_address(url_test_address),
            allow_lan: allow_lan != 0,
            speed_test_url: normalize_speed_test_url(speed_test_url),
            inbound_udp_enabled: inbound_udp_enabled != 0,
            def_allow_insecure: def_allow_insecure != 0,
            def_fingerprint: def_fingerprint.trim().to_owned(),
            hotkey_connect: normalize_hotkey(hotkey_connect),
            hotkey_previous: normalize_hotkey(hotkey_previous),
            hotkey_next: normalize_hotkey(hotkey_next),
        })
    }

    /// Saves the settings for the next launch.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when `SQLite` cannot update the row.
    pub fn save(&self, settings: &AppSettings) -> Result<(), AppSettingsStoreError> {
        self.connection.execute(
            "INSERT INTO app_settings (id, connect_on_launch, close_to_tray, launch_at_login, core_preference, tun_enabled, log_level, system_proxy_mode, locale, socks_port, http_port, clash_api_port, mux_enabled, auto_select_lowest_latency, url_test_address, allow_lan, speed_test_url, inbound_udp_enabled, def_allow_insecure, def_fingerprint, hotkey_connect, hotkey_previous, hotkey_next)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)
             ON CONFLICT(id) DO UPDATE SET
                 connect_on_launch = excluded.connect_on_launch,
                 close_to_tray = excluded.close_to_tray,
                 launch_at_login = excluded.launch_at_login,
                 core_preference = excluded.core_preference,
                 system_proxy_mode = excluded.system_proxy_mode,
                 locale = excluded.locale,
                 tun_enabled = excluded.tun_enabled,
                 log_level = excluded.log_level,
                 socks_port = excluded.socks_port,
                 http_port = excluded.http_port,
                 clash_api_port = excluded.clash_api_port,
                 mux_enabled = excluded.mux_enabled,
                 auto_select_lowest_latency = excluded.auto_select_lowest_latency,
                 url_test_address = excluded.url_test_address,
                 allow_lan = excluded.allow_lan,
                 speed_test_url = excluded.speed_test_url,
                 inbound_udp_enabled = excluded.inbound_udp_enabled,
                 def_allow_insecure = excluded.def_allow_insecure,
                 def_fingerprint = excluded.def_fingerprint,
                 hotkey_connect = excluded.hotkey_connect,
                 hotkey_previous = excluded.hotkey_previous,
                 hotkey_next = excluded.hotkey_next",
            params![
                i64::from(settings.connect_on_launch),
                i64::from(settings.close_to_tray),
                i64::from(settings.launch_at_login),
                settings.core_preference.name(),
                i64::from(settings.tun_enabled),
                log_level_name(settings.log_level),
                settings.system_proxy_mode.name(),
                settings.locale.name(),
                i64::from(settings.socks_port),
                i64::from(settings.http_port),
                i64::from(settings.clash_api_port),
                i64::from(settings.mux_enabled),
                i64::from(settings.auto_select_lowest_latency),
                normalize_url_test_address(&settings.url_test_address),
                i64::from(settings.allow_lan),
                normalize_speed_test_url(&settings.speed_test_url),
                i64::from(settings.inbound_udp_enabled),
                i64::from(settings.def_allow_insecure),
                settings.def_fingerprint.trim(),
                normalize_hotkey(&settings.hotkey_connect),
                normalize_hotkey(&settings.hotkey_previous),
                normalize_hotkey(&settings.hotkey_next),
            ],
        )?;
        Ok(())
    }
}

/// What connecting does to the host's System Proxy.
///
/// Matches v2rayN's four choices. PAC starts a loopback `PacServer` and points
/// the host at its URL (global script only — not Core rule mode).
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SystemProxyModeSetting {
    /// Point the host at this session's local proxies.
    #[default]
    Managed,
    /// Point the host at a proxy auto-configuration file `MgClash` serves.
    Pac,
    /// Clear the host's proxy while connected, restoring it on disconnect.
    Cleared,
    /// Leave the host's proxy exactly as the user configured it.
    Unchanged,
}

impl SystemProxyModeSetting {
    /// Converts to the session layer's own type.
    ///
    /// PAC needs the URL of the server handing out the script, which only the
    /// shell knows, so the caller supplies it. Without one the mode falls back
    /// to the managed proxy rather than pointing the host at nothing.
    #[must_use]
    pub fn mode(self, pac_url: Option<&str>) -> SystemProxyMode {
        match self {
            Self::Managed => SystemProxyMode::Managed,
            Self::Pac => pac_url.map_or(SystemProxyMode::Managed, |url| {
                SystemProxyMode::Pac(url.to_owned())
            }),
            Self::Cleared => SystemProxyMode::Cleared,
            Self::Unchanged => SystemProxyMode::Unchanged,
        }
    }

    /// The stable value stored in `SQLite` and exchanged with the webview.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::Pac => "pac",
            Self::Cleared => "cleared",
            Self::Unchanged => "unchanged",
        }
    }
}

/// Reads the stored System Proxy mode, or `None` for an unknown value.
#[must_use]
pub fn parse_system_proxy_mode(value: &str) -> Option<SystemProxyModeSetting> {
    match value {
        "managed" => Some(SystemProxyModeSetting::Managed),
        "pac" => Some(SystemProxyModeSetting::Pac),
        "cleared" => Some(SystemProxyModeSetting::Cleared),
        "unchanged" => Some(SystemProxyModeSetting::Unchanged),
        _ => None,
    }
}

/// The language the window renders in.
///
/// Chinese is the language every source string is written in, so it needs no
/// dictionary; English is a translation layered over it.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum LocaleSetting {
    /// The language the window opens in.
    #[default]
    #[serde(rename = "en")]
    English,
    /// The language every source string is written in, so it needs no
    /// dictionary of its own.
    #[serde(rename = "zh-Hans")]
    SimplifiedChinese,
    #[serde(rename = "zh-Hant")]
    TraditionalChinese,
    #[serde(rename = "de")]
    German,
    #[serde(rename = "fr")]
    French,
    #[serde(rename = "es")]
    Spanish,
    #[serde(rename = "it")]
    Italian,
    #[serde(rename = "ru")]
    Russian,
    #[serde(rename = "ja")]
    Japanese,
    #[serde(rename = "ko")]
    Korean,
}

impl LocaleSetting {
    /// Every language, matching the webview's own list.
    pub const ALL: &'static [Self] = &[
        Self::English,
        Self::SimplifiedChinese,
        Self::TraditionalChinese,
        Self::German,
        Self::French,
        Self::Spanish,
        Self::Italian,
        Self::Russian,
        Self::Japanese,
        Self::Korean,
    ];

    /// The stable value stored in `SQLite` and exchanged with the webview.
    ///
    /// BCP 47 tags rather than invented names, so the stored value means the
    /// same thing to anything else that reads it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::SimplifiedChinese => "zh-Hans",
            Self::TraditionalChinese => "zh-Hant",
            Self::German => "de",
            Self::French => "fr",
            Self::Spanish => "es",
            Self::Italian => "it",
            Self::Russian => "ru",
            Self::Japanese => "ja",
            Self::Korean => "ko",
        }
    }
}

/// Reads the stored language, or `None` for an unknown value.
#[must_use]
pub fn parse_locale(value: &str) -> Option<LocaleSetting> {
    LocaleSetting::ALL
        .iter()
        .copied()
        .find(|locale| locale.name() == value)
}

/// The Core the user picked, as the settings panel offers it.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CorePreferenceSetting {
    /// Let the capability matrix decide from the node and enabled features.
    #[default]
    Auto,
    SingBox,
    Xray,
}

impl CorePreferenceSetting {
    /// Converts to the matrix's own type.
    #[must_use]
    pub const fn preference(self) -> CorePreference {
        match self {
            Self::Auto => CorePreference::Auto,
            Self::SingBox => CorePreference::Fixed(CoreType::SingBox),
            Self::Xray => CorePreference::Fixed(CoreType::Xray),
        }
    }

    /// The stable value stored in `SQLite` and exchanged with the webview.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::SingBox => "sing-box",
            Self::Xray => "xray",
        }
    }
}

/// Parses the stable stored value.
#[must_use]
pub fn parse_core_preference(value: &str) -> Option<CorePreferenceSetting> {
    match value {
        "auto" => Some(CorePreferenceSetting::Auto),
        "sing-box" => Some(CorePreferenceSetting::SingBox),
        "xray" => Some(CorePreferenceSetting::Xray),
        _ => None,
    }
}

/// Parses the stable value exchanged with the desktop webview.
#[must_use]
pub fn parse_log_level(value: &str) -> Option<LogLevel> {
    match value {
        "error" => Some(LogLevel::Error),
        "warn" => Some(LogLevel::Warn),
        "info" => Some(LogLevel::Info),
        "debug" => Some(LogLevel::Debug),
        "trace" => Some(LogLevel::Trace),
        _ => None,
    }
}

/// Trims a blank URL test address back to the documented default.
#[must_use]
pub fn normalize_url_test_address(value: impl AsRef<str>) -> String {
    let trimmed = value.as_ref().trim();
    if trimmed.is_empty() {
        DEFAULT_URL_TEST_ADDRESS.to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Trims a blank speed-test URL back to the documented default.
#[must_use]
pub fn normalize_speed_test_url(value: impl AsRef<str>) -> String {
    let trimmed = value.as_ref().trim();
    if trimmed.is_empty() {
        DEFAULT_SPEED_TEST_URL.to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Normalizes a hotkey chord; empty means the binding is disabled.
#[must_use]
pub fn normalize_hotkey(value: impl AsRef<str>) -> String {
    value
        .as_ref()
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            if part.eq_ignore_ascii_case("ctrl")
                || part.eq_ignore_ascii_case("control")
            {
                "Ctrl".to_owned()
            } else if part.eq_ignore_ascii_case("alt")
                || part.eq_ignore_ascii_case("option")
            {
                "Alt".to_owned()
            } else if part.eq_ignore_ascii_case("shift") {
                "Shift".to_owned()
            } else if part.eq_ignore_ascii_case("meta")
                || part.eq_ignore_ascii_case("cmd")
                || part.eq_ignore_ascii_case("command")
            {
                "Meta".to_owned()
            } else if part.len() == 1 {
                part.to_ascii_uppercase()
            } else {
                // Keep multi-character keys like Enter / ArrowLeft readable.
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => {
                        first.to_ascii_uppercase().to_string() + chars.as_str()
                    }
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Reads a stored inbound port, rejecting zero and out-of-range values.
fn parse_stored_port(value: i64) -> Result<u16, AppSettingsStoreError> {
    u16::try_from(value)
        .ok()
        .filter(|port| *port != 0)
        .ok_or(AppSettingsStoreError::InvalidStoredValue {
            value: value.to_string(),
        })
}

#[must_use]
pub const fn log_level_name(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "error",
        LogLevel::Warn => "warn",
        LogLevel::Info => "info",
        LogLevel::Debug => "debug",
        LogLevel::Trace => "trace",
    }
}
