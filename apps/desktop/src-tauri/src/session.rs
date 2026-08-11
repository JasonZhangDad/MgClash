//! Desktop session state shared by the `MgClash` Tauri commands.
//!
//! The service keeps the UI free of Core JSON: it turns a sharing URI into the
//! shared node model, saves the credential in the OS store, and drives
//! [`DesktopSession`] for connect and disconnect.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::num::NonZeroU16;
use std::path::Path;
use std::time::Instant;

use magies_domain::{CredentialRef, NodeModelError, ProxyNode, ProxyProtocol, TimestampMillis};
use magies_profiles::{
    CredentialCodec, CredentialCodecError, DnsProfile, DnsServer, DnsStrategy, LocalHttpProfile,
    LocalSocksProfile, ManualNodeStoreError, ShareLinkParseError, ShareLinkParser,
    SqliteManualNodeStore, SqliteSubscriptionStore, SubscriptionTransactionError,
};
use magies_routing::{RouteProfile, RoutingMode};
use magies_session::{
    CoreSessionControl, DesktopSession, DesktopSessionError, DesktopSessionProfile, NetworkEvent,
    NetworkRecoveryPolicy, RecoveryError, RecoveryOutcome, SessionHealthProbe,
    SystemProxySessionControl,
};
use magies_storage::{SecretStore, SecretStoreError};
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::routing_mode::{
    RoutingModeStoreError, SqliteRoutingModeStore, route_profile_for, routing_mode_name,
};

/// The Core the V0.1 desktop shell drives.
const CORE_NAME: &str = "sing-box";

/// Fixed session settings the V0.1 shell does not let the user edit yet.
#[derive(Clone, Debug)]
pub struct SessionDefaults {
    pub socks: LocalSocksProfile,
    pub http: LocalHttpProfile,
    pub clash_api_port: NonZeroU16,
    pub dns: DnsProfile,
    pub route: RouteProfile,
    pub system_proxy: bool,
}

impl SessionDefaults {
    /// Builds the V0.1 defaults: loopback SOCKS/HTTP, the system resolver, and
    /// Global routing behind System Proxy.
    ///
    /// # Panics
    ///
    /// Panics only if these compile-time constants stop satisfying the DNS and
    /// routing validators, which the crates' own tests already cover.
    #[must_use]
    pub fn v01() -> Self {
        Self {
            socks: LocalSocksProfile::default(),
            http: LocalHttpProfile::default(),
            clash_api_port: NonZeroU16::new(9_090).expect("the Clash API port is nonzero"),
            dns: DnsProfile::new(
                vec![DnsServer::system("system").expect("\"system\" is a valid DNS server tag")],
                Vec::new(),
                "system",
                DnsStrategy::PreferIpv4,
                false,
                false,
            )
            .expect("the single system resolver is a valid DNS profile"),
            route: route_profile_for(RoutingMode::Global),
            system_proxy: true,
        }
    }

    fn mode(&self) -> &'static str {
        routing_mode_name(self.route.mode())
    }
}

/// The selected node as the dashboard renders it. Credentials never appear here.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSummary {
    pub id: Uuid,
    pub name: String,
    pub protocol: ProxyProtocol,
    pub server: String,
    pub port: u16,
    pub deletable: bool,
    pub latency_ms: Option<u32>,
    pub last_tested_at: Option<i64>,
}

impl From<&ProxyNode> for NodeSummary {
    fn from(node: &ProxyNode) -> Self {
        Self {
            id: node.id,
            name: node.name.as_str().to_owned(),
            protocol: node.protocol_type,
            server: node.server.as_str().to_owned(),
            port: node.port.get(),
            deletable: node.subscription_id.is_none(),
            latency_ms: node.latency_ms,
            last_tested_at: node.last_tested_at.map(TimestampMillis::get),
        }
    }
}

/// Everything the dashboard needs for one render.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    pub connected: bool,
    pub node: Option<NodeSummary>,
    pub core: &'static str,
    pub mode: &'static str,
    pub system_proxy: bool,
    pub socks_port: u16,
    pub http_port: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UrlTestTarget {
    pub node_id: Uuid,
    pub http_port: u16,
}

/// Owns the selected node and the orchestrated session behind the commands.
pub struct SessionService<S, C, P> {
    session: DesktopSession<S, C, P>,
    defaults: SessionDefaults,
    node: Option<ProxyNode>,
    manual_nodes: SqliteManualNodeStore,
    subscription_nodes: SqliteSubscriptionStore,
    routing_mode: SqliteRoutingModeStore,
    recovery: NetworkRecoveryPolicy,
}

impl<S, C, P> SessionService<S, C, P>
where
    S: SecretStore,
    C: CoreSessionControl,
    P: SystemProxySessionControl,
{
    /// Restores the selected subscription or manual node when constructing the service.
    ///
    /// # Errors
    ///
    /// Returns a typed node-store error when either persisted selection cannot
    /// be read.
    pub fn new(
        session: DesktopSession<S, C, P>,
        defaults: SessionDefaults,
        manual_nodes: SqliteManualNodeStore,
        subscription_nodes: SqliteSubscriptionStore,
        routing_mode: SqliteRoutingModeStore,
    ) -> Result<Self, SessionInitializationError> {
        let node = subscription_nodes
            .selected_node()?
            .or(manual_nodes.selected_node()?);
        let mut defaults = defaults;
        defaults.route = route_profile_for(routing_mode.load()?);
        Ok(Self {
            session,
            defaults,
            node,
            manual_nodes,
            subscription_nodes,
            routing_mode,
            recovery: NetworkRecoveryPolicy::default(),
        })
    }

    /// Records a network change or wake so the next [`Self::recover`] pass can
    /// act on it once the debounce window closes.
    pub fn observe_network(&mut self, event: NetworkEvent, now: Instant) {
        self.recovery.observe(event, now);
    }

    /// When the pending network event becomes actionable, if one is pending.
    #[must_use]
    pub const fn recovery_due_at(&self) -> Option<Instant> {
        self.recovery.due_at()
    }

    /// Runs one recovery pass, restarting the session only if `probe` says the
    /// Core stopped answering.
    ///
    /// # Errors
    ///
    /// Returns the recovery policy's typed error when the session could not be
    /// stopped or could not be restarted within its attempt budget.
    pub fn recover(
        &mut self,
        now: Instant,
        probe: &impl SessionHealthProbe,
    ) -> Result<RecoveryOutcome, RecoveryError<C::Error, P::Error>> {
        self.recovery.recover(now, &mut self.session, probe)
    }

    /// Parses a sharing URI, saves its credential, and selects the node.
    ///
    /// # Errors
    ///
    /// Returns a typed parse, credential, or secret store error. The previously
    /// selected node is kept when any step fails.
    pub fn import_node(
        &mut self,
        uri: &str,
    ) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        if self.session.is_running() {
            return Err(SessionCommandError::SessionActive);
        }
        let id = Uuid::new_v4();
        let credential_ref =
            CredentialRef::new(format!("node/{id}")).map_err(SessionCommandError::CredentialRef)?;
        let (node, credential) = ShareLinkParser
            .parse(uri, id, credential_ref)
            .map_err(SessionCommandError::ShareLink)?
            .into_parts();
        let secret =
            CredentialCodec::encode(&credential).map_err(SessionCommandError::Credential)?;
        self.session
            .secret_store()
            .put(&node.credential_ref, &secret)
            .map_err(SessionCommandError::Secret)?;

        if let Err(store) = self.manual_nodes.save_and_select(&node) {
            return match self.session.secret_store().delete(&node.credential_ref) {
                Ok(()) => Err(SessionCommandError::NodeStore(store)),
                Err(secret) => {
                    Err(SessionCommandError::NodeStoreAndSecretRollback { store, secret })
                }
            };
        }
        self.subscription_nodes
            .clear_selected_node()
            .map_err(SessionCommandError::SubscriptionNodeStore)?;

        self.node = Some(node);
        Ok(self.status())
    }

    /// Lists every persisted manual and active subscription node without
    /// exposing credentials.
    ///
    /// # Errors
    ///
    /// Returns a typed storage error when the node database is unreadable.
    pub fn nodes(&self) -> Result<Vec<NodeSummary>, SessionCommandError<C::Error, P::Error>> {
        let mut nodes = self
            .manual_nodes
            .nodes()
            .map_err(SessionCommandError::NodeStore)?;
        nodes.extend(
            self.subscription_nodes
                .active_nodes()
                .map_err(SessionCommandError::SubscriptionNodeStore)?,
        );
        Ok(nodes.iter().map(NodeSummary::from).collect())
    }

    /// Returns one active persisted node without exposing its credential.
    ///
    /// # Errors
    ///
    /// Returns a typed not-found error when `id` is not a manual or active
    /// subscription node, or a typed storage error when the database is unreadable.
    pub fn node(&self, id: Uuid) -> Result<NodeSummary, SessionCommandError<C::Error, P::Error>> {
        self.nodes()?
            .into_iter()
            .find(|node| node.id == id)
            .ok_or(SessionCommandError::NodeStore(
                ManualNodeStoreError::NodeNotFound { id },
            ))
    }

    /// Records the latest endpoint test without changing the selected node.
    ///
    /// # Errors
    ///
    /// Returns a typed not-found or storage error when the node cannot be updated.
    pub fn record_node_latency(
        &mut self,
        id: Uuid,
        latency_ms: Option<u32>,
        tested_at: TimestampMillis,
    ) -> Result<NodeSummary, SessionCommandError<C::Error, P::Error>> {
        let node = match self.manual_nodes.update_latency(id, latency_ms, tested_at) {
            Ok(node) => node,
            Err(ManualNodeStoreError::NodeNotFound { .. }) => self
                .subscription_nodes
                .update_node_latency(id, latency_ms, tested_at)
                .map_err(SessionCommandError::SubscriptionNodeStore)?,
            Err(error) => return Err(SessionCommandError::NodeStore(error)),
        };
        if self.node.as_ref().is_some_and(|selected| selected.id == id) {
            self.node = Some(node.clone());
        }
        Ok(NodeSummary::from(&node))
    }

    /// Selects a persisted node while the session is disconnected.
    ///
    /// # Errors
    ///
    /// Returns a typed error while connected, when the node is missing, or when
    /// the node database cannot be updated.
    pub fn select_node(
        &mut self,
        id: Uuid,
    ) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        if self.session.is_running() {
            return Err(SessionCommandError::SessionActive);
        }
        let node = match self.manual_nodes.select(id) {
            Ok(node) => {
                self.subscription_nodes
                    .clear_selected_node()
                    .map_err(SessionCommandError::SubscriptionNodeStore)?;
                node
            }
            Err(ManualNodeStoreError::NodeNotFound { .. }) => self
                .subscription_nodes
                .select_node(id)
                .map_err(SessionCommandError::SubscriptionNodeStore)?,
            Err(error) => return Err(SessionCommandError::NodeStore(error)),
        };
        self.node = Some(node);
        Ok(self.status())
    }

    /// Saves the routing mode used by the next connection.
    ///
    /// # Errors
    ///
    /// Returns a typed active-session or persistence error.
    pub fn set_routing_mode(
        &mut self,
        mode: RoutingMode,
    ) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        if self.session.is_running() {
            return Err(SessionCommandError::SessionActive);
        }
        self.routing_mode
            .save(mode)
            .map_err(SessionCommandError::RoutingModeStore)?;
        self.defaults.route = route_profile_for(mode);
        Ok(self.status())
    }

    /// Deletes a persisted node and its operating-system credential.
    ///
    /// # Errors
    ///
    /// Returns a typed error while connected, when the node is missing, or when
    /// storage cannot delete the node metadata or credential.
    pub fn delete_node(
        &mut self,
        id: Uuid,
    ) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        if self.session.is_running() {
            return Err(SessionCommandError::SessionActive);
        }
        let node = match self.manual_nodes.delete(id) {
            Ok(node) => node,
            Err(error @ ManualNodeStoreError::NodeNotFound { .. }) => {
                let is_subscription_node = self
                    .subscription_nodes
                    .active_nodes()
                    .map_err(SessionCommandError::SubscriptionNodeStore)?
                    .iter()
                    .any(|node| node.id == id);
                if is_subscription_node {
                    return Err(SessionCommandError::SubscriptionNodeReadOnly { id });
                }
                return Err(SessionCommandError::NodeStore(error));
            }
            Err(error) => return Err(SessionCommandError::NodeStore(error)),
        };
        if self.node.as_ref().is_some_and(|selected| selected.id == id) {
            self.node = None;
        }
        self.session
            .secret_store()
            .delete(&node.credential_ref)
            .map_err(SessionCommandError::DeleteSecret)?;
        Ok(self.status())
    }

    /// Reloads the persisted selection after subscription metadata or nodes
    /// change while the session is disconnected.
    ///
    /// # Errors
    ///
    /// Returns a typed storage error when either node database cannot be read.
    pub fn sync_selected_node(
        &mut self,
    ) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        if self.session.is_running() {
            return Err(SessionCommandError::SessionActive);
        }
        self.node = self
            .subscription_nodes
            .selected_node()
            .map_err(SessionCommandError::SubscriptionNodeStore)?
            .or(self
                .manual_nodes
                .selected_node()
                .map_err(SessionCommandError::NodeStore)?);
        Ok(self.status())
    }

    /// Starts the Core and System Proxy for the selected node.
    ///
    /// # Errors
    ///
    /// Returns [`SessionCommandError::NoSelectedNode`] before any node is
    /// imported, otherwise the orchestrator's typed error.
    pub fn connect(&mut self) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        let node = self
            .node
            .clone()
            .ok_or(SessionCommandError::NoSelectedNode)?;
        let profile = DesktopSessionProfile::new(
            node,
            self.defaults.dns.clone(),
            self.defaults.route.clone(),
        )
        .with_local_proxies(self.defaults.socks, self.defaults.http)
        .with_clash_api_port(self.defaults.clash_api_port)
        .with_system_proxy(self.defaults.system_proxy);

        self.session
            .start(&profile)
            .map_err(SessionCommandError::Session)?;
        Ok(self.status())
    }

    /// Restores System Proxy and stops the Core, keeping the selected node.
    ///
    /// # Errors
    ///
    /// Returns the orchestrator's typed error, including when no session is
    /// running.
    pub fn disconnect(&mut self) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        self.session.stop().map_err(SessionCommandError::Session)?;
        Ok(self.status())
    }

    /// The running session's on-disk config, for the diagnostic bundle.
    #[must_use]
    pub fn runtime_config_path(&self) -> Option<&Path> {
        self.session.config_path()
    }

    #[must_use]
    pub fn status(&self) -> SessionStatus {
        SessionStatus {
            connected: self.session.is_running(),
            node: self.node.as_ref().map(NodeSummary::from),
            core: CORE_NAME,
            mode: self.defaults.mode(),
            system_proxy: self.defaults.system_proxy,
            socks_port: self.defaults.socks.port().get(),
            http_port: self.defaults.http.port().get(),
        }
    }

    /// Returns the selected node and local HTTP proxy for a real URL test.
    ///
    /// # Errors
    ///
    /// Returns a typed inactive-session error until the Core and its local
    /// proxy are running.
    pub fn url_test_target(
        &self,
    ) -> Result<UrlTestTarget, SessionCommandError<C::Error, P::Error>> {
        if !self.session.is_running() {
            return Err(SessionCommandError::SessionInactive);
        }
        Ok(UrlTestTarget {
            node_id: self
                .node
                .as_ref()
                .ok_or(SessionCommandError::NoSelectedNode)?
                .id,
            http_port: self.defaults.http.port().get(),
        })
    }

    /// Returns the loopback API used for one-second Core traffic samples.
    ///
    /// # Errors
    ///
    /// Returns a typed inactive-session error until the Core is running.
    pub fn traffic_api_address(
        &self,
    ) -> Result<SocketAddr, SessionCommandError<C::Error, P::Error>> {
        if !self.session.is_running() {
            return Err(SessionCommandError::SessionInactive);
        }
        Ok(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            self.defaults.clash_api_port.get(),
        ))
    }
}

#[derive(Debug, Error)]
pub enum SessionInitializationError {
    #[error("failed to read the manual node selection")]
    ManualNodeStore(#[from] ManualNodeStoreError),
    #[error("failed to read the subscription node selection")]
    SubscriptionNodeStore(#[from] SubscriptionTransactionError),
    #[error("failed to load the routing mode")]
    RoutingModeStore(#[from] RoutingModeStoreError),
}

#[derive(Debug, Error)]
pub enum SessionCommandError<C, P>
where
    C: std::error::Error + 'static,
    P: std::error::Error + 'static,
{
    #[error("no node has been imported yet")]
    NoSelectedNode,
    #[error("failed to build the credential reference for the imported node")]
    CredentialRef(#[source] NodeModelError),
    #[error("failed to parse the sharing URI")]
    ShareLink(#[source] ShareLinkParseError),
    #[error("failed to encode the node credential")]
    Credential(#[source] CredentialCodecError),
    #[error("failed to save the node credential")]
    Secret(#[source] SecretStoreError),
    #[error("failed to change the manual node store")]
    NodeStore(#[source] ManualNodeStoreError),
    #[error("failed to change the subscription node store")]
    SubscriptionNodeStore(#[source] SubscriptionTransactionError),
    #[error("failed to save the routing mode")]
    RoutingModeStore(#[source] RoutingModeStoreError),
    #[error("subscription node {id} is managed by its subscription")]
    SubscriptionNodeReadOnly { id: Uuid },
    #[error("failed to save the node and roll back its credential")]
    NodeStoreAndSecretRollback {
        store: ManualNodeStoreError,
        secret: SecretStoreError,
    },
    #[error("failed to delete the node credential")]
    DeleteSecret(#[source] SecretStoreError),
    #[error("nodes cannot be changed while the session is connected")]
    SessionActive,
    #[error("this command requires a running proxy session")]
    SessionInactive,
    #[error("failed to change the desktop proxy session")]
    Session(#[source] DesktopSessionError<C, P>),
}

impl<C, P> SessionCommandError<C, P>
where
    C: std::error::Error + 'static,
    P: std::error::Error + 'static,
{
    /// The stable machine-readable code the UI branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NoSelectedNode => "no_selected_node",
            Self::CredentialRef(_) => "invalid_credential_reference",
            Self::ShareLink(_) => "invalid_share_link",
            Self::Credential(_) => "credential_encode_failed",
            Self::Secret(_) | Self::DeleteSecret(_) => "secret_store_failed",
            Self::NodeStore(ManualNodeStoreError::NodeNotFound { .. })
            | Self::SubscriptionNodeStore(SubscriptionTransactionError::NodeNotFound { .. }) => {
                "node_not_found"
            }
            Self::NodeStore(_)
            | Self::NodeStoreAndSecretRollback { .. }
            | Self::SubscriptionNodeStore(_) => "node_store_failed",
            Self::SubscriptionNodeReadOnly { .. } => "subscription_node_read_only",
            Self::RoutingModeStore(_) => "routing_mode_store_failed",
            Self::SessionActive => "session_active",
            Self::SessionInactive => "session_inactive",
            Self::Session(_) => "session_failed",
        }
    }
}
