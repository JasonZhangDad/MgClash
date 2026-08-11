//! Persisted desktop route rules and their validated Core profile.

use std::path::Path;

use magies_routing::{
    Network, RouteConfigError, RouteOutbound, RouteProfile, RoutingMode, RoutingRule,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const CREATE_ROUTE_SETTINGS_TABLE: &str = "
    CREATE TABLE IF NOT EXISTS route_settings (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        settings_json TEXT NOT NULL
    );
";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RouteRuleKind {
    Domain,
    DomainSuffix,
    DomainKeyword,
    IpCidr,
    IpCidr6,
    GeoIp,
    GeoSite,
    Port,
    Network,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DesktopRouteOutbound {
    #[default]
    Proxy,
    Direct,
}

impl From<DesktopRouteOutbound> for RouteOutbound {
    fn from(outbound: DesktopRouteOutbound) -> Self {
        match outbound {
            DesktopRouteOutbound::Proxy => Self::Proxy,
            DesktopRouteOutbound::Direct => Self::Direct,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteRuleSetting {
    pub kind: RouteRuleKind,
    pub value: String,
    pub outbound: DesktopRouteOutbound,
    pub enabled: bool,
}

impl RouteRuleSetting {
    fn rule(&self, priority: i32) -> Result<RoutingRule, RouteSettingsError> {
        let outbound = self.outbound.into();
        Ok(match self.kind {
            RouteRuleKind::Domain => {
                RoutingRule::domain(&self.value, outbound, priority, self.enabled)?
            }
            RouteRuleKind::DomainSuffix => {
                RoutingRule::domain_suffix(&self.value, outbound, priority, self.enabled)?
            }
            RouteRuleKind::DomainKeyword => {
                RoutingRule::domain_keyword(&self.value, outbound, priority, self.enabled)?
            }
            RouteRuleKind::IpCidr => {
                RoutingRule::ip_cidr(&self.value, outbound, priority, self.enabled)?
            }
            RouteRuleKind::IpCidr6 => {
                RoutingRule::ip_cidr6(&self.value, outbound, priority, self.enabled)?
            }
            RouteRuleKind::GeoIp => {
                RoutingRule::geo_ip(&self.value, outbound, priority, self.enabled)?
            }
            RouteRuleKind::GeoSite => {
                RoutingRule::geo_site(&self.value, outbound, priority, self.enabled)?
            }
            RouteRuleKind::Port => {
                let port = self.value.trim().parse().map_err(|_| {
                    RouteSettingsError::InvalidPortValue {
                        value: self.value.clone(),
                    }
                })?;
                RoutingRule::port(port, outbound, priority, self.enabled)?
            }
            RouteRuleKind::Network => {
                let network = match self.value.trim().to_ascii_lowercase().as_str() {
                    "tcp" => Network::Tcp,
                    "udp" => Network::Udp,
                    _ => {
                        return Err(RouteSettingsError::InvalidNetwork {
                            value: self.value.clone(),
                        });
                    }
                };
                RoutingRule::network(network, outbound, priority, self.enabled)
            }
        })
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteSettings {
    pub rules: Vec<RouteRuleSetting>,
    pub final_outbound: DesktopRouteOutbound,
}

impl RouteSettings {
    /// Builds the validated route profile used by the selected desktop mode.
    ///
    /// # Errors
    ///
    /// Returns a typed matcher or profile error.
    pub fn profile(&self, mode: RoutingMode) -> Result<RouteProfile, RouteSettingsError> {
        match mode {
            RoutingMode::Global => Ok(RouteProfile::new(mode, Vec::new(), RouteOutbound::Proxy)?),
            RoutingMode::Direct => Ok(RouteProfile::new(mode, Vec::new(), RouteOutbound::Direct)?),
            RoutingMode::Rule => {
                let rules = self
                    .rules
                    .iter()
                    .zip(0_i32..)
                    .map(|(rule, priority)| rule.rule(priority))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(RouteProfile::new(mode, rules, self.final_outbound.into())?)
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum RouteSettingsError {
    #[error("route port is not an integer: {value}")]
    InvalidPortValue { value: String },
    #[error("route network must be tcp or udp, got {value}")]
    InvalidNetwork { value: String },
    #[error(transparent)]
    Profile(#[from] RouteConfigError),
}

#[derive(Debug, Error)]
pub enum RouteSettingsStoreError {
    #[error("failed to access the route settings database")]
    Database(#[from] rusqlite::Error),
    #[error("failed to encode the route settings")]
    Encode(#[source] serde_json::Error),
    #[error("the route settings database contains invalid JSON")]
    InvalidStoredJson(#[source] serde_json::Error),
    #[error("the route settings database contains invalid settings")]
    InvalidStoredSettings(#[source] RouteSettingsError),
    #[error("refusing to save invalid route settings")]
    InvalidSettings(#[from] RouteSettingsError),
}

pub struct SqliteRouteSettingsStore {
    connection: Connection,
}

impl SqliteRouteSettingsStore {
    /// Opens or creates the route settings table in `path`.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when `SQLite` cannot be opened or migrated.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RouteSettingsStoreError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Opens an isolated store for tests.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when the in-memory schema cannot be created.
    pub fn open_in_memory() -> Result<Self, RouteSettingsStoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self, RouteSettingsStoreError> {
        connection.execute_batch(CREATE_ROUTE_SETTINGS_TABLE)?;
        Ok(Self { connection })
    }

    /// Loads saved settings, defaulting to no rules and a proxy final.
    ///
    /// # Errors
    ///
    /// Returns a typed database or invalid-stored-value error.
    pub fn load(&self) -> Result<RouteSettings, RouteSettingsStoreError> {
        let value = self
            .connection
            .query_row(
                "SELECT settings_json FROM route_settings WHERE id = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(value) = value else {
            return Ok(RouteSettings::default());
        };
        let settings: RouteSettings =
            serde_json::from_str(&value).map_err(RouteSettingsStoreError::InvalidStoredJson)?;
        settings
            .profile(RoutingMode::Rule)
            .map_err(RouteSettingsStoreError::InvalidStoredSettings)?;
        Ok(settings)
    }

    /// Validates and saves settings for the next launch.
    ///
    /// # Errors
    ///
    /// Returns a typed validation, encoding, or database error.
    pub fn save(&self, settings: &RouteSettings) -> Result<(), RouteSettingsStoreError> {
        settings.profile(RoutingMode::Rule)?;
        let value = serde_json::to_string(settings).map_err(RouteSettingsStoreError::Encode)?;
        self.connection.execute(
            "INSERT INTO route_settings (id, settings_json) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET settings_json = excluded.settings_json",
            params![value],
        )?;
        Ok(())
    }
}
