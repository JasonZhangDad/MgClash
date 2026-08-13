use std::collections::{HashMap, HashSet};
use std::path::Path;

use magies_domain::ProxyNode;
use rusqlite::{Connection, TransactionBehavior, params};
use uuid::Uuid;

/// Persists one user-defined order across manual and subscription nodes.
#[derive(Debug)]
pub struct SqliteNodeOrderStore {
    connection: Connection,
}

impl SqliteNodeOrderStore {
    /// Opens or creates the node-order database.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when the file or schema cannot be opened.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, NodeOrderStoreError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated in-memory store for tests.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when the schema cannot be initialized.
    pub fn open_in_memory() -> Result<Self, NodeOrderStoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    /// Applies the saved order and appends previously unseen nodes stably.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when the saved positions cannot be read.
    pub fn order_nodes(
        &self,
        mut nodes: Vec<ProxyNode>,
    ) -> Result<Vec<ProxyNode>, NodeOrderStoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT node_id, position FROM node_order")?;
        let positions = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<Result<HashMap<_, _>, _>>()?;
        nodes.sort_by_key(|node| {
            positions
                .get(&node.id.to_string())
                .copied()
                .unwrap_or(i64::MAX)
        });
        Ok(nodes)
    }

    /// Returns the persisted node order without applying it to a node list.
    ///
    /// # Errors
    ///
    /// Returns a typed database or identifier error for unreadable rows.
    pub fn ordered_ids(&self) -> Result<Vec<Uuid>, NodeOrderStoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT node_id FROM node_order ORDER BY position")?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|row| {
                let value = row?;
                Uuid::parse_str(&value).map_err(|source| NodeOrderStoreError::InvalidId {
                    value,
                    source,
                })
            })
            .collect()
    }

    /// Atomically replaces the complete saved order.
    ///
    /// # Errors
    ///
    /// Rejects duplicate node IDs before changing storage and returns a typed
    /// database error when the new order cannot be committed.
    pub fn save(&mut self, ids: &[Uuid]) -> Result<(), NodeOrderStoreError> {
        let mut unique = HashSet::with_capacity(ids.len());
        for id in ids {
            if !unique.insert(*id) {
                return Err(NodeOrderStoreError::DuplicateNode { id: *id });
            }
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute("DELETE FROM node_order", [])?;
        for (position, id) in ids.iter().enumerate() {
            transaction.execute(
                "INSERT INTO node_order (node_id, position) VALUES (?1, ?2)",
                params![id.to_string(), position],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn from_connection(connection: Connection) -> Result<Self, NodeOrderStoreError> {
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS node_order (
                node_id TEXT PRIMARY KEY NOT NULL,
                position INTEGER NOT NULL CHECK (position >= 0)
             );",
        )?;
        Ok(Self { connection })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NodeOrderStoreError {
    #[error("node order database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("node {id} appears more than once in the saved order")]
    DuplicateNode { id: Uuid },
    #[error("node reorder expected {expected} ids but received {actual}")]
    IncompleteReorder { expected: usize, actual: usize },
    #[error("node order database contains invalid UUID {value}")]
    InvalidId {
        value: String,
        #[source]
        source: uuid::Error,
    },
}
