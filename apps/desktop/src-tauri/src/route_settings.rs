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

pub const DEFAULT_ROUTE_SCHEME_ID: &str = "default";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteScheme {
    pub id: String,
    pub name: String,
    pub settings: RouteSettings,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteSchemeBundle {
    pub active_scheme_id: String,
    pub schemes: Vec<RouteScheme>,
}

impl RouteSchemeBundle {
    #[must_use]
    pub fn default_bundle() -> Self {
        Self {
            active_scheme_id: DEFAULT_ROUTE_SCHEME_ID.to_owned(),
            schemes: vec![RouteScheme {
                id: DEFAULT_ROUTE_SCHEME_ID.to_owned(),
                name: "默认".to_owned(),
                settings: RouteSettings::default(),
            }],
        }
    }

    /// Returns the settings for the active routing scheme.
    ///
    /// # Panics
    ///
    /// Panics only if the bundle contains no schemes or the active id is missing,
    /// which `default_bundle` and `save` prevent.
    #[must_use]
    pub fn active_settings(&self) -> RouteSettings {
        self.schemes
            .iter()
            .find(|scheme| scheme.id == self.active_scheme_id)
            .map(|scheme| scheme.settings.clone())
            .expect("route scheme bundles always contain the active scheme")
    }

    /// Builds the validated route profile for the active scheme.
    ///
    /// # Errors
    ///
    /// Returns a typed matcher or profile error.
    pub fn profile(&self, mode: RoutingMode) -> Result<RouteProfile, RouteSettingsError> {
        self.active_settings().profile(mode)
    }

    /// Switches the active routing scheme.
    ///
    /// # Errors
    ///
    /// Returns a typed error when `scheme_id` is not present.
    pub fn select_scheme(&mut self, scheme_id: &str) -> Result<(), RouteSettingsError> {
        if !self.schemes.iter().any(|scheme| scheme.id == scheme_id) {
            return Err(RouteSettingsError::UnknownScheme {
                id: scheme_id.to_owned(),
            });
        }
        self.active_scheme_id = scheme_id.to_owned();
        Ok(())
    }

    /// Adds a routing scheme cloned from the active one.
    ///
    /// # Errors
    ///
    /// Returns a typed error when `name` is blank or `scheme_id` already exists.
    pub fn add_scheme(
        &mut self,
        scheme_id: String,
        name: String,
    ) -> Result<(), RouteSettingsError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(RouteSettingsError::EmptySchemeName);
        }
        if self.schemes.iter().any(|scheme| scheme.id == scheme_id) {
            return Err(RouteSettingsError::DuplicateSchemeId {
                id: scheme_id.clone(),
            });
        }
        self.schemes.push(RouteScheme {
            id: scheme_id,
            name: name.to_owned(),
            settings: self.active_settings(),
        });
        Ok(())
    }

    /// Deletes one routing scheme when it is not the only entry.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the scheme is missing or it is the last one.
    pub fn delete_scheme(&mut self, scheme_id: &str) -> Result<(), RouteSettingsError> {
        if self.schemes.len() <= 1 {
            return Err(RouteSettingsError::LastScheme);
        }
        let index = self
            .schemes
            .iter()
            .position(|scheme| scheme.id == scheme_id)
            .ok_or_else(|| RouteSettingsError::UnknownScheme {
                id: scheme_id.to_owned(),
            })?;
        self.schemes.remove(index);
        if self.active_scheme_id == scheme_id {
            self.active_scheme_id = self.schemes[0].id.clone();
        }
        Ok(())
    }

    /// Replaces the active scheme's settings.
    ///
    /// # Errors
    ///
    /// Returns a typed matcher or profile error.
    pub fn set_active_settings(
        &mut self,
        settings: RouteSettings,
    ) -> Result<(), RouteSettingsError> {
        settings.profile(RoutingMode::Rule)?;
        let active = self
            .schemes
            .iter_mut()
            .find(|scheme| scheme.id == self.active_scheme_id)
            .expect("route scheme bundles always contain the active scheme");
        active.settings = settings;
        Ok(())
    }
}

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
    ProcessName,
    ProcessPath,
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
            RouteRuleKind::ProcessName => {
                RoutingRule::process_name(&self.value, outbound, priority, self.enabled)?
            }
            RouteRuleKind::ProcessPath => {
                RoutingRule::process_path(&self.value, outbound, priority, self.enabled)?
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
    #[error("routing scheme name cannot be empty")]
    EmptySchemeName,
    #[error("duplicate routing scheme id: {id}")]
    DuplicateSchemeId { id: String },
    #[error("unknown routing scheme id: {id}")]
    UnknownScheme { id: String },
    #[error("the last routing scheme cannot be deleted")]
    LastScheme,
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
        Ok(self.load_bundle()?.active_settings())
    }

    /// Loads the complete routing-scheme bundle.
    ///
    /// # Errors
    ///
    /// Returns a typed database or invalid-stored-value error.
    pub fn load_bundle(&self) -> Result<RouteSchemeBundle, RouteSettingsStoreError> {
        let value = self
            .connection
            .query_row(
                "SELECT settings_json FROM route_settings WHERE id = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(value) = value else {
            return Ok(RouteSchemeBundle::default_bundle());
        };
        decode_bundle(&value)
    }

    /// Validates and saves settings for the next launch.
    ///
    /// # Errors
    ///
    /// Returns a typed validation, encoding, or database error.
    pub fn save(&self, settings: &RouteSettings) -> Result<(), RouteSettingsStoreError> {
        let mut bundle = self.load_bundle()?;
        bundle
            .set_active_settings(settings.clone())
            .map_err(RouteSettingsStoreError::InvalidSettings)?;
        self.save_bundle(&bundle)
    }

    /// Validates and saves the complete routing-scheme bundle.
    ///
    /// # Errors
    ///
    /// Returns a typed validation, encoding, or database error.
    pub fn save_bundle(
        &self,
        bundle: &RouteSchemeBundle,
    ) -> Result<(), RouteSettingsStoreError> {
        bundle
            .profile(RoutingMode::Rule)
            .map_err(RouteSettingsStoreError::InvalidSettings)?;
        if bundle.schemes.is_empty() {
            return Err(RouteSettingsStoreError::InvalidSettings(
                RouteSettingsError::LastScheme,
            ));
        }
        if !bundle
            .schemes
            .iter()
            .any(|scheme| scheme.id == bundle.active_scheme_id)
        {
            return Err(RouteSettingsStoreError::InvalidSettings(
                RouteSettingsError::UnknownScheme {
                    id: bundle.active_scheme_id.clone(),
                },
            ));
        }
        let value =
            serde_json::to_string(bundle).map_err(RouteSettingsStoreError::Encode)?;
        self.connection.execute(
            "INSERT INTO route_settings (id, settings_json) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET settings_json = excluded.settings_json",
            params![value],
        )?;
        Ok(())
    }
}

fn decode_bundle(value: &str) -> Result<RouteSchemeBundle, RouteSettingsStoreError> {
    if let Ok(bundle) = serde_json::from_str::<RouteSchemeBundle>(value) {
        bundle
            .profile(RoutingMode::Rule)
            .map_err(RouteSettingsStoreError::InvalidStoredSettings)?;
        return Ok(bundle);
    }
    let legacy: RouteSettings =
        serde_json::from_str(value).map_err(RouteSettingsStoreError::InvalidStoredJson)?;
    legacy
        .profile(RoutingMode::Rule)
        .map_err(RouteSettingsStoreError::InvalidStoredSettings)?;
    Ok(RouteSchemeBundle {
        active_scheme_id: DEFAULT_ROUTE_SCHEME_ID.to_owned(),
        schemes: vec![RouteScheme {
            id: DEFAULT_ROUTE_SCHEME_ID.to_owned(),
            name: "默认".to_owned(),
            settings: legacy,
        }],
    })
}
