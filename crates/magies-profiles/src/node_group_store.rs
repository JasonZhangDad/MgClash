use std::collections::HashMap;
use std::path::Path;

use magies_domain::ProxyNode;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// How a named group chooses a node when the user connects one of its members.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeGroupStrategy {
    /// The selected node is the outbound. This is the v2rayN SELECT group.
    #[default]
    Select,
    /// Core measures members and keeps the lowest-latency one. v2rayN URL-TEST.
    UrlTest,
    /// Tries members in list order until one responds. v2rayN FALLBACK.
    Fallback,
    /// Distributes traffic across members. v2rayN LOAD-BALANCE.
    LoadBalance,
}

impl NodeGroupStrategy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::UrlTest => "urltest",
            Self::Fallback => "fallback",
            Self::LoadBalance => "loadbalance",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "urltest" => Self::UrlTest,
            "fallback" => Self::Fallback,
            "loadbalance" | "load-balance" | "load_balance" => Self::LoadBalance,
            _ => Self::Select,
        }
    }

    #[must_use]
    pub const fn uses_group_outbound(self) -> bool {
        !matches!(self, Self::Select)
    }

    #[must_use]
    pub const fn uses_observatory(self) -> bool {
        matches!(self, Self::UrlTest | Self::Fallback)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeGroup {
    pub id: Uuid,
    pub name: String,
    pub strategy: NodeGroupStrategy,
}

/// One named group and the node ids assigned to it, for profile export/import.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeGroupSnapshot {
    pub id: Uuid,
    pub name: String,
    pub strategy: NodeGroupStrategy,
    pub node_ids: Vec<Uuid>,
}

/// Persists named node groups independently from subscription snapshots.
#[derive(Debug)]
pub struct SqliteNodeGroupStore {
    connection: Connection,
}

impl SqliteNodeGroupStore {
    /// Opens or creates the node-group database.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when the file or schema cannot be opened.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, NodeGroupStoreError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated in-memory store for tests.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when the schema cannot be initialized.
    pub fn open_in_memory() -> Result<Self, NodeGroupStoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    /// Lists named groups in creation order.
    ///
    /// # Errors
    ///
    /// Returns a typed database or identifier error for unreadable rows.
    pub fn groups(&self) -> Result<Vec<NodeGroup>, NodeGroupStoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, name, strategy FROM node_groups ORDER BY rowid")?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .map(|row| {
                let (id, name, strategy) = row?;
                Ok(NodeGroup {
                    id: parse_id(&id)?,
                    name,
                    strategy: NodeGroupStrategy::parse(&strategy),
                })
            })
            .collect()
    }

    /// Loads one group by identifier.
    ///
    /// # Errors
    ///
    /// Returns a typed database or identifier error for an unreadable row.
    pub fn group(&self, id: Uuid) -> Result<Option<NodeGroup>, NodeGroupStoreError> {
        self.connection
            .query_row(
                "SELECT id, name, strategy FROM node_groups WHERE id = ?1",
                [id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .map(|(id, name, strategy)| {
                Ok(NodeGroup {
                    id: parse_id(&id)?,
                    name,
                    strategy: NodeGroupStrategy::parse(&strategy),
                })
            })
            .transpose()
    }

    /// Assigns a node to a trimmed named group, or clears its assignment.
    /// Existing names are reused so callers do not need a separate create flow.
    ///
    /// # Errors
    ///
    /// Rejects blank names before changing storage and returns a typed database
    /// or identifier error when the assignment cannot be committed.
    pub fn assign(
        &mut self,
        node_id: Uuid,
        name: Option<&str>,
    ) -> Result<Option<NodeGroup>, NodeGroupStoreError> {
        let name = name.map(str::trim);
        if name == Some("") {
            return Err(NodeGroupStoreError::EmptyName);
        }

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some(name) = name else {
            transaction.execute(
                "DELETE FROM node_group_assignments WHERE node_id = ?1",
                [node_id.to_string()],
            )?;
            transaction.commit()?;
            return Ok(None);
        };

        let existing = transaction
            .query_row(
                "SELECT id, strategy FROM node_groups WHERE name = ?1",
                [name],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let (group_id, strategy, is_new) = match existing {
            Some((id, strategy)) => (parse_id(&id)?, NodeGroupStrategy::parse(&strategy), false),
            None => (Uuid::new_v4(), NodeGroupStrategy::Select, true),
        };
        if is_new {
            transaction.execute(
                "INSERT INTO node_groups (id, name, strategy) VALUES (?1, ?2, ?3)",
                params![group_id.to_string(), name, strategy.as_str()],
            )?;
        }
        transaction.execute(
            "INSERT INTO node_group_assignments (node_id, group_id) VALUES (?1, ?2)
             ON CONFLICT(node_id) DO UPDATE SET group_id = excluded.group_id",
            params![node_id.to_string(), group_id.to_string()],
        )?;
        transaction.commit()?;
        Ok(Some(NodeGroup {
            id: group_id,
            name: name.to_owned(),
            strategy,
        }))
    }

    /// Changes how a group selects a node on connect.
    ///
    /// # Errors
    ///
    /// Returns [`NodeGroupStoreError::GroupNotFound`] when the id is unknown.
    pub fn set_strategy(
        &self,
        id: Uuid,
        strategy: NodeGroupStrategy,
    ) -> Result<NodeGroup, NodeGroupStoreError> {
        let updated = self.connection.execute(
            "UPDATE node_groups SET strategy = ?1 WHERE id = ?2",
            params![strategy.as_str(), id.to_string()],
        )?;
        if updated == 0 {
            return Err(NodeGroupStoreError::GroupNotFound { id });
        }
        self.group(id)?
            .ok_or(NodeGroupStoreError::GroupNotFound { id })
    }

    /// Exports every group and its member node ids in creation order.
    ///
    /// # Errors
    ///
    /// Returns a typed database or identifier error for unreadable rows.
    pub fn snapshot(&self) -> Result<Vec<NodeGroupSnapshot>, NodeGroupStoreError> {
        let groups = self.groups()?;
        let mut snapshots = Vec::with_capacity(groups.len());
        for group in groups {
            let mut statement = self.connection.prepare(
                "SELECT node_id FROM node_group_assignments WHERE group_id = ?1 ORDER BY rowid",
            )?;
            let node_ids = statement
                .query_map([group.id.to_string()], |row| row.get::<_, String>(0))?
                .map(|row| parse_id(&row?))
                .collect::<Result<Vec<_>, _>>()?;
            snapshots.push(NodeGroupSnapshot {
                id: group.id,
                name: group.name,
                strategy: group.strategy,
                node_ids,
            });
        }
        Ok(snapshots)
    }

    /// Replaces every group and assignment atomically.
    ///
    /// # Errors
    ///
    /// Returns a typed database or identifier error without a partial write.
    pub fn replace_all(
        &mut self,
        snapshots: &[NodeGroupSnapshot],
    ) -> Result<(), NodeGroupStoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute("DELETE FROM node_group_assignments", [])?;
        transaction.execute("DELETE FROM node_groups", [])?;
        for snapshot in snapshots {
            transaction.execute(
                "INSERT INTO node_groups (id, name, strategy) VALUES (?1, ?2, ?3)",
                params![
                    snapshot.id.to_string(),
                    snapshot.name,
                    snapshot.strategy.as_str()
                ],
            )?;
            for node_id in &snapshot.node_ids {
                transaction.execute(
                    "INSERT INTO node_group_assignments (node_id, group_id) VALUES (?1, ?2)",
                    params![node_id.to_string(), snapshot.id.to_string()],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// Applies saved group identifiers to the supplied nodes.
    ///
    /// # Errors
    ///
    /// Returns a typed database or identifier error without partially changing
    /// the nodes when assignments cannot be loaded.
    pub fn apply(&self, nodes: &mut [ProxyNode]) -> Result<(), NodeGroupStoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT node_id, group_id FROM node_group_assignments")?;
        let assignments = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .map(|row| {
                let (node_id, group_id) = row?;
                Ok((parse_id(&node_id)?, parse_id(&group_id)?))
            })
            .collect::<Result<HashMap<_, _>, NodeGroupStoreError>>()?;
        for node in nodes {
            node.group_id = assignments.get(&node.id).copied();
        }
        Ok(())
    }

    fn from_connection(connection: Connection) -> Result<Self, NodeGroupStoreError> {
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS node_groups (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL UNIQUE,
                strategy TEXT NOT NULL DEFAULT 'select'
             );
             CREATE TABLE IF NOT EXISTS node_group_assignments (
                node_id TEXT PRIMARY KEY NOT NULL,
                group_id TEXT NOT NULL REFERENCES node_groups(id)
             );",
        )?;
        if let Err(error) =
            connection.execute_batch("ALTER TABLE node_groups ADD COLUMN strategy TEXT NOT NULL DEFAULT 'select';")
            && !error.to_string().contains("duplicate column")
        {
            return Err(error.into());
        }
        Ok(Self { connection })
    }
}

fn parse_id(value: &str) -> Result<Uuid, NodeGroupStoreError> {
    Uuid::parse_str(value).map_err(|source| NodeGroupStoreError::InvalidId {
        value: value.to_owned(),
        source,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum NodeGroupStoreError {
    #[error("node group name must not be empty")]
    EmptyName,
    #[error("node group {id} was not found")]
    GroupNotFound { id: Uuid },
    #[error("node group database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("node group database contains invalid UUID {value}")]
    InvalidId {
        value: String,
        #[source]
        source: uuid::Error,
    },
}
