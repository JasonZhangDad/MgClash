use std::collections::HashSet;
use std::path::Path;

use magies_domain::{
    CredentialRef, NodeModelError, ProxyNode, ProxyProtocol, Subscription, SubscriptionModelError,
    TimestampMillis,
};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::SubscriptionValidators;

#[derive(Debug)]
pub struct SubscriptionUpdate {
    subscription_id: Uuid,
    nodes: Vec<ProxyNode>,
    validators: SubscriptionValidators,
    updated_at: TimestampMillis,
}

impl SubscriptionUpdate {
    /// Creates a validated temporary node collection for one subscription.
    ///
    /// # Errors
    ///
    /// Returns a typed error when a node belongs to a different subscription
    /// or when the collection repeats a node UUID.
    pub fn new(
        subscription_id: Uuid,
        mut nodes: Vec<ProxyNode>,
        validators: SubscriptionValidators,
        updated_at: TimestampMillis,
    ) -> Result<Self, SubscriptionTransactionError> {
        let mut node_ids = HashSet::with_capacity(nodes.len());
        for node in &mut nodes {
            if let Some(actual_subscription_id) = node.subscription_id {
                if actual_subscription_id != subscription_id {
                    return Err(
                        SubscriptionTransactionError::NodeBelongsToAnotherSubscription {
                            node_id: node.id,
                            expected_subscription_id: subscription_id,
                            actual_subscription_id,
                        },
                    );
                }
            }
            if !node_ids.insert(node.id) {
                return Err(SubscriptionTransactionError::DuplicateNodeId { node_id: node.id });
            }
            node.subscription_id = Some(subscription_id);
        }

        Ok(Self {
            subscription_id,
            nodes,
            validators,
            updated_at,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubscriptionCommit {
    pub removed_nodes: usize,
    pub inserted_nodes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionState {
    pub validators: SubscriptionValidators,
    pub last_updated_at: Option<TimestampMillis>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeletedSubscription {
    pub subscription: Subscription,
    pub nodes: Vec<ProxyNode>,
}

#[derive(Debug)]
pub struct SqliteSubscriptionStore {
    connection: Connection,
}

impl SqliteSubscriptionStore {
    /// Opens or creates a `SQLite` database and initializes the subscription schema.
    ///
    /// # Errors
    ///
    /// Returns a database error when the path cannot be opened or initialized.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SubscriptionTransactionError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated in-memory `SQLite` subscription store.
    ///
    /// # Errors
    ///
    /// Returns a database error when `SQLite` cannot initialize the schema.
    pub fn open_in_memory() -> Result<Self, SubscriptionTransactionError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    /// Inserts a subscription before its nodes are updated.
    ///
    /// # Errors
    ///
    /// Returns a database error when the subscription cannot be inserted.
    pub fn insert_subscription(
        &self,
        subscription: &Subscription,
    ) -> Result<(), SubscriptionTransactionError> {
        self.connection.execute(
            "INSERT INTO subscriptions (
                id, name, url_secret_ref, auto_update, update_interval,
                etag, last_modified, last_updated_at, enabled,
                user_agent, include_keywords, exclude_keywords
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                subscription.id.to_string(),
                subscription.name.as_str(),
                subscription.url_secret_ref.as_str(),
                subscription.auto_update,
                i64::from(subscription.update_interval_minutes.get()),
                subscription.etag,
                subscription.last_modified,
                subscription.last_updated_at.map(TimestampMillis::get),
                subscription.enabled,
                subscription.user_agent,
                subscription.include_keywords,
                subscription.exclude_keywords,
            ],
        )?;
        Ok(())
    }

    /// Loads every subscription in insertion order.
    ///
    /// # Errors
    ///
    /// Returns a typed database or model error when a row cannot be read.
    pub fn subscriptions(&self) -> Result<Vec<Subscription>, SubscriptionTransactionError> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, url_secret_ref, auto_update, update_interval,
                    etag, last_modified, last_updated_at, enabled,
                    user_agent, include_keywords, exclude_keywords
             FROM subscriptions ORDER BY rowid",
        )?;
        let mut rows = statement.query([])?;
        let mut subscriptions = Vec::new();
        while let Some(row) = rows.next()? {
            subscriptions.push(decode_subscription(row)?);
        }
        Ok(subscriptions)
    }

    /// Loads one subscription by identifier.
    ///
    /// # Errors
    ///
    /// Returns a typed database or model error when a row cannot be read.
    pub fn subscription(
        &self,
        id: Uuid,
    ) -> Result<Option<Subscription>, SubscriptionTransactionError> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, url_secret_ref, auto_update, update_interval,
                    etag, last_modified, last_updated_at, enabled,
                    user_agent, include_keywords, exclude_keywords
             FROM subscriptions WHERE id = ?1",
        )?;
        let mut rows = statement.query([id.to_string()])?;
        rows.next()?.map(decode_subscription).transpose()
    }

    /// Updates user-editable subscription settings without changing fetch state.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriptionTransactionError::SubscriptionNotFound`] when the
    /// subscription is absent, or a database error when the update fails.
    pub fn update_subscription_settings(
        &self,
        subscription: &Subscription,
    ) -> Result<(), SubscriptionTransactionError> {
        let updated = self.connection.execute(
            "UPDATE subscriptions
             SET name = ?1, auto_update = ?2, update_interval = ?3, enabled = ?4,
                 user_agent = ?5, include_keywords = ?6, exclude_keywords = ?7
             WHERE id = ?8",
            params![
                subscription.name.as_str(),
                subscription.auto_update,
                i64::from(subscription.update_interval_minutes.get()),
                subscription.enabled,
                subscription.user_agent,
                subscription.include_keywords,
                subscription.exclude_keywords,
                subscription.id.to_string(),
            ],
        )?;
        ensure_subscription_updated(updated, subscription.id)
    }

    /// Records a successful not-modified fetch without replacing its nodes.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriptionTransactionError::SubscriptionNotFound`] when the
    /// subscription is absent, or a database error when the update fails.
    pub fn touch_subscription(
        &self,
        id: Uuid,
        validators: &SubscriptionValidators,
        updated_at: TimestampMillis,
    ) -> Result<(), SubscriptionTransactionError> {
        let updated = self.connection.execute(
            "UPDATE subscriptions
             SET etag = ?1, last_modified = ?2, last_updated_at = ?3
             WHERE id = ?4",
            params![
                validators.etag(),
                validators.last_modified(),
                updated_at.get(),
                id.to_string(),
            ],
        )?;
        ensure_subscription_updated(updated, id)
    }

    /// Deletes a subscription and returns its metadata and cascaded nodes.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriptionTransactionError::SubscriptionNotFound`] when the
    /// subscription is absent. Database and row-decoding failures roll back.
    pub fn delete_subscription(
        &mut self,
        id: Uuid,
    ) -> Result<DeletedSubscription, SubscriptionTransactionError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let subscription = {
            let mut statement = transaction.prepare(
                "SELECT id, name, url_secret_ref, auto_update, update_interval,
                        etag, last_modified, last_updated_at, enabled,
                        user_agent, include_keywords, exclude_keywords
                 FROM subscriptions WHERE id = ?1",
            )?;
            let mut rows = statement.query([id.to_string()])?;
            rows.next()?
                .map(decode_subscription)
                .transpose()?
                .ok_or(SubscriptionTransactionError::SubscriptionNotFound { id })?
        };
        let nodes = {
            let mut statement = transaction.prepare(
                "SELECT id, name, protocol, server, port, credential_ref,
                        transport_json, tls_json, udp_enabled, subscription_id,
                        group_id, latency_ms, last_tested_at, enabled
                 FROM nodes WHERE subscription_id = ?1 ORDER BY id",
            )?;
            let mut rows = statement.query([id.to_string()])?;
            let mut nodes = Vec::new();
            while let Some(row) = rows.next()? {
                nodes.push(decode_node(row)?);
            }
            nodes
        };
        transaction.execute("DELETE FROM subscriptions WHERE id = ?1", [id.to_string()])?;
        transaction.commit()?;
        Ok(DeletedSubscription {
            subscription,
            nodes,
        })
    }

    /// Atomically replaces all nodes for a subscription and updates fetch metadata.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the subscription is missing or any database
    /// or serialization operation fails. All changes roll back on failure.
    pub fn apply_update(
        &mut self,
        update: &SubscriptionUpdate,
    ) -> Result<SubscriptionCommit, SubscriptionTransactionError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM subscriptions WHERE id = ?1)",
            [update.subscription_id.to_string()],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(SubscriptionTransactionError::SubscriptionNotFound {
                id: update.subscription_id,
            });
        }

        let removed_nodes = transaction.execute(
            "DELETE FROM nodes WHERE subscription_id = ?1",
            [update.subscription_id.to_string()],
        )?;
        {
            let mut statement = transaction.prepare_cached(
                "INSERT INTO nodes (
                    id, name, protocol, server, port, credential_ref,
                    transport_json, tls_json, udp_enabled, subscription_id,
                    group_id, latency_ms, last_tested_at, enabled,
                    created_at, updated_at
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                    ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16
                 )",
            )?;
            for node in &update.nodes {
                let transport = encode_json(node.transport.as_ref(), "transport")?;
                let tls = encode_json(node.tls.as_ref(), "tls")?;
                statement.execute(params![
                    node.id.to_string(),
                    node.name.as_str(),
                    protocol_name(node.protocol_type),
                    node.server.as_str(),
                    i64::from(node.port.get()),
                    node.credential_ref.as_str(),
                    transport,
                    tls,
                    node.udp_enabled,
                    update.subscription_id.to_string(),
                    node.group_id.map(|id| id.to_string()),
                    node.latency_ms.map(i64::from),
                    node.last_tested_at.map(TimestampMillis::get),
                    node.enabled,
                    update.updated_at.get(),
                    update.updated_at.get(),
                ])?;
            }
        }

        transaction.execute(
            "UPDATE subscriptions
             SET etag = ?1, last_modified = ?2, last_updated_at = ?3
             WHERE id = ?4",
            params![
                update.validators.etag(),
                update.validators.last_modified(),
                update.updated_at.get(),
                update.subscription_id.to_string(),
            ],
        )?;
        transaction.commit()?;

        Ok(SubscriptionCommit {
            removed_nodes,
            inserted_nodes: update.nodes.len(),
        })
    }

    /// Loads the nodes currently committed for one subscription.
    ///
    /// # Errors
    ///
    /// Returns a typed error when a database row is unreadable or corrupt.
    pub fn subscription_nodes(
        &self,
        subscription_id: Uuid,
    ) -> Result<Vec<ProxyNode>, SubscriptionTransactionError> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, protocol, server, port, credential_ref,
                    transport_json, tls_json, udp_enabled, subscription_id,
                    group_id, latency_ms, last_tested_at, enabled
             FROM nodes WHERE subscription_id = ?1 ORDER BY id",
        )?;
        let mut rows = statement.query([subscription_id.to_string()])?;
        let mut nodes = Vec::new();
        while let Some(row) = rows.next()? {
            nodes.push(decode_node(row)?);
        }
        Ok(nodes)
    }

    /// Loads every node owned by an enabled subscription, including disabled
    /// nodes so the UI can show and re-enable them.
    ///
    /// # Errors
    ///
    /// Returns a typed error when a database row is unreadable or corrupt.
    pub fn listed_nodes(&self) -> Result<Vec<ProxyNode>, SubscriptionTransactionError> {
        let mut statement = self.connection.prepare(
            "SELECT n.id, n.name, n.protocol, n.server, n.port, n.credential_ref,
                    n.transport_json, n.tls_json, n.udp_enabled, n.subscription_id,
                    n.group_id, n.latency_ms, n.last_tested_at, n.enabled
             FROM nodes n
             JOIN subscriptions s ON s.id = n.subscription_id
             WHERE s.enabled = 1
             ORDER BY s.rowid, n.rowid",
        )?;
        let mut rows = statement.query([])?;
        let mut nodes = Vec::new();
        while let Some(row) = rows.next()? {
            nodes.push(decode_node(row)?);
        }
        Ok(nodes)
    }

    /// Loads every enabled node owned by an enabled subscription.
    ///
    /// # Errors
    ///
    /// Returns a typed error when a database row is unreadable or corrupt.
    pub fn active_nodes(&self) -> Result<Vec<ProxyNode>, SubscriptionTransactionError> {
        let mut statement = self.connection.prepare(
            "SELECT n.id, n.name, n.protocol, n.server, n.port, n.credential_ref,
                    n.transport_json, n.tls_json, n.udp_enabled, n.subscription_id,
                    n.group_id, n.latency_ms, n.last_tested_at, n.enabled
             FROM nodes n
             JOIN subscriptions s ON s.id = n.subscription_id
             WHERE n.enabled = 1 AND s.enabled = 1
             ORDER BY s.rowid, n.rowid",
        )?;
        let mut rows = statement.query([])?;
        let mut nodes = Vec::new();
        while let Some(row) = rows.next()? {
            nodes.push(decode_node(row)?);
        }
        Ok(nodes)
    }

    /// Sets whether a subscription-owned node may be selected or connected.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriptionTransactionError::NodeNotFound`] when `id` is
    /// absent, or a typed database error when the update cannot be stored.
    pub fn set_node_enabled(
        &self,
        id: Uuid,
        enabled: bool,
    ) -> Result<ProxyNode, SubscriptionTransactionError> {
        let mut node = {
            let mut statement = self.connection.prepare(
                "SELECT id, name, protocol, server, port, credential_ref,
                        transport_json, tls_json, udp_enabled, subscription_id,
                        group_id, latency_ms, last_tested_at, enabled
                 FROM nodes WHERE id = ?1",
            )?;
            let mut rows = statement.query([id.to_string()])?;
            rows.next()?
                .map(decode_node)
                .transpose()?
                .ok_or(SubscriptionTransactionError::NodeNotFound { id })?
        };
        self.connection.execute(
            "UPDATE nodes SET enabled = ?1 WHERE id = ?2",
            params![i64::from(enabled), id.to_string()],
        )?;
        node.enabled = enabled;
        Ok(node)
    }

    /// Records the latest endpoint test without changing subscription
    /// ownership, enabled state, or selection.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriptionTransactionError::NodeNotFound`] when `id` is
    /// absent, or a typed database/row error when the update cannot be stored.
    pub fn update_node_latency(
        &self,
        id: Uuid,
        latency_ms: Option<u32>,
        tested_at: TimestampMillis,
    ) -> Result<ProxyNode, SubscriptionTransactionError> {
        let mut node = {
            let mut statement = self.connection.prepare(
                "SELECT id, name, protocol, server, port, credential_ref,
                        transport_json, tls_json, udp_enabled, subscription_id,
                        group_id, latency_ms, last_tested_at, enabled
                 FROM nodes WHERE id = ?1",
            )?;
            let mut rows = statement.query([id.to_string()])?;
            rows.next()?
                .map(decode_node)
                .transpose()?
                .ok_or(SubscriptionTransactionError::NodeNotFound { id })?
        };
        self.connection.execute(
            "UPDATE nodes SET latency_ms = ?1, last_tested_at = ?2 WHERE id = ?3",
            params![latency_ms.map(i64::from), tested_at.get(), id.to_string()],
        )?;
        node.latency_ms = latency_ms;
        node.last_tested_at = Some(tested_at);
        Ok(node)
    }

    /// Persists one enabled subscription node as the desktop selection.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriptionTransactionError::NodeNotFound`] when the node or
    /// its subscription is disabled or absent.
    pub fn select_node(&mut self, id: Uuid) -> Result<ProxyNode, SubscriptionTransactionError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let node = {
            let mut statement = transaction.prepare(
                "SELECT n.id, n.name, n.protocol, n.server, n.port, n.credential_ref,
                        n.transport_json, n.tls_json, n.udp_enabled, n.subscription_id,
                        n.group_id, n.latency_ms, n.last_tested_at, n.enabled
                 FROM nodes n
                 JOIN subscriptions s ON s.id = n.subscription_id
                 WHERE n.id = ?1 AND n.enabled = 1 AND s.enabled = 1",
            )?;
            let mut rows = statement.query([id.to_string()])?;
            rows.next()?
                .map(decode_node)
                .transpose()?
                .ok_or(SubscriptionTransactionError::NodeNotFound { id })?
        };
        transaction.execute(
            "INSERT INTO subscription_node_selection (singleton, node_id)
             VALUES (1, ?1)
             ON CONFLICT(singleton) DO UPDATE SET node_id = excluded.node_id",
            [id.to_string()],
        )?;
        transaction.commit()?;
        Ok(node)
    }

    /// Loads the selected enabled subscription node, if it still exists.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the selected database row is unreadable.
    pub fn selected_node(&self) -> Result<Option<ProxyNode>, SubscriptionTransactionError> {
        let mut statement = self.connection.prepare(
            "SELECT n.id, n.name, n.protocol, n.server, n.port, n.credential_ref,
                    n.transport_json, n.tls_json, n.udp_enabled, n.subscription_id,
                    n.group_id, n.latency_ms, n.last_tested_at, n.enabled
             FROM subscription_node_selection selected
             JOIN nodes n ON n.id = selected.node_id
             JOIN subscriptions s ON s.id = n.subscription_id
             WHERE selected.singleton = 1 AND n.enabled = 1 AND s.enabled = 1",
        )?;
        let mut rows = statement.query([])?;
        rows.next()?.map(decode_node).transpose()
    }

    /// Clears the persisted subscription-node selection.
    ///
    /// # Errors
    ///
    /// Returns a database error when the selection cannot be removed.
    pub fn clear_selected_node(&self) -> Result<(), SubscriptionTransactionError> {
        self.connection
            .execute("DELETE FROM subscription_node_selection", [])?;
        Ok(())
    }

    /// Loads the validators and last successful update time for a subscription.
    ///
    /// # Errors
    ///
    /// Returns a database error when the state cannot be read.
    pub fn subscription_state(
        &self,
        subscription_id: Uuid,
    ) -> Result<Option<SubscriptionState>, SubscriptionTransactionError> {
        self.connection
            .query_row(
                "SELECT etag, last_modified, last_updated_at
                 FROM subscriptions WHERE id = ?1",
                [subscription_id.to_string()],
                |row| {
                    Ok(SubscriptionState {
                        validators: SubscriptionValidators::new(row.get(0)?, row.get(1)?),
                        last_updated_at: row.get::<_, Option<i64>>(2)?.map(TimestampMillis::new),
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    fn from_connection(connection: Connection) -> Result<Self, SubscriptionTransactionError> {
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS subscriptions (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                url_secret_ref TEXT NOT NULL,
                auto_update INTEGER NOT NULL CHECK (auto_update IN (0, 1)),
                update_interval INTEGER NOT NULL CHECK (update_interval > 0),
                etag TEXT,
                last_modified TEXT,
                last_updated_at INTEGER,
                enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
                user_agent TEXT,
                include_keywords TEXT NOT NULL DEFAULT '',
                exclude_keywords TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                protocol TEXT NOT NULL,
                server TEXT NOT NULL,
                port INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
                credential_ref TEXT NOT NULL,
                transport_json TEXT,
                tls_json TEXT,
                udp_enabled INTEGER NOT NULL CHECK (udp_enabled IN (0, 1)),
                subscription_id TEXT NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
                group_id TEXT,
                latency_ms INTEGER,
                last_tested_at INTEGER,
                enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS nodes_subscription_id
             ON nodes(subscription_id);
             CREATE TABLE IF NOT EXISTS subscription_node_selection (
                singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
                node_id TEXT NOT NULL
             );",
        )?;
        // Existing installs keep the older column list; only a duplicate column
        // is tolerated so a second open after upgrade is a no-op.
        for migration in [
            "ALTER TABLE subscriptions ADD COLUMN user_agent TEXT;",
            "ALTER TABLE subscriptions ADD COLUMN include_keywords TEXT NOT NULL DEFAULT '';",
            "ALTER TABLE subscriptions ADD COLUMN exclude_keywords TEXT NOT NULL DEFAULT '';",
        ] {
            if let Err(error) = connection.execute_batch(migration)
                && !error.to_string().contains("duplicate column")
            {
                return Err(error.into());
            }
        }
        Ok(Self { connection })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SubscriptionTransactionError {
    #[error(
        "node {node_id} belongs to subscription {actual_subscription_id}, not {expected_subscription_id}"
    )]
    NodeBelongsToAnotherSubscription {
        node_id: Uuid,
        expected_subscription_id: Uuid,
        actual_subscription_id: Uuid,
    },
    #[error("subscription update contains duplicate node ID {node_id}")]
    DuplicateNodeId { node_id: Uuid },
    #[error("subscription {id} does not exist")]
    SubscriptionNotFound { id: Uuid },
    #[error("enabled subscription node {id} does not exist")]
    NodeNotFound { id: Uuid },
    #[error("subscription database operation failed")]
    Database { source: rusqlite::Error },
    #[error("failed to serialize subscription node {field}")]
    SerializeNodeField {
        field: &'static str,
        source: serde_json::Error,
    },
    #[error("stored subscription node has an invalid UUID in {column}")]
    InvalidStoredUuid {
        column: &'static str,
        source: uuid::Error,
    },
    #[error("stored subscription node has unsupported protocol {value}")]
    InvalidStoredProtocol { value: String },
    #[error("stored subscription node failed domain validation")]
    InvalidStoredNode { source: NodeModelError },
    #[error("stored subscription failed domain validation")]
    InvalidStoredSubscription { source: SubscriptionModelError },
    #[error("stored subscription node has invalid JSON in {column}")]
    InvalidStoredJson {
        column: &'static str,
        source: serde_json::Error,
    },
}

fn ensure_subscription_updated(
    updated: usize,
    id: Uuid,
) -> Result<(), SubscriptionTransactionError> {
    if updated == 0 {
        Err(SubscriptionTransactionError::SubscriptionNotFound { id })
    } else {
        Ok(())
    }
}

fn decode_subscription(row: &Row<'_>) -> Result<Subscription, SubscriptionTransactionError> {
    let id = decode_uuid(&row.get::<_, String>(0)?, "id")?;
    let url_secret_ref = CredentialRef::new(row.get::<_, String>(2)?)
        .map_err(|source| SubscriptionTransactionError::InvalidStoredNode { source })?;
    let mut subscription = Subscription::new(
        id,
        row.get::<_, String>(1)?,
        url_secret_ref,
        row.get::<_, u32>(4)?,
    )
    .map_err(|source| SubscriptionTransactionError::InvalidStoredSubscription { source })?;
    subscription.auto_update = row.get(3)?;
    subscription.etag = row.get(5)?;
    subscription.last_modified = row.get(6)?;
    subscription.last_updated_at = row.get::<_, Option<i64>>(7)?.map(TimestampMillis::new);
    subscription.enabled = row.get(8)?;
    subscription.user_agent = row.get(9)?;
    subscription.include_keywords = row.get::<_, Option<String>>(10)?.unwrap_or_default();
    subscription.exclude_keywords = row.get::<_, Option<String>>(11)?.unwrap_or_default();
    Ok(subscription)
}

impl From<rusqlite::Error> for SubscriptionTransactionError {
    fn from(source: rusqlite::Error) -> Self {
        Self::Database { source }
    }
}

fn encode_json<T: Serialize>(
    value: Option<&T>,
    field: &'static str,
) -> Result<Option<String>, SubscriptionTransactionError> {
    value
        .map(|value| {
            serde_json::to_string(value).map_err(|source| {
                SubscriptionTransactionError::SerializeNodeField { field, source }
            })
        })
        .transpose()
}

fn decode_node(row: &Row<'_>) -> Result<ProxyNode, SubscriptionTransactionError> {
    let id_text: String = row.get(0)?;
    let id = decode_uuid(&id_text, "id")?;
    let protocol = decode_protocol(&row.get::<_, String>(2)?)?;
    let credential_ref = CredentialRef::new(row.get::<_, String>(5)?)
        .map_err(|source| SubscriptionTransactionError::InvalidStoredNode { source })?;
    let mut node = ProxyNode::new(
        id,
        row.get::<_, String>(1)?,
        protocol,
        row.get::<_, String>(3)?,
        row.get::<_, u32>(4)?,
        Some(credential_ref),
    )
    .map_err(|source| SubscriptionTransactionError::InvalidStoredNode { source })?;
    node.transport = decode_json(row.get(6)?, "transport_json")?;
    node.tls = decode_json(row.get(7)?, "tls_json")?;
    node.udp_enabled = row.get(8)?;
    let subscription_id: String = row.get(9)?;
    node.subscription_id = Some(decode_uuid(&subscription_id, "subscription_id")?);
    node.group_id = row
        .get::<_, Option<String>>(10)?
        .map(|value| decode_uuid(&value, "group_id"))
        .transpose()?;
    node.latency_ms = row.get(11)?;
    node.last_tested_at = row.get::<_, Option<i64>>(12)?.map(TimestampMillis::new);
    node.enabled = row.get(13)?;
    Ok(node)
}

fn decode_uuid(value: &str, column: &'static str) -> Result<Uuid, SubscriptionTransactionError> {
    Uuid::parse_str(value)
        .map_err(|source| SubscriptionTransactionError::InvalidStoredUuid { column, source })
}

fn protocol_name(protocol: ProxyProtocol) -> &'static str {
    match protocol {
        ProxyProtocol::Vless => "vless",
        ProxyProtocol::Vmess => "vmess",
        ProxyProtocol::Trojan => "trojan",
        ProxyProtocol::Shadowsocks => "shadowsocks",
        ProxyProtocol::Hysteria2 => "hysteria2",
        ProxyProtocol::Tuic => "tuic",
    }
}

fn decode_protocol(value: &str) -> Result<ProxyProtocol, SubscriptionTransactionError> {
    match value {
        "vless" => Ok(ProxyProtocol::Vless),
        "vmess" => Ok(ProxyProtocol::Vmess),
        "trojan" => Ok(ProxyProtocol::Trojan),
        "shadowsocks" => Ok(ProxyProtocol::Shadowsocks),
        "hysteria2" => Ok(ProxyProtocol::Hysteria2),
        "tuic" => Ok(ProxyProtocol::Tuic),
        _ => Err(SubscriptionTransactionError::InvalidStoredProtocol {
            value: value.to_owned(),
        }),
    }
}

fn decode_json<T: DeserializeOwned>(
    value: Option<String>,
    column: &'static str,
) -> Result<Option<T>, SubscriptionTransactionError> {
    value
        .map(|value| {
            serde_json::from_str(&value).map_err(|source| {
                SubscriptionTransactionError::InvalidStoredJson { column, source }
            })
        })
        .transpose()
}
