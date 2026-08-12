//! Persisted desktop application settings.
//!
//! These are the preferences that shape how the shell behaves rather than how
//! traffic is routed: whether closing the window quits, whether a session starts
//! by itself, and how much of the log the panel keeps showing.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::logs::LogLevel;

const CREATE_APP_SETTINGS_TABLE: &str = "
    CREATE TABLE IF NOT EXISTS app_settings (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        connect_on_launch INTEGER NOT NULL,
        close_to_tray INTEGER NOT NULL,
        launch_at_login INTEGER NOT NULL,
        log_level TEXT NOT NULL
    );
";

/// What the shell does outside of proxying, as the settings panel edits it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Connect the last selected node as soon as the app starts.
    pub connect_on_launch: bool,
    /// Closing the window hides it instead of quitting.
    pub close_to_tray: bool,
    /// Start the app when the user logs in.
    pub launch_at_login: bool,
    /// The minimum level the log panel shows on launch.
    pub log_level: LogLevel,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // Off by default: starting a proxy without being asked is a
            // surprise, and the PRD treats connecting as an explicit action.
            connect_on_launch: false,
            close_to_tray: true,
            launch_at_login: false,
            log_level: LogLevel::Info,
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
                "SELECT connect_on_launch, close_to_tray, launch_at_login, log_level
                 FROM app_settings WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((connect_on_launch, close_to_tray, launch_at_login, log_level)) = row else {
            return Ok(AppSettings::default());
        };

        Ok(AppSettings {
            connect_on_launch: connect_on_launch != 0,
            close_to_tray: close_to_tray != 0,
            launch_at_login: launch_at_login != 0,
            log_level: parse_log_level(&log_level)
                .ok_or(AppSettingsStoreError::InvalidStoredValue { value: log_level })?,
        })
    }

    /// Saves the settings for the next launch.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when `SQLite` cannot update the row.
    pub fn save(&self, settings: AppSettings) -> Result<(), AppSettingsStoreError> {
        self.connection.execute(
            "INSERT INTO app_settings (id, connect_on_launch, close_to_tray, launch_at_login, log_level)
             VALUES (1, ?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                 connect_on_launch = excluded.connect_on_launch,
                 close_to_tray = excluded.close_to_tray,
                 launch_at_login = excluded.launch_at_login,
                 log_level = excluded.log_level",
            params![
                i64::from(settings.connect_on_launch),
                i64::from(settings.close_to_tray),
                i64::from(settings.launch_at_login),
                log_level_name(settings.log_level),
            ],
        )?;
        Ok(())
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
