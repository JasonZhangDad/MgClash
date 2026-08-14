//! Full profile export/import: preferences, manual nodes with credentials,
//! subscriptions with URLs, groups, and node order.
//!
//! Unlike [`PreferencesBundle`](crate::preferences_backup::PreferencesBundle),
//! this file contains **plaintext secrets**. Treat it like a password export.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use magies_domain::ProxyNode;
use magies_profiles::{DnsConfigError, NodeGroupSnapshot, StoredNodeCredential};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::app_settings::AppSettings;
use crate::dns_settings::DnsSettings;
use crate::route_settings::RouteSettings;
use crate::subscriptions::SubscriptionBackupEntry;

const PROFILE_SCHEMA: u32 = 1;

/// One manual node and its decoded credential for profile backup.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualNodeBackupEntry {
    pub node: ProxyNode,
    pub credential: StoredNodeCredential,
}

/// Manual nodes, groups, order, and selection — everything except subscriptions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileNodesData {
    pub manual_nodes: Vec<ManualNodeBackupEntry>,
    pub groups: Vec<NodeGroupSnapshot>,
    pub node_order: Vec<Uuid>,
    pub selected_node_id: Option<Uuid>,
}

/// Portable snapshot of the entire desktop profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileBundle {
    pub schema: u32,
    pub app: AppSettings,
    pub route: RouteSettings,
    pub dns: DnsSettings,
    pub manual_nodes: Vec<ManualNodeBackupEntry>,
    pub subscriptions: Vec<SubscriptionBackupEntry>,
    pub groups: Vec<NodeGroupSnapshot>,
    pub node_order: Vec<Uuid>,
    pub selected_node_id: Option<Uuid>,
}

impl ProfileBundle {
    #[must_use]
    pub fn new(
        app: AppSettings,
        route: RouteSettings,
        dns: DnsSettings,
        nodes: ProfileNodesData,
        subscriptions: Vec<SubscriptionBackupEntry>,
    ) -> Self {
        Self {
            schema: PROFILE_SCHEMA,
            app,
            route,
            dns,
            manual_nodes: nodes.manual_nodes,
            groups: nodes.groups,
            node_order: nodes.node_order,
            selected_node_id: nodes.selected_node_id,
            subscriptions,
        }
    }

    /// Writes the bundle into `directory` and returns the file it created.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the bundle cannot be serialized or written.
    pub fn write_to(&self, directory: &Path) -> Result<PathBuf, ProfileBackupError> {
        let contents = serde_json::to_vec_pretty(self)
            .map_err(|source| ProfileBackupError::Serialize { source })?;
        let path = directory.join(format!("mgclash-profile-{}.json", export_stamp()));
        fs::write(&path, contents).map_err(|source| ProfileBackupError::Write {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    }

    /// Reads and validates a previously exported profile file.
    ///
    /// # Errors
    ///
    /// Returns a typed read, parse, schema, or settings validation error.
    pub fn read_from(path: &Path) -> Result<Self, ProfileBackupError> {
        let contents = fs::read_to_string(path).map_err(|source| ProfileBackupError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let bundle: Self = serde_json::from_str(&contents)
            .map_err(|source| ProfileBackupError::InvalidJson { source })?;
        if bundle.schema != PROFILE_SCHEMA {
            return Err(ProfileBackupError::UnsupportedSchema {
                schema: bundle.schema,
            });
        }
        bundle
            .route
            .profile(magies_routing::RoutingMode::Rule)
            .map_err(ProfileBackupError::InvalidRoute)?;
        bundle
            .dns
            .profile()
            .map_err(ProfileBackupError::InvalidDns)?;
        for entry in &bundle.manual_nodes {
            if entry.node.subscription_id.is_some() {
                return Err(ProfileBackupError::SubscriptionManualNode { id: entry.node.id });
            }
        }
        Ok(bundle)
    }

    #[must_use]
    pub fn nodes_data(&self) -> ProfileNodesData {
        ProfileNodesData {
            manual_nodes: self.manual_nodes.clone(),
            groups: self.groups.clone(),
            node_order: self.node_order.clone(),
            selected_node_id: self.selected_node_id,
        }
    }
}

fn export_stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[derive(Debug, Error)]
pub enum ProfileBackupError {
    #[error("failed to serialize profile")]
    Serialize { source: serde_json::Error },
    #[error("failed to write profile to {}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("failed to read profile from {}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("profile file is not valid JSON")]
    InvalidJson { source: serde_json::Error },
    #[error("profile schema {schema} is not supported")]
    UnsupportedSchema { schema: u32 },
    #[error("profile file contains invalid route settings")]
    InvalidRoute(#[source] crate::route_settings::RouteSettingsError),
    #[error("profile file contains invalid DNS settings")]
    InvalidDns(#[source] DnsConfigError),
    #[error("profile manual node {id} incorrectly references a subscription")]
    SubscriptionManualNode { id: Uuid },
}

impl ProfileBackupError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Serialize { .. } | Self::Write { .. } => "profile_export_failed",
            Self::Read { .. }
            | Self::InvalidJson { .. }
            | Self::UnsupportedSchema { .. }
            | Self::SubscriptionManualNode { .. } => "profile_import_failed",
            Self::InvalidRoute(_) => "invalid_route_settings",
            Self::InvalidDns(_) => "invalid_dns_settings",
        }
    }
}
