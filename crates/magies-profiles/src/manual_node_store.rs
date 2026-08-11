use std::path::Path;

use magies_domain::ProxyNode;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use uuid::Uuid;

/// `SQLite` metadata storage for nodes imported from individual sharing links.
#[derive(Debug)]
pub struct SqliteManualNodeStore {
    connection: Connection,
}

impl SqliteManualNodeStore {
    /// Opens or creates the manual-node database.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when the file or schema cannot be opened.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ManualNodeStoreError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated in-memory store for tests.
    ///
    /// # Errors
    ///
    /// Returns a typed database error when the schema cannot be initialized.
    pub fn open_in_memory() -> Result<Self, ManualNodeStoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    /// Inserts or updates a manual node and makes it the sole selection.
    ///
    /// # Errors
    ///
    /// Rejects subscription-owned nodes and returns serialization or database
    /// errors without changing the existing selection.
    pub fn save_and_select(&mut self, node: &ProxyNode) -> Result<(), ManualNodeStoreError> {
        if node.subscription_id.is_some() {
            return Err(ManualNodeStoreError::SubscriptionNode { id: node.id });
        }
        let json = serde_json::to_string(node)
            .map_err(|source| ManualNodeStoreError::SerializeNode { source })?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "UPDATE manual_nodes SET selected = 0 WHERE selected = 1",
            [],
        )?;
        transaction.execute(
            "INSERT INTO manual_nodes (id, node_json, selected)
             VALUES (?1, ?2, 1)
             ON CONFLICT(id) DO UPDATE SET node_json = excluded.node_json, selected = 1",
            params![node.id.to_string(), json],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Loads every manual node in import order.
    ///
    /// # Errors
    ///
    /// Returns a typed database or deserialization error for unreadable rows.
    pub fn nodes(&self) -> Result<Vec<ProxyNode>, ManualNodeStoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT node_json FROM manual_nodes ORDER BY rowid")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| decode_node(&row?)).collect()
    }

    /// Loads the selected node, if one exists.
    ///
    /// # Errors
    ///
    /// Returns a typed database or deserialization error for an unreadable row.
    pub fn selected_node(&self) -> Result<Option<ProxyNode>, ManualNodeStoreError> {
        self.connection
            .query_row(
                "SELECT node_json FROM manual_nodes WHERE selected = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| decode_node(&json))
            .transpose()
    }

    /// Makes an existing node the sole selection.
    ///
    /// # Errors
    ///
    /// Returns [`ManualNodeStoreError::NodeNotFound`] without changing the
    /// current selection when `id` is absent.
    pub fn select(&mut self, id: Uuid) -> Result<ProxyNode, ManualNodeStoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let json = transaction
            .query_row(
                "SELECT node_json FROM manual_nodes WHERE id = ?1",
                [id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(ManualNodeStoreError::NodeNotFound { id })?;
        let node = decode_node(&json)?;
        transaction.execute(
            "UPDATE manual_nodes SET selected = 0 WHERE selected = 1",
            [],
        )?;
        transaction.execute(
            "UPDATE manual_nodes SET selected = 1 WHERE id = ?1",
            [id.to_string()],
        )?;
        transaction.commit()?;
        Ok(node)
    }

    /// Deletes and returns an existing node.
    ///
    /// # Errors
    ///
    /// Returns [`ManualNodeStoreError::NodeNotFound`] when `id` is absent, or
    /// a typed database/deserialization error for an unreadable row.
    pub fn delete(&mut self, id: Uuid) -> Result<ProxyNode, ManualNodeStoreError> {
        let json = self
            .connection
            .query_row(
                "SELECT node_json FROM manual_nodes WHERE id = ?1",
                [id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(ManualNodeStoreError::NodeNotFound { id })?;
        let node = decode_node(&json)?;
        self.connection
            .execute("DELETE FROM manual_nodes WHERE id = ?1", [id.to_string()])?;
        Ok(node)
    }

    fn from_connection(connection: Connection) -> Result<Self, ManualNodeStoreError> {
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS manual_nodes (
                id TEXT PRIMARY KEY NOT NULL,
                node_json TEXT NOT NULL,
                selected INTEGER NOT NULL CHECK (selected IN (0, 1))
             );
             CREATE UNIQUE INDEX IF NOT EXISTS manual_nodes_single_selection
             ON manual_nodes(selected) WHERE selected = 1;",
        )?;
        Ok(Self { connection })
    }
}

fn decode_node(json: &str) -> Result<ProxyNode, ManualNodeStoreError> {
    serde_json::from_str(json).map_err(|source| ManualNodeStoreError::DeserializeNode { source })
}

#[derive(Debug, thiserror::Error)]
pub enum ManualNodeStoreError {
    #[error("manual node database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("failed to serialize a manual node")]
    SerializeNode { source: serde_json::Error },
    #[error("stored manual node is invalid")]
    DeserializeNode { source: serde_json::Error },
    #[error("node {id} belongs to a subscription")]
    SubscriptionNode { id: Uuid },
    #[error("manual node {id} was not found")]
    NodeNotFound { id: Uuid },
}
