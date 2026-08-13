//! Persisted desktop DNS settings and their validated Core profile.

use std::path::Path;

use magies_profiles::{
    DnsConfigError, DnsProfile, DnsRule, DnsServer, DnsStrategy, PlainDnsTransport,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const CREATE_DNS_SETTINGS_TABLE: &str = "
    CREATE TABLE IF NOT EXISTS dns_settings (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        settings_json TEXT NOT NULL
    );
";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DnsMode {
    #[default]
    System,
    PlainUdp,
    PlainTcp,
    Doh,
    Dot,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DesktopDnsStrategy {
    #[default]
    PreferIpv4,
    PreferIpv6,
    Ipv4Only,
    Ipv6Only,
}

impl From<DesktopDnsStrategy> for DnsStrategy {
    fn from(strategy: DesktopDnsStrategy) -> Self {
        match strategy {
            DesktopDnsStrategy::PreferIpv4 => Self::PreferIpv4,
            DesktopDnsStrategy::PreferIpv6 => Self::PreferIpv6,
            DesktopDnsStrategy::Ipv4Only => Self::Ipv4Only,
            DesktopDnsStrategy::Ipv6Only => Self::Ipv6Only,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DnsTemplate {
    #[default]
    Simple,
    Advanced,
}

/// The complete DNS settings exchanged with the desktop webview.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsSettings {
    pub mode: DnsMode,
    pub server: String,
    pub port: u16,
    pub doh_path: String,
    pub strategy: DesktopDnsStrategy,
    pub fake_ip_enabled: bool,
    pub ipv6_enabled: bool,
    pub system_domains: Vec<String>,
    #[serde(default)]
    pub bootstrap: String,
    #[serde(default)]
    pub hosts: String,
    #[serde(default)]
    pub template: DnsTemplate,
}

impl Default for DnsSettings {
    fn default() -> Self {
        Self {
            mode: DnsMode::System,
            server: "1.1.1.1".to_owned(),
            port: 53,
            doh_path: "/dns-query".to_owned(),
            strategy: DesktopDnsStrategy::PreferIpv4,
            fake_ip_enabled: false,
            ipv6_enabled: false,
            system_domains: Vec::new(),
            bootstrap: String::new(),
            hosts: String::new(),
            template: DnsTemplate::Simple,
        }
    }
}

impl DnsSettings {
    /// Builds the validated DNS profile used by the next Core session.
    ///
    /// # Errors
    ///
    /// Returns the profile generator's typed validation error.
    pub fn profile(&self) -> Result<DnsProfile, DnsConfigError> {
        let system = DnsServer::system("system")?;
        let mut servers = vec![system];
        let bootstrap_tag = match self.bootstrap.trim() {
            "" => "system".to_owned(),
            value if value.parse::<std::net::IpAddr>().is_ok() => {
                servers.push(DnsServer::plain(
                    "bootstrap",
                    PlainDnsTransport::Udp,
                    value,
                    53,
                    None,
                )?);
                "bootstrap".to_owned()
            }
            value => value.to_owned(),
        };
        let final_server = match self.mode {
            DnsMode::System => "system",
            DnsMode::PlainUdp | DnsMode::PlainTcp => {
                let transport = if self.mode == DnsMode::PlainUdp {
                    PlainDnsTransport::Udp
                } else {
                    PlainDnsTransport::Tcp
                };
                servers.push(DnsServer::plain(
                    "primary",
                    transport,
                    &self.server,
                    u32::from(self.port),
                    Some(&bootstrap_tag),
                )?);
                "primary"
            }
            DnsMode::Doh => {
                servers.push(DnsServer::doh(
                    "primary",
                    &self.server,
                    u32::from(self.port),
                    &self.doh_path,
                    &bootstrap_tag,
                )?);
                "primary"
            }
            DnsMode::Dot => {
                servers.push(DnsServer::dot(
                    "primary",
                    &self.server,
                    u32::from(self.port),
                    &bootstrap_tag,
                )?);
                "primary"
            }
        };
        let rules = if self.system_domains.is_empty() {
            Vec::new()
        } else {
            vec![DnsRule::domain_suffix(
                self.system_domains.clone(),
                "system",
            )?]
        };
        let host_entries = parse_hosts(&self.hosts)?;
        DnsProfile::with_hosts(
            servers,
            rules,
            final_server,
            self.strategy.into(),
            self.fake_ip_enabled,
            self.ipv6_enabled,
            host_entries,
        )
    }
}

fn parse_hosts(value: &str) -> Result<Vec<(String, std::net::IpAddr)>, DnsConfigError> {
    let mut entries = Vec::new();
    for line in value.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((domain, address)) = line.split_once([' ', '\t']) else {
            continue;
        };
        let address = address
            .trim()
            .parse()
            .map_err(|_| DnsConfigError::InvalidServerAddress)?;
        entries.push((domain.trim().to_owned(), address));
    }
    Ok(entries)
}

#[derive(Debug, Error)]
pub enum DnsSettingsStoreError {
    #[error("failed to access the DNS settings database")]
    Database(#[from] rusqlite::Error),
    #[error("failed to encode the DNS settings")]
    Encode(#[source] serde_json::Error),
    #[error("the DNS settings database contains invalid JSON")]
    InvalidStoredJson(#[source] serde_json::Error),
    #[error("the DNS settings database contains invalid settings")]
    InvalidStoredSettings(#[source] DnsConfigError),
    #[error("refusing to save invalid DNS settings")]
    InvalidSettings(#[from] DnsConfigError),
}

pub struct SqliteDnsSettingsStore {
    connection: Connection,
}

impl SqliteDnsSettingsStore {
    /// Opens or creates the DNS settings table in `path`.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when `SQLite` cannot be opened or migrated.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DnsSettingsStoreError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Opens an isolated store for tests.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when the in-memory schema cannot be created.
    pub fn open_in_memory() -> Result<Self, DnsSettingsStoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self, DnsSettingsStoreError> {
        connection.execute_batch(CREATE_DNS_SETTINGS_TABLE)?;
        Ok(Self { connection })
    }

    /// Loads saved settings, defaulting to System DNS on first launch.
    ///
    /// # Errors
    ///
    /// Returns a typed database or invalid-stored-value error.
    pub fn load(&self) -> Result<DnsSettings, DnsSettingsStoreError> {
        let value = self
            .connection
            .query_row(
                "SELECT settings_json FROM dns_settings WHERE id = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(value) = value else {
            return Ok(DnsSettings::default());
        };
        let settings: DnsSettings =
            serde_json::from_str(&value).map_err(DnsSettingsStoreError::InvalidStoredJson)?;
        settings
            .profile()
            .map_err(DnsSettingsStoreError::InvalidStoredSettings)?;
        Ok(settings)
    }

    /// Validates and saves settings for the next launch.
    ///
    /// # Errors
    ///
    /// Returns a typed validation, encoding, or database error.
    pub fn save(&self, settings: &DnsSettings) -> Result<(), DnsSettingsStoreError> {
        settings.profile()?;
        let value = serde_json::to_string(settings).map_err(DnsSettingsStoreError::Encode)?;
        self.connection.execute(
            "INSERT INTO dns_settings (id, settings_json) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET settings_json = excluded.settings_json",
            params![value],
        )?;
        Ok(())
    }
}
