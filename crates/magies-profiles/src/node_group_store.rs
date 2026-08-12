use std::collections::HashMap;
use std::path::Path;

use magies_domain::ProxyNode;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeGroup {
    pub id: Uuid,
    pub name: String,
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
            .prepare("SELECT id, name FROM node_groups ORDER BY rowid")?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .map(|row| {
                let (id, name) = row?;
                Ok(NodeGroup {
                    id: parse_id(&id)?,
                    name,
                })
            })
            .collect()
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

        let existing_id = transaction
            .query_row(
                "SELECT id FROM node_groups WHERE name = ?1",
                [name],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let (group_id, is_new) = match existing_id {
            Some(id) => (parse_id(&id)?, false),
            None => (Uuid::new_v4(), true),
        };
        if is_new {
            transaction.execute(
                "INSERT INTO node_groups (id, name) VALUES (?1, ?2)",
                params![group_id.to_string(), name],
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
        }))
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
                name TEXT NOT NULL UNIQUE
             );
             CREATE TABLE IF NOT EXISTS node_group_assignments (
                node_id TEXT PRIMARY KEY NOT NULL,
                group_id TEXT NOT NULL REFERENCES node_groups(id)
             );",
        )?;
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
    #[error("node group database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("node group database contains invalid UUID {value}")]
    InvalidId {
        value: String,
        #[source]
        source: uuid::Error,
    },
}
