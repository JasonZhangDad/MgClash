//! Desktop session state shared by the `MgClash` Tauri commands.
//!
//! The service keeps the UI free of Core JSON: it turns a sharing URI into the
//! shared node model, saves the credential in the OS store, and drives
//! [`DesktopSession`] for connect and disconnect.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::num::NonZeroU16;
use std::path::Path;
use std::time::Instant;

use magies_core_runtime::{LocalProxyPortChecker, LocalProxyPortError};
use magies_domain::{
    CoreType, CredentialRef, NodeModelError, NodeName, ProxyNode, ProxyProtocol, ServerAddress,
    TimestampMillis, TlsConfig, TransportConfig,
};
use magies_platform::{CpuArchitecture, OperatingSystem};
use magies_profiles::{
    BulkImportError, BulkNodeImportParser, CoreCapabilityMatrix, CorePreference, CoreRequirements,
    CoreSelectionError, CredentialCodec, CredentialCodecError, DnsConfigError, DnsProfile,
    LocalHttpProfile, LocalProxyConfigError, LocalSocksProfile, ManualNodeDraft,
    ManualNodeDraftError, ManualNodeStoreError, NodeFingerprint, NodeGroup, NodeGroupStoreError,
    NodeOrderStoreError, ShareLinkParseError, ShareLinkParser, ShareLinkQrCode, ShareLinkQrError,
    ShareLinkSerializer, ShareLinkSerializerError, SqliteManualNodeStore, SqliteNodeGroupStore,
    SqliteNodeOrderStore, SqliteSubscriptionStore, StoredNodeCredential,
    SubscriptionTransactionError, TunProfile, TunProfileError, core_name, node_fingerprint,
};
use magies_routing::{RouteProfile, RoutingMode};
use magies_session::{
    CoreSessionControl, DesktopSession, DesktopSessionError, DesktopSessionProfile, NetworkEvent,
    NetworkRecoveryPolicy, RecoveryError, RecoveryOutcome, SessionHealthProbe, SystemProxyMode,
    SystemProxySessionControl,
};
use magies_storage::{SecretStore, SecretStoreError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::core_control::describe;
use crate::dns_settings::{DnsSettings, DnsSettingsStoreError, SqliteDnsSettingsStore};
use crate::route_settings::{
    RouteSettings, RouteSettingsError, RouteSettingsStoreError, SqliteRouteSettingsStore,
};
use crate::routing_mode::{
    RoutingModeStoreError, SqliteRoutingModeStore, route_profile_for, routing_mode_name,
};

/// The MTU sing-box documents as its TUN default.
const DEFAULT_TUN_MTU: u16 = 9_000;

/// The operating system this build runs on, or `None` outside the support
/// matrix.
fn host_operating_system() -> Option<OperatingSystem> {
    match std::env::consts::OS {
        "windows" => Some(OperatingSystem::Windows),
        "linux" => Some(OperatingSystem::Linux),
        "macos" => Some(OperatingSystem::MacOs),
        _ => None,
    }
}

/// The architecture this build runs on, for the Core capability matrix.
fn host_architecture() -> CpuArchitecture {
    if std::env::consts::ARCH == "aarch64" {
        CpuArchitecture::Aarch64
    } else {
        CpuArchitecture::X86_64
    }
}

/// Fixed session settings the V0.1 shell does not let the user edit yet.
#[derive(Clone, Debug)]
pub struct SessionDefaults {
    pub socks: LocalSocksProfile,
    pub http: LocalHttpProfile,
    pub clash_api_port: NonZeroU16,
    pub dns: DnsProfile,
    pub route: RouteProfile,
    pub system_proxy: SystemProxyMode,
    /// When true the next connect asks the Core for multiplex / mux.
    pub mux_enabled: bool,
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
            dns: DnsSettings::default()
                .profile()
                .expect("the default DNS settings are valid"),
            route: route_profile_for(RoutingMode::Global),
            system_proxy: SystemProxyMode::Managed,
            mux_enabled: false,
        }
    }

    fn mode(&self) -> &'static str {
        routing_mode_name(self.route.mode())
    }
}

/// What one bulk import produced, as the UI reports it back to the user.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkImportReport {
    pub imported: usize,
    /// Lines dropped because the same node appeared earlier in the same body.
    pub duplicates: usize,
    pub failures: Vec<BulkImportLineReport>,
    pub status: SessionStatus,
}

/// One line the import could not use, with the reason already flattened.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkImportLineReport {
    /// Absent when the failure belongs to no single line, such as a node that
    /// parsed but could not be persisted.
    pub line: Option<usize>,
    pub message: String,
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
    pub group_id: Option<Uuid>,
    /// The stream transport, as the node table shows it.
    pub transport: &'static str,
    /// Which TLS layer the node uses, or `None` for plaintext.
    pub tls: Option<&'static str>,
    pub deletable: bool,
    /// When false the node stays listed but cannot be selected or connected.
    pub enabled: bool,
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
            group_id: node.group_id,
            transport: transport_name(node.protocol_type, node.transport.as_ref()),
            tls: node.tls.as_ref().map(tls_name),
            deletable: node.subscription_id.is_none(),
            enabled: node.enabled,
            latency_ms: node.latency_ms,
            last_tested_at: node.last_tested_at.map(TimestampMillis::get),
        }
    }
}

/// The stable name the webview and the settings store share.
const fn system_proxy_mode_name(mode: &SystemProxyMode) -> &'static str {
    match mode {
        SystemProxyMode::Managed => "managed",
        SystemProxyMode::Pac(_) => "pac",
        SystemProxyMode::Cleared => "cleared",
        SystemProxyMode::Unchanged => "unchanged",
    }
}

/// Hysteria2 and TUIC carry their own QUIC transport, `WireGuard` is its own
/// tunnel, `AnyTLS` is TLS from the first byte, and `Naive` tunnels over HTTP/2
/// or QUIC; the model stores `None` for all of them, so the protocol decides
/// which label a missing transport gets.
const fn transport_name(protocol: ProxyProtocol, transport: Option<&TransportConfig>) -> &'static str {
    match transport {
        Some(TransportConfig::Tcp) => "tcp",
        Some(TransportConfig::WebSocket { .. }) => "ws",
        Some(TransportConfig::HttpUpgrade { .. }) => "httpupgrade",
        Some(TransportConfig::Grpc { .. }) => "grpc",
        Some(TransportConfig::XHttp { .. }) => "xhttp",
        None if matches!(protocol, ProxyProtocol::WireGuard) => "wireguard",
        None if matches!(protocol, ProxyProtocol::AnyTls) => "anytls",
        None if matches!(protocol, ProxyProtocol::Naive) => "naive",
        None => "quic",
    }
}

/// Whether the node verifies its server by a pinned digest.
///
/// Reality authenticates by public key instead, so it never carries a pin.
const fn pins_a_certificate(tls: Option<&TlsConfig>) -> bool {
    matches!(
        tls,
        Some(TlsConfig::Tls {
            pinned_sha256: Some(_),
            ..
        })
    )
}

/// The TLS layer as the node table labels it.
///
/// A pinned node is called out because it behaves differently: only Xray can
/// verify the digest, so the table showing a bare `tls` would hide the reason a
/// connection attempt refuses the Core the user picked.
const fn tls_name(tls: &TlsConfig) -> &'static str {
    match tls {
        TlsConfig::Tls {
            pinned_sha256: Some(_),
            ..
        } => "tls+pin",
        TlsConfig::Tls { .. } => "tls",
        TlsConfig::Reality { .. } => "reality",
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeGroupSummary {
    pub id: Uuid,
    pub name: String,
}

impl From<NodeGroup> for NodeGroupSummary {
    fn from(group: NodeGroup) -> Self {
        Self {
            id: group.id,
            name: group.name,
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
    pub dns: DnsSettings,
    pub mode: &'static str,
    pub route: RouteSettings,
    pub system_proxy: bool,
    /// Which of the three System Proxy modes the next session will use.
    pub system_proxy_mode: &'static str,
    pub socks_port: u16,
    pub http_port: u16,
    pub clash_api_port: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UrlTestTarget {
    pub node_id: Uuid,
    pub http_port: u16,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NodeMoveDirection {
    Up,
    Down,
}

/// The stores that together form the desktop's unified node list.
pub struct NodeStores {
    manual: SqliteManualNodeStore,
    subscription: SqliteSubscriptionStore,
    order: SqliteNodeOrderStore,
    groups: SqliteNodeGroupStore,
}

impl NodeStores {
    #[must_use]
    pub const fn new(
        manual: SqliteManualNodeStore,
        subscription: SqliteSubscriptionStore,
        order: SqliteNodeOrderStore,
        groups: SqliteNodeGroupStore,
    ) -> Self {
        Self {
            manual,
            subscription,
            order,
            groups,
        }
    }
}

/// Owns the selected node and the orchestrated session behind the commands.
pub struct SessionService<S, C, P>
where
    C: CoreSessionControl,
{
    session: DesktopSession<S, C, P>,
    defaults: SessionDefaults,
    node: Option<ProxyNode>,
    manual_nodes: SqliteManualNodeStore,
    subscription_nodes: SqliteSubscriptionStore,
    node_order: SqliteNodeOrderStore,
    node_groups: SqliteNodeGroupStore,
    routing_mode: SqliteRoutingModeStore,
    route_settings: SqliteRouteSettingsStore,
    current_route_settings: RouteSettings,
    dns_settings: SqliteDnsSettingsStore,
    current_dns_settings: DnsSettings,
    recovery: NetworkRecoveryPolicy,
    core_preference: CorePreference,
    tun_enabled: bool,
    /// The running Core's output, held until the shell claims it with
    /// [`SessionService::take_core_output`]. Dropping it would discard every
    /// line the Core prints, which is what happened before the log panel.
    core_output: Option<C::Output>,
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
        node_stores: NodeStores,
        routing_mode: SqliteRoutingModeStore,
        route_settings: SqliteRouteSettingsStore,
        dns_settings: SqliteDnsSettingsStore,
    ) -> Result<Self, SessionInitializationError> {
        let mut node = node_stores
            .subscription
            .selected_node()?
            .or(node_stores.manual.selected_node()?);
        if let Some(node) = &mut node {
            node_stores.groups.apply(std::slice::from_mut(node))?;
        }
        let mut defaults = defaults;
        let mode = routing_mode.load()?;
        let current_route_settings = route_settings.load()?;
        defaults.route = current_route_settings.profile(mode)?;
        let current_dns_settings = dns_settings.load()?;
        defaults.dns = current_dns_settings.profile()?;
        Ok(Self {
            session,
            defaults,
            node,
            manual_nodes: node_stores.manual,
            subscription_nodes: node_stores.subscription,
            node_order: node_stores.order,
            node_groups: node_stores.groups,
            routing_mode,
            route_settings,
            current_route_settings,
            dns_settings,
            current_dns_settings,
            recovery: NetworkRecoveryPolicy::default(),
            core_preference: CorePreference::default(),
            tun_enabled: false,
            core_output: None,
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

    /// Runs a due network recovery or detects and recovers a crashed Core.
    ///
    /// # Errors
    ///
    /// Returns the recovery policy's typed stop or bounded-restart error.
    pub fn monitor_recovery(
        &mut self,
        now: Instant,
        probe: &impl SessionHealthProbe,
    ) -> Result<RecoveryOutcome, RecoveryError<C::Error, P::Error>> {
        self.recovery.monitor(now, &mut self.session, probe)
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
        self.store_new_node(node, &credential)
    }

    /// Validates a manually entered node, saves its credential, and selects it.
    ///
    /// # Errors
    ///
    /// Returns a typed validation, credential, or secret store error. The
    /// previously selected node is kept when any step fails.
    pub fn create_node(
        &mut self,
        draft: ManualNodeDraft,
    ) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        if self.session.is_running() {
            return Err(SessionCommandError::SessionActive);
        }
        let id = Uuid::new_v4();
        let credential_ref =
            CredentialRef::new(format!("node/{id}")).map_err(SessionCommandError::CredentialRef)?;
        let (node, credential) = draft
            .build(id, credential_ref)
            .map_err(SessionCommandError::ManualNodeDraft)?;
        self.store_new_node(node, &credential)
    }

    /// Returns the full form draft for a manual node, including its credential.
    ///
    /// Subscription nodes stay read-only: their next refresh owns those fields.
    ///
    /// # Errors
    ///
    /// Returns a typed read-only, not-found, secret, or credential error.
    pub fn node_draft(
        &self,
        id: Uuid,
    ) -> Result<ManualNodeDraft, SessionCommandError<C::Error, P::Error>> {
        let node = self
            .manual_nodes
            .nodes()
            .map_err(SessionCommandError::NodeStore)?
            .into_iter()
            .find(|node| node.id == id);
        if node.is_none()
            && self
                .subscription_nodes
                .active_nodes()
                .map_err(SessionCommandError::SubscriptionNodeStore)?
                .iter()
                .any(|node| node.id == id)
        {
            return Err(SessionCommandError::SubscriptionNodeReadOnly { id });
        }
        let node = node.ok_or(SessionCommandError::NodeStore(
            ManualNodeStoreError::NodeNotFound { id },
        ))?;
        let secret = self
            .session
            .secret_store()
            .get(&node.credential_ref)
            .map_err(SessionCommandError::Secret)?;
        let credential =
            CredentialCodec::decode(&secret).map_err(SessionCommandError::Credential)?;
        Ok(ManualNodeDraft::from_stored(&node, &credential))
    }

    /// Replaces a manual node's endpoint, transport, TLS, and credential in place.
    ///
    /// Preserves id, group, latency history, and enabled flag. The secret is
    /// overwritten under the same credential reference.
    ///
    /// # Errors
    ///
    /// Returns a typed active-session, validation, read-only, not-found, or
    /// persistence error.
    pub fn update_node(
        &mut self,
        id: Uuid,
        draft: ManualNodeDraft,
    ) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        if self.session.is_running() {
            return Err(SessionCommandError::SessionActive);
        }
        let existing = self
            .manual_nodes
            .nodes()
            .map_err(SessionCommandError::NodeStore)?
            .into_iter()
            .find(|node| node.id == id);
        if existing.is_none()
            && self
                .subscription_nodes
                .active_nodes()
                .map_err(SessionCommandError::SubscriptionNodeStore)?
                .iter()
                .any(|node| node.id == id)
        {
            return Err(SessionCommandError::SubscriptionNodeReadOnly { id });
        }
        let existing = existing.ok_or(SessionCommandError::NodeStore(
            ManualNodeStoreError::NodeNotFound { id },
        ))?;
        let (mut node, credential) = draft
            .build(id, existing.credential_ref.clone())
            .map_err(SessionCommandError::ManualNodeDraft)?;
        node.group_id = existing.group_id;
        node.latency_ms = existing.latency_ms;
        node.last_tested_at = existing.last_tested_at;
        node.enabled = existing.enabled;
        let secret =
            CredentialCodec::encode(&credential).map_err(SessionCommandError::Credential)?;
        self.session
            .secret_store()
            .put(&node.credential_ref, &secret)
            .map_err(SessionCommandError::Secret)?;
        let mut node = self
            .manual_nodes
            .update(&node)
            .map_err(SessionCommandError::NodeStore)?;
        self.node_groups
            .apply(std::slice::from_mut(&mut node))
            .map_err(SessionCommandError::NodeGroupStore)?;
        if self.node.as_ref().is_some_and(|selected| selected.id == id) {
            self.node = Some(node);
        }
        Ok(self.status())
    }

    /// Imports every sharing link in a pasted or opened body.
    ///
    /// Unlike [`Self::import_node`], a line that fails to parse or persist is
    /// reported rather than aborting the batch, and an existing selection is
    /// preserved: pasting a list never moves the user off the node they chose.
    ///
    /// # Errors
    ///
    /// Returns a typed error only when the body as a whole is unreadable.
    pub fn import_nodes(
        &mut self,
        content: &[u8],
    ) -> Result<BulkImportReport, SessionCommandError<C::Error, P::Error>> {
        if self.session.is_running() {
            return Err(SessionCommandError::SessionActive);
        }
        let outcome = BulkNodeImportParser
            .parse(content)
            .map_err(SessionCommandError::BulkImport)?;

        // Parse failures already arrive in line order; persistence failures are
        // appended after them because they belong to no particular line.
        let mut failures: Vec<BulkImportLineReport> = outcome
            .failures
            .iter()
            .map(|failure| BulkImportLineReport {
                line: Some(failure.line),
                message: describe(&failure.reason),
            })
            .collect();
        let stored = self.stored_fingerprints();
        let mut duplicates = outcome.duplicates;
        let mut imported = Vec::new();
        for parsed in outcome.nodes {
            let (node, credential) = parsed.into_parts();
            // Skipping a node already stored is what makes re-importing the same
            // list idempotent rather than doubling it.
            if node_fingerprint(&node, &credential).is_some_and(|print| stored.contains(&print)) {
                duplicates += 1;
                continue;
            }
            match self.store_imported_node(&node, &credential) {
                Ok(()) => imported.push(node),
                Err(message) => failures.push(BulkImportLineReport {
                    line: None,
                    message,
                }),
            }
        }

        // Leave an existing choice alone, but do not strand the user with a
        // full list and nothing selected.
        if self.node.is_none()
            && let Some(first) = imported.first()
        {
            self.manual_nodes
                .save_and_select(first)
                .map_err(SessionCommandError::NodeStore)?;
            self.subscription_nodes
                .clear_selected_node()
                .map_err(SessionCommandError::SubscriptionNodeStore)?;
            self.node = Some(first.clone());
        }

        Ok(BulkImportReport {
            imported: imported.len(),
            duplicates,
            failures,
            status: self.status(),
        })
    }

    /// Fingerprints every node already stored, so an import can skip repeats.
    ///
    /// A node whose secret cannot be read is left out rather than failing the
    /// import: the worst outcome is a duplicate row, whereas refusing would
    /// block the user over one unrelated broken entry. The skip is logged so it
    /// stays visible instead of silently changing behaviour.
    fn stored_fingerprints(&self) -> HashSet<NodeFingerprint> {
        let Ok(nodes) = self.manual_nodes.nodes() else {
            tracing::warn!("could not read stored nodes; this import will not skip repeats");
            return HashSet::new();
        };
        nodes
            .into_iter()
            .filter_map(|node| {
                let secret = self.session.secret_store().get(&node.credential_ref).ok()?;
                let credential = CredentialCodec::decode(&secret).ok()?;
                node_fingerprint(&node, &credential)
            })
            .collect()
    }

    /// Persists one node of a batch, rolling its secret back on failure.
    ///
    /// Returns the message to report against the line instead of aborting the
    /// whole import.
    fn store_imported_node(
        &mut self,
        node: &ProxyNode,
        credential: &StoredNodeCredential,
    ) -> Result<(), String> {
        let secret = CredentialCodec::encode(credential).map_err(|error| describe(&error))?;
        self.session
            .secret_store()
            .put(&node.credential_ref, &secret)
            .map_err(|error| describe(&error))?;
        if let Err(store) = self.manual_nodes.save(node) {
            let message = describe(&store);
            return Err(
                match self.session.secret_store().delete(&node.credential_ref) {
                    Ok(()) => message,
                    Err(secret) => format!("{message}; {}", describe(&secret)),
                },
            );
        }
        Ok(())
    }

    /// Persists a freshly built node and its credential, rolling the secret back
    /// when the node store rejects it.
    fn store_new_node(
        &mut self,
        node: ProxyNode,
        credential: &StoredNodeCredential,
    ) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        let secret =
            CredentialCodec::encode(credential).map_err(SessionCommandError::Credential)?;
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
                .listed_nodes()
                .map_err(SessionCommandError::SubscriptionNodeStore)?,
        );
        let mut nodes = self
            .node_order
            .order_nodes(nodes)
            .map_err(SessionCommandError::NodeOrderStore)?;
        self.node_groups
            .apply(&mut nodes)
            .map_err(SessionCommandError::NodeGroupStore)?;
        Ok(nodes.iter().map(NodeSummary::from).collect())
    }

    /// Lists all named groups in creation order.
    ///
    /// # Errors
    ///
    /// Returns a typed group-store error when the database is unreadable.
    pub fn node_groups(
        &self,
    ) -> Result<Vec<NodeGroupSummary>, SessionCommandError<C::Error, P::Error>> {
        self.node_groups
            .groups()
            .map(|groups| groups.into_iter().map(NodeGroupSummary::from).collect())
            .map_err(SessionCommandError::NodeGroupStore)
    }

    /// Duplicates one manual node, secret and all.
    ///
    /// The copy keeps the original's name — it is a clone, not a variant — and
    /// the selection does not move: cloning exists so a node can be duplicated
    /// and then edited. Subscription nodes are refused because the subscription
    /// owns them, and a copy would outlive a refresh that removed the original.
    ///
    /// # Errors
    ///
    /// Returns a typed not-found error for an unknown or subscription-owned
    /// node, and the secret store's error when the credential cannot be copied.
    pub fn clone_node(
        &mut self,
        id: Uuid,
    ) -> Result<Vec<NodeSummary>, SessionCommandError<C::Error, P::Error>> {
        if self.session.is_running() {
            return Err(SessionCommandError::SessionActive);
        }
        let source = self
            .manual_nodes
            .nodes()
            .map_err(SessionCommandError::NodeStore)?
            .into_iter()
            .find(|node| node.id == id)
            .ok_or(SessionCommandError::NodeStore(
                ManualNodeStoreError::NodeNotFound { id },
            ))?;
        let secret = self
            .session
            .secret_store()
            .get(&source.credential_ref)
            .map_err(SessionCommandError::Secret)?;

        let clone_id = Uuid::new_v4();
        let credential_ref = CredentialRef::new(format!("node/{clone_id}"))
            .map_err(SessionCommandError::CredentialRef)?;
        self.session
            .secret_store()
            .put(&credential_ref, &secret)
            .map_err(SessionCommandError::Secret)?;
        let mut clone = source;
        clone.id = clone_id;
        clone.credential_ref = credential_ref;

        if let Err(store) = self.manual_nodes.save(&clone) {
            // The copied secret must not outlive the node it belongs to.
            return match self.session.secret_store().delete(&clone.credential_ref) {
                Ok(()) => Err(SessionCommandError::NodeStore(store)),
                Err(secret) => {
                    Err(SessionCommandError::NodeStoreAndSecretRollback { store, secret })
                }
            };
        }
        self.nodes()
    }

    /// Deletes every node that repeats one earlier in the list.
    ///
    /// "Repeat" is the same fingerprint the bulk import uses — server, port and
    /// credential — so two entries that differ only by name count as one. The
    /// first occurrence is the one kept, which leaves the user's ordering and
    /// selection alone.
    ///
    /// # Errors
    ///
    /// Returns a typed storage error. A node whose secret cannot be read is
    /// skipped rather than deleted: without its credential there is no way to
    /// tell whether it repeats anything.
    pub fn remove_duplicate_nodes(
        &mut self,
    ) -> Result<usize, SessionCommandError<C::Error, P::Error>> {
        if self.session.is_running() {
            return Err(SessionCommandError::SessionActive);
        }
        let mut seen = HashSet::new();
        let mut repeated = Vec::new();
        for node in self
            .manual_nodes
            .nodes()
            .map_err(SessionCommandError::NodeStore)?
        {
            let Ok(secret) = self.session.secret_store().get(&node.credential_ref) else {
                continue;
            };
            let Ok(credential) = CredentialCodec::decode(&secret) else {
                continue;
            };
            let Some(fingerprint) = node_fingerprint(&node, &credential) else {
                continue;
            };
            if !seen.insert(fingerprint) {
                repeated.push(node.id);
            }
        }
        let removed = repeated.len();
        for id in repeated {
            self.delete_node(id)?;
        }
        Ok(removed)
    }

    /// Writes one stored node back out as a sharing URI.
    ///
    /// The credential is read from the OS store and used only to build the link;
    /// it is never returned on its own, and the link itself is the secret, which
    /// is why this is only ever handed to the clipboard the user asked for.
    ///
    /// # Errors
    ///
    /// Returns a typed not-found error for an unknown node, the secret store's
    /// error when the credential cannot be read, and the serializer's error when
    /// the node has no representation as a link.
    pub fn export_node_link(
        &self,
        id: Uuid,
    ) -> Result<String, SessionCommandError<C::Error, P::Error>> {
        let node = self
            .stored_nodes()?
            .into_iter()
            .find(|node| node.id == id)
            .ok_or(SessionCommandError::NodeStore(
                ManualNodeStoreError::NodeNotFound { id },
            ))?;
        let secret = self
            .session
            .secret_store()
            .get(&node.credential_ref)
            .map_err(SessionCommandError::Secret)?;
        let credential =
            CredentialCodec::decode(&secret).map_err(SessionCommandError::Credential)?;
        ShareLinkSerializer::serialize(&node, &credential)
            .map_err(SessionCommandError::ShareLinkExport)
    }

    /// One node's sharing link rendered as a scannable QR code.
    ///
    /// The code *is* the credential, in the same way the link is: anyone who
    /// photographs it has the node. It is only ever shown on the user's own
    /// screen, at their request.
    ///
    /// # Errors
    ///
    /// Returns the export error for a node with no representable link, and the
    /// renderer's error when the link cannot be encoded.
    pub fn node_qr_code(
        &self,
        id: Uuid,
    ) -> Result<String, SessionCommandError<C::Error, P::Error>> {
        let link = self.export_node_link(id)?;
        ShareLinkQrCode::svg(&link).map_err(SessionCommandError::ShareLinkQr)
    }

    /// Every stored node as the shared model, manual and subscription alike.
    fn stored_nodes(&self) -> Result<Vec<ProxyNode>, SessionCommandError<C::Error, P::Error>> {
        let mut nodes = self
            .manual_nodes
            .nodes()
            .map_err(SessionCommandError::NodeStore)?;
        nodes.extend(
            self.subscription_nodes
                .active_nodes()
                .map_err(SessionCommandError::SubscriptionNodeStore)?,
        );
        Ok(nodes)
    }

    /// Assigns any active manual or subscription node to a named local group.
    /// Passing `None` clears the node's group.
    ///
    /// # Errors
    ///
    /// Returns a typed not-found or group-store error without changing another
    /// node's assignment.
    pub fn set_node_group(
        &mut self,
        id: Uuid,
        group_name: Option<&str>,
    ) -> Result<Vec<NodeSummary>, SessionCommandError<C::Error, P::Error>> {
        if !self.nodes()?.iter().any(|node| node.id == id) {
            return Err(SessionCommandError::NodeStore(
                ManualNodeStoreError::NodeNotFound { id },
            ));
        }
        let group = self
            .node_groups
            .assign(id, group_name)
            .map_err(SessionCommandError::NodeGroupStore)?;
        if let Some(selected) = &mut self.node
            && selected.id == id
        {
            selected.group_id = group.map(|group| group.id);
        }
        self.nodes()
    }

    /// Moves a node one position in the unified manual/subscription list.
    ///
    /// Moving the first node up or the last node down is an idempotent no-op.
    ///
    /// # Errors
    ///
    /// Returns a typed node-store error when the node is absent or the new
    /// order cannot be persisted.
    pub fn move_node(
        &mut self,
        id: Uuid,
        direction: NodeMoveDirection,
    ) -> Result<Vec<NodeSummary>, SessionCommandError<C::Error, P::Error>> {
        let mut nodes = self.nodes()?;
        let index =
            nodes
                .iter()
                .position(|node| node.id == id)
                .ok_or(SessionCommandError::NodeStore(
                    ManualNodeStoreError::NodeNotFound { id },
                ))?;
        let target = match direction {
            NodeMoveDirection::Up => index.checked_sub(1),
            NodeMoveDirection::Down if index + 1 < nodes.len() => Some(index + 1),
            NodeMoveDirection::Down => None,
        };
        if let Some(target) = target {
            nodes.swap(index, target);
            self.node_order
                .save(&nodes.iter().map(|node| node.id).collect::<Vec<_>>())
                .map_err(SessionCommandError::NodeOrderStore)?;
        }
        Ok(nodes)
    }

    /// Replaces the unified node order with an explicit id list.
    ///
    /// `ids` must be a permutation of the current list: every persisted node
    /// exactly once. Used by "sort by latency" and any future drag-reorder.
    ///
    /// # Errors
    ///
    /// Returns a typed not-found or order-store error when the list is incomplete,
    /// contains unknowns, or cannot be persisted.
    pub fn reorder_nodes(
        &mut self,
        ids: &[Uuid],
    ) -> Result<Vec<NodeSummary>, SessionCommandError<C::Error, P::Error>> {
        let current = self.nodes()?;
        if ids.len() != current.len() {
            return Err(SessionCommandError::NodeOrderStore(
                NodeOrderStoreError::IncompleteReorder {
                    expected: current.len(),
                    actual: ids.len(),
                },
            ));
        }
        let mut seen = HashSet::with_capacity(ids.len());
        for id in ids {
            if !seen.insert(*id) {
                return Err(SessionCommandError::NodeOrderStore(
                    NodeOrderStoreError::DuplicateNode { id: *id },
                ));
            }
            if !current.iter().any(|node| node.id == *id) {
                return Err(SessionCommandError::NodeStore(
                    ManualNodeStoreError::NodeNotFound { id: *id },
                ));
            }
        }
        self.node_order
            .save(ids)
            .map_err(SessionCommandError::NodeOrderStore)?;
        self.nodes()
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
        let mut node = match self.manual_nodes.update_latency(id, latency_ms, tested_at) {
            Ok(node) => node,
            Err(ManualNodeStoreError::NodeNotFound { .. }) => self
                .subscription_nodes
                .update_node_latency(id, latency_ms, tested_at)
                .map_err(SessionCommandError::SubscriptionNodeStore)?,
            Err(error) => return Err(SessionCommandError::NodeStore(error)),
        };
        self.node_groups
            .apply(std::slice::from_mut(&mut node))
            .map_err(SessionCommandError::NodeGroupStore)?;
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
        self.select_node_while_stopped(id)
    }

    /// Selects a node and, when a session was running, reconnects through it.
    ///
    /// Used by previous/next navigation so an active connection is not a hard
    /// block on changing servers.
    ///
    /// # Errors
    ///
    /// Returns a typed error when selection fails, or when disconnect/reconnect
    /// fails. A failed reconnect leaves the new node selected but idle.
    pub fn switch_node(
        &mut self,
        id: Uuid,
    ) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        let was_running = self.session.is_running();
        if was_running {
            self.session.stop().map_err(SessionCommandError::Session)?;
        }
        self.select_node_while_stopped(id)?;
        if was_running {
            self.connect()?;
        }
        Ok(self.status())
    }

    fn select_node_while_stopped(
        &mut self,
        id: Uuid,
    ) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        let mut node = match self.manual_nodes.select(id) {
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
        if !node.enabled {
            return Err(SessionCommandError::NodeDisabled { id });
        }
        self.node_groups
            .apply(std::slice::from_mut(&mut node))
            .map_err(SessionCommandError::NodeGroupStore)?;
        self.node = Some(node);
        Ok(self.status())
    }

    /// Enables or disables a persisted node without deleting it.
    ///
    /// Disabled nodes stay in the list (and keep their credentials) but cannot
    /// be selected or connected. Disabling the current selection clears it.
    ///
    /// # Errors
    ///
    /// Returns a typed active-session, not-found, or persistence error.
    pub fn set_node_enabled(
        &mut self,
        id: Uuid,
        enabled: bool,
    ) -> Result<Vec<NodeSummary>, SessionCommandError<C::Error, P::Error>> {
        if self.session.is_running() {
            return Err(SessionCommandError::SessionActive);
        }

        let manual = self
            .manual_nodes
            .nodes()
            .map_err(SessionCommandError::NodeStore)?
            .into_iter()
            .find(|node| node.id == id);
        if let Some(mut node) = manual {
            node.enabled = enabled;
            self.manual_nodes
                .update(&node)
                .map_err(SessionCommandError::NodeStore)?;
        } else {
            self.subscription_nodes
                .set_node_enabled(id, enabled)
                .map_err(SessionCommandError::SubscriptionNodeStore)?;
        }

        if !enabled && self.node.as_ref().is_some_and(|node| node.id == id) {
            self.node = None;
            self.manual_nodes
                .clear_selected()
                .map_err(SessionCommandError::NodeStore)?;
            self.subscription_nodes
                .clear_selected_node()
                .map_err(SessionCommandError::SubscriptionNodeStore)?;
        } else if let Some(selected) = self.node.as_mut()
            && selected.id == id
        {
            selected.enabled = enabled;
        }

        self.nodes()
    }

    /// Changes the editable endpoint fields of a persisted manual node.
    ///
    /// Protocol, transport, TLS, and the credential reference remain untouched.
    /// Subscription nodes stay read-only because their next refresh owns those
    /// fields.
    ///
    /// # Errors
    ///
    /// Returns a typed active-session, validation, read-only, not-found, or
    /// persistence error.
    pub fn edit_node(
        &mut self,
        id: Uuid,
        name: impl Into<String>,
        server: impl Into<String>,
        port: u32,
    ) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        if self.session.is_running() {
            return Err(SessionCommandError::SessionActive);
        }
        let mut node = self
            .manual_nodes
            .nodes()
            .map_err(SessionCommandError::NodeStore)?
            .into_iter()
            .find(|node| node.id == id);
        if node.is_none()
            && self
                .subscription_nodes
                .active_nodes()
                .map_err(SessionCommandError::SubscriptionNodeStore)?
                .iter()
                .any(|node| node.id == id)
        {
            return Err(SessionCommandError::SubscriptionNodeReadOnly { id });
        }
        let node = node.as_mut().ok_or(SessionCommandError::NodeStore(
            ManualNodeStoreError::NodeNotFound { id },
        ))?;
        node.name = NodeName::new(name).map_err(SessionCommandError::InvalidNode)?;
        node.server = ServerAddress::new(server).map_err(SessionCommandError::InvalidNode)?;
        node.port = u16::try_from(port).ok().and_then(NonZeroU16::new).ok_or(
            SessionCommandError::InvalidNode(NodeModelError::InvalidPort { port }),
        )?;
        let mut node = self
            .manual_nodes
            .update(node)
            .map_err(SessionCommandError::NodeStore)?;
        self.node_groups
            .apply(std::slice::from_mut(&mut node))
            .map_err(SessionCommandError::NodeGroupStore)?;
        if self.node.as_ref().is_some_and(|selected| selected.id == id) {
            self.node = Some(node);
        }
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
        let profile = self
            .current_route_settings
            .profile(mode)
            .map_err(SessionCommandError::InvalidRouteSettings)?;
        self.routing_mode
            .save(mode)
            .map_err(SessionCommandError::RoutingModeStore)?;
        self.defaults.route = profile;
        Ok(self.status())
    }

    /// Saves the ordered route rules used by the next connection.
    ///
    /// # Errors
    ///
    /// Returns a typed active-session, validation, or persistence error.
    pub fn set_route_settings(
        &mut self,
        settings: RouteSettings,
    ) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        if self.session.is_running() {
            return Err(SessionCommandError::SessionActive);
        }
        settings
            .profile(RoutingMode::Rule)
            .map_err(SessionCommandError::InvalidRouteSettings)?;
        let profile = settings
            .profile(self.defaults.route.mode())
            .map_err(SessionCommandError::InvalidRouteSettings)?;
        self.route_settings
            .save(&settings)
            .map_err(SessionCommandError::RouteSettingsStore)?;
        self.defaults.route = profile;
        self.current_route_settings = settings;
        Ok(self.status())
    }

    /// Saves the DNS settings used by the next connection.
    ///
    /// # Errors
    ///
    /// Returns a typed active-session, validation, or persistence error.
    pub fn set_dns_settings(
        &mut self,
        settings: DnsSettings,
    ) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        if self.session.is_running() {
            return Err(SessionCommandError::SessionActive);
        }
        let profile = settings
            .profile()
            .map_err(SessionCommandError::InvalidDnsSettings)?;
        self.dns_settings
            .save(&settings)
            .map_err(SessionCommandError::DnsSettingsStore)?;
        self.defaults.dns = profile;
        self.current_dns_settings = settings;
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
        if let Some(node) = &mut self.node {
            self.node_groups
                .apply(std::slice::from_mut(node))
                .map_err(SessionCommandError::NodeGroupStore)?;
        }
        Ok(self.status())
    }

    /// Starts the Core and System Proxy for the selected node.
    ///
    /// # Errors
    ///
    /// Returns [`SessionCommandError::NoSelectedNode`] before any node is
    /// imported, otherwise the orchestrator's typed error.
    pub fn connect(&mut self) -> Result<SessionStatus, SessionCommandError<C::Error, P::Error>> {
        // Checked before anything is generated or spawned: another proxy client
        // holding a loopback port makes the Core exit on its own, and its exit
        // code names nothing the user can act on.
        LocalProxyPortChecker::check_with_allow_lan(
            self.defaults.socks.port().get(),
            self.defaults.http.port().get(),
            self.defaults.socks.allow_lan(),
        )
        .map_err(SessionCommandError::LocalProxyPort)?;
        let node = self
            .node
            .clone()
            .ok_or(SessionCommandError::NoSelectedNode)?;
        // Resolved before anything starts: a preference the node cannot satisfy
        // is an error the user sees, not a silent fallback to the other Core.
        let core = self
            .selected_core()
            .map_err(SessionCommandError::CoreSelection)?;
        let profile = DesktopSessionProfile::new(
            node,
            self.defaults.dns.clone(),
            self.defaults.route.clone(),
        )
        .with_core(core)
        .with_local_proxies(self.defaults.socks, self.defaults.http)
        .with_clash_api_port(self.defaults.clash_api_port)
        .with_mux(self.defaults.mux_enabled);
        // The two are mutually exclusive in DesktopSession, so TUN replaces
        // System Proxy rather than being layered on top of it.
        let profile = match self.tun_profile()? {
            Some(tun) => profile.with_system_proxy(false).with_tun(tun, true),
            None => profile.with_system_proxy_mode(self.defaults.system_proxy.clone()),
        };

        let output = self
            .session
            .start(&profile)
            .map_err(SessionCommandError::Session)?;
        self.core_output = Some(output);
        Ok(self.status())
    }

    /// The loopback ports, routing and DNS this session starts from.
    #[must_use]
    pub const fn defaults(&self) -> &SessionDefaults {
        &self.defaults
    }

    /// Hands the running Core's output stream to the caller, once.
    pub fn take_core_output(&mut self) -> Option<C::Output> {
        self.core_output.take()
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

    /// The Core that would run the selected node, and why not when none can.
    ///
    /// With no node selected there is nothing to match against, so the answer
    /// is the preference itself rather than a guess.
    ///
    /// # Errors
    ///
    /// Returns the matrix's reason when the preferred Core cannot serve the
    /// node, or when no Core can.
    pub fn selected_core(&self) -> Result<CoreType, CoreSelectionError> {
        let Some(node) = self.node.as_ref() else {
            return Ok(match self.core_preference {
                CorePreference::Fixed(core) => core,
                CorePreference::Auto => CoreType::SingBox,
            });
        };
        let requirements =
            CoreRequirements::new(node.protocol_type, self.tun_enabled, host_architecture());
        let requirements = if pins_a_certificate(node.tls.as_ref()) {
            requirements.with_certificate_pin()
        } else {
            requirements
        };
        let requirements = if matches!(node.transport, Some(TransportConfig::XHttp { .. })) {
            requirements.with_xhttp()
        } else {
            requirements
        };
        CoreCapabilityMatrix::select(self.core_preference, requirements)
    }

    /// Replaces the Core the session should run.
    pub fn set_core_preference(&mut self, preference: CorePreference) {
        self.core_preference = preference;
    }

    /// Replaces what the next session does to the host's System Proxy.
    pub fn set_system_proxy_mode(&mut self, mode: SystemProxyMode) {
        self.defaults.system_proxy = mode;
    }

    /// Updates the local SOCKS/HTTP inbound ports and Clash API port.
    ///
    /// # Errors
    ///
    /// Returns a typed active-session or invalid-port error. All three ports
    /// must differ so the Core can bind each one.
    pub fn set_local_proxies(
        &mut self,
        socks_port: u16,
        http_port: u16,
        clash_api_port: u16,
    ) -> Result<(), SessionCommandError<C::Error, P::Error>> {
        if self.session.is_running() {
            return Err(SessionCommandError::SessionActive);
        }
        if socks_port == http_port || socks_port == clash_api_port || http_port == clash_api_port {
            return Err(SessionCommandError::DuplicateLocalProxyPort {
                port: if socks_port == http_port || socks_port == clash_api_port {
                    socks_port
                } else {
                    http_port
                },
            });
        }
        self.defaults.socks = LocalSocksProfile::new(u32::from(socks_port))
            .map_err(SessionCommandError::LocalProxyConfig)?
            .with_allow_lan(self.defaults.socks.allow_lan())
            .with_udp_enabled(self.defaults.socks.udp_enabled());
        self.defaults.http = LocalHttpProfile::new(u32::from(http_port))
            .map_err(SessionCommandError::LocalProxyConfig)?
            .with_allow_lan(self.defaults.http.allow_lan());
        // Reuse the SOCKS constructor so a zero Clash API port fails the same
        // way as a zero inbound port, instead of inventing a third error path.
        self.defaults.clash_api_port = LocalSocksProfile::new(u32::from(clash_api_port))
            .map_err(SessionCommandError::LocalProxyConfig)?
            .port();
        Ok(())
    }

    /// Lets LAN peers reach the local SOCKS/HTTP inbounds on the next connect.
    pub fn set_allow_lan(&mut self, enabled: bool) {
        self.defaults.socks = self.defaults.socks.with_allow_lan(enabled);
        self.defaults.http = self.defaults.http.with_allow_lan(enabled);
    }

    /// Enables SOCKS UDP associate for the next Xray session.
    pub fn set_inbound_udp_enabled(&mut self, enabled: bool) {
        self.defaults.socks = self.defaults.socks.with_udp_enabled(enabled);
    }

    /// Turns Core multiplex / mux on or off for the next session.
    pub fn set_mux_enabled(&mut self, enabled: bool) {
        self.defaults.mux_enabled = enabled;
    }

    /// Turns TUN routing on or off for the next session.
    pub fn set_tun_enabled(&mut self, enabled: bool) {
        self.tun_enabled = enabled;
    }

    /// The TUN profile for this host, or `None` when TUN is off or unavailable.
    ///
    /// # Errors
    ///
    /// Returns a typed error when this platform cannot provide TUN at all, so
    /// the refusal names the reason instead of quietly falling back to System
    /// Proxy.
    fn tun_profile(&self) -> Result<Option<TunProfile>, SessionCommandError<C::Error, P::Error>> {
        if !self.tun_enabled {
            return Ok(None);
        }
        let os = host_operating_system().ok_or(SessionCommandError::TunUnavailable)?;
        TunProfile::new(os, false, DEFAULT_TUN_MTU, true, true)
            .map(Some)
            .map_err(SessionCommandError::TunProfile)
    }

    #[must_use]
    pub fn status(&self) -> SessionStatus {
        // A node the chosen Core cannot serve still has to render, so the
        // status falls back to the preference and the connect attempt is what
        // surfaces the typed error.
        let core = self.selected_core().unwrap_or(match self.core_preference {
            CorePreference::Fixed(core) => core,
            CorePreference::Auto => CoreType::SingBox,
        });
        SessionStatus {
            connected: self.session.is_running(),
            node: self.node.as_ref().map(NodeSummary::from),
            core: core_name(core),
            dns: self.current_dns_settings.clone(),
            mode: self.defaults.mode(),
            route: self.current_route_settings.clone(),
            system_proxy: self.defaults.system_proxy != SystemProxyMode::Unchanged,
            system_proxy_mode: system_proxy_mode_name(&self.defaults.system_proxy),
            socks_port: self.defaults.socks.port().get(),
            http_port: self.defaults.http.port().get(),
            clash_api_port: self.defaults.clash_api_port.get(),
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
    #[error("failed to read node groups")]
    NodeGroupStore(#[from] NodeGroupStoreError),
    #[error("failed to load the routing mode")]
    RoutingModeStore(#[from] RoutingModeStoreError),
    #[error("failed to load the route settings")]
    RouteSettingsStore(#[from] RouteSettingsStoreError),
    #[error("the saved route settings are invalid")]
    RouteSettings(#[from] RouteSettingsError),
    #[error("failed to load the DNS settings")]
    DnsSettingsStore(#[from] DnsSettingsStoreError),
    #[error("the saved DNS settings are invalid")]
    DnsSettings(#[from] DnsConfigError),
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
    #[error("invalid node settings")]
    InvalidNode(#[source] NodeModelError),
    #[error("failed to parse the sharing URI")]
    ShareLink(#[source] ShareLinkParseError),
    #[error("this platform cannot provide TUN")]
    TunUnavailable,
    #[error("invalid TUN settings")]
    TunProfile(#[source] TunProfileError),
    #[error("no usable Core for this node")]
    CoreSelection(#[source] CoreSelectionError),
    #[error("a local proxy port is unavailable")]
    LocalProxyPort(#[source] LocalProxyPortError),
    #[error("invalid local proxy port")]
    LocalProxyConfig(#[source] LocalProxyConfigError),
    #[error("local proxy ports cannot share port {port}")]
    DuplicateLocalProxyPort { port: u16 },
    #[error("this node has no sharing link")]
    ShareLinkExport(#[source] ShareLinkSerializerError),
    #[error("the sharing link could not be drawn as a QR code")]
    ShareLinkQr(#[source] ShareLinkQrError),
    #[error("invalid manual node settings")]
    ManualNodeDraft(#[source] ManualNodeDraftError),
    #[error("the imported node list could not be read")]
    BulkImport(#[source] BulkImportError),
    #[error("failed to encode the node credential")]
    Credential(#[source] CredentialCodecError),
    #[error("failed to save the node credential")]
    Secret(#[source] SecretStoreError),
    #[error("failed to change the manual node store")]
    NodeStore(#[source] ManualNodeStoreError),
    #[error("failed to change the subscription node store")]
    SubscriptionNodeStore(#[source] SubscriptionTransactionError),
    #[error("failed to save the node order")]
    NodeOrderStore(#[source] NodeOrderStoreError),
    #[error("failed to change node groups")]
    NodeGroupStore(#[source] NodeGroupStoreError),
    #[error("failed to save the routing mode")]
    RoutingModeStore(#[source] RoutingModeStoreError),
    #[error("invalid route settings")]
    InvalidRouteSettings(#[source] RouteSettingsError),
    #[error("failed to save the route settings")]
    RouteSettingsStore(#[source] RouteSettingsStoreError),
    #[error("invalid DNS settings")]
    InvalidDnsSettings(#[source] DnsConfigError),
    #[error("failed to save the DNS settings")]
    DnsSettingsStore(#[source] DnsSettingsStoreError),
    #[error("subscription node {id} is managed by its subscription")]
    SubscriptionNodeReadOnly { id: Uuid },
    #[error("node {id} is disabled")]
    NodeDisabled { id: Uuid },
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
            Self::InvalidNode(_) => "invalid_node",
            Self::ShareLink(_) => "invalid_share_link",
            Self::ManualNodeDraft(_) => "invalid_manual_node",
            Self::CoreSelection(_) => "core_unavailable",
            Self::LocalProxyPort(_) => "local_proxy_port_unavailable",
            Self::LocalProxyConfig(_) | Self::DuplicateLocalProxyPort { .. } => {
                "invalid_local_proxy_port"
            }
            Self::ShareLinkExport(_) => "share_link_unavailable",
            Self::ShareLinkQr(_) => "qr_code_unavailable",
            Self::TunUnavailable | Self::TunProfile(_) => "tun_unavailable",
            Self::BulkImport(_) => "invalid_node_list",
            Self::Credential(_) => "credential_encode_failed",
            Self::Secret(_) | Self::DeleteSecret(_) => "secret_store_failed",
            Self::NodeStore(ManualNodeStoreError::NodeNotFound { .. })
            | Self::SubscriptionNodeStore(SubscriptionTransactionError::NodeNotFound { .. }) => {
                "node_not_found"
            }
            Self::NodeStore(_)
            | Self::NodeStoreAndSecretRollback { .. }
            | Self::SubscriptionNodeStore(_) => "node_store_failed",
            Self::NodeOrderStore(_) => "node_order_store_failed",
            Self::NodeGroupStore(NodeGroupStoreError::EmptyName) => "invalid_node_group",
            Self::NodeGroupStore(_) => "node_group_store_failed",
            Self::SubscriptionNodeReadOnly { .. } => "subscription_node_read_only",
            Self::NodeDisabled { .. } => "node_disabled",
            Self::RoutingModeStore(_) => "routing_mode_store_failed",
            Self::InvalidRouteSettings(_) => "invalid_route_settings",
            Self::RouteSettingsStore(_) => "route_settings_store_failed",
            Self::InvalidDnsSettings(_) => "invalid_dns_settings",
            Self::DnsSettingsStore(_) => "dns_settings_store_failed",
            Self::SessionActive => "session_active",
            Self::SessionInactive => "session_inactive",
            Self::Session(_) => "session_failed",
        }
    }
}
