//! Persisted desktop routing-mode settings.

use std::path::Path;

use magies_routing::{RouteOutbound, RouteProfile, RoutingMode};
use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;

const CREATE_ROUTING_MODE_TABLE: &str = "
    CREATE TABLE IF NOT EXISTS routing_mode (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        mode TEXT NOT NULL
    );
";

#[derive(Debug, Error)]
pub enum RoutingModeValueError {
    #[error("unsupported routing mode `{value}`")]
    Unsupported { value: String },
}

#[derive(Debug, Error)]
pub enum RoutingModeStoreError {
    #[error("failed to access the routing-mode database")]
    Database(#[from] rusqlite::Error),
    #[error("the routing-mode database contains unsupported value `{value}`")]
    InvalidStoredValue { value: String },
}

pub struct SqliteRoutingModeStore {
    connection: Connection,
}

impl SqliteRoutingModeStore {
    /// Opens or creates the routing-mode table in `path`.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when `SQLite` cannot be opened or migrated.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RoutingModeStoreError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Opens an isolated store for tests.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when the in-memory schema cannot be created.
    pub fn open_in_memory() -> Result<Self, RoutingModeStoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self, RoutingModeStoreError> {
        connection.execute_batch(CREATE_ROUTING_MODE_TABLE)?;
        Ok(Self { connection })
    }

    /// Loads the saved mode, defaulting to Global on first launch.
    ///
    /// # Errors
    ///
    /// Returns a typed database or invalid-stored-value error.
    pub fn load(&self) -> Result<RoutingMode, RoutingModeStoreError> {
        let value = self
            .connection
            .query_row("SELECT mode FROM routing_mode WHERE id = 1", [], |row| {
                row.get::<_, String>(0)
            })
            .optional()?;
        let Some(value) = value else {
            return Ok(RoutingMode::Global);
        };
        parse_routing_mode(&value).map_err(|RoutingModeValueError::Unsupported { value }| {
            RoutingModeStoreError::InvalidStoredValue { value }
        })
    }

    /// Saves the mode for the next launch.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when `SQLite` cannot update the row.
    pub fn save(&self, mode: RoutingMode) -> Result<(), RoutingModeStoreError> {
        self.connection.execute(
            "INSERT INTO routing_mode (id, mode) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET mode = excluded.mode",
            params![routing_mode_name(mode)],
        )?;
        Ok(())
    }
}

/// Parses the stable value exchanged with the desktop webview.
///
/// # Errors
///
/// Returns a typed error for any value outside Global, Rule, and Direct.
pub fn parse_routing_mode(value: &str) -> Result<RoutingMode, RoutingModeValueError> {
    match value {
        "global" => Ok(RoutingMode::Global),
        "rule" => Ok(RoutingMode::Rule),
        "direct" => Ok(RoutingMode::Direct),
        _ => Err(RoutingModeValueError::Unsupported {
            value: value.to_owned(),
        }),
    }
}

#[must_use]
pub const fn routing_mode_name(mode: RoutingMode) -> &'static str {
    match mode {
        RoutingMode::Global => "global",
        RoutingMode::Rule => "rule",
        RoutingMode::Direct => "direct",
    }
}

/// Builds the current no-custom-rules route for one desktop mode.
///
/// # Panics
///
/// Panics only if a desktop mode stops mapping to its valid final outbound.
#[must_use]
pub fn route_profile_for(mode: RoutingMode) -> RouteProfile {
    let final_outbound = match mode {
        RoutingMode::Global | RoutingMode::Rule => RouteOutbound::Proxy,
        RoutingMode::Direct => RouteOutbound::Direct,
    };
    RouteProfile::new(mode, Vec::new(), final_outbound)
        .expect("desktop routing modes always use their valid final outbound")
}
