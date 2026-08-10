//! Desktop session state shared by the `MgClash` Tauri commands.
//!
//! The service keeps the UI free of Core JSON: it turns a sharing URI into the
//! shared node model, saves the credential in the OS store, and drives
//! [`DesktopSession`] for connect and disconnect.

use magies_domain::{CredentialRef, NodeModelError, ProxyNode, ProxyProtocol};
use magies_profiles::{
    CredentialCodec, CredentialCodecError, DnsProfile, DnsServer, DnsStrategy, LocalHttpProfile,
    LocalSocksProfile, ShareLinkParseError, ShareLinkParser,
};
use magies_routing::{RouteOutbound, RouteProfile, RoutingMode};
use magies_session::{
    CoreSessionControl, DesktopSession, DesktopSessionError, DesktopSessionProfile,
    SystemProxySessionControl,
};
use magies_storage::{SecretStore, SecretStoreError};
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

/// The Core the V0.1 desktop shell drives.
const CORE_NAME: &str = "sing-box";

/// Fixed session settings the V0.1 shell does not let the user edit yet.
#[derive(Clone, Debug)]
pub struct SessionDefaults {
    pub socks: LocalSocksProfile,
    pub http: LocalHttpProfile,
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
            dns: DnsProfile::new(
                vec![DnsServer::system("system").expect("\"system\" is a valid DNS server tag")],
                Vec::new(),
                "system",
                DnsStrategy::PreferIpv4,
                false,
                false,
            )
            .expect("the single system resolver is a valid DNS profile"),
            route: RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy)
                .expect("Global mode with a proxy final outbound is valid"),
            system_proxy: true,
        }
    }

    fn mode(&self) -> &'static str {
        match self.route.mode() {
            RoutingMode::Global => "global",
            RoutingMode::Rule => "rule",
            RoutingMode::Direct => "direct",
        }
    }
}

/// The selected node as the dashboard renders it. Credentials never appear here.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSummary {
    pub name: String,
    pub protocol: ProxyProtocol,
    pub server: String,
    pub port: u16,
}

impl From<&ProxyNode> for NodeSummary {
    fn from(node: &ProxyNode) -> Self {
        Self {
            name: node.name.as_str().to_owned(),
            protocol: node.protocol_type,
            server: node.server.as_str().to_owned(),
            port: node.port.get(),
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

/// Owns the selected node and the orchestrated session behind the commands.
pub struct SessionService<S, C, P> {
    session: DesktopSession<S, C, P>,
    defaults: SessionDefaults,
    node: Option<ProxyNode>,
}

impl<S, C, P> SessionService<S, C, P>
where
    S: SecretStore,
    C: CoreSessionControl,
    P: SystemProxySessionControl,
{
    #[must_use]
    pub const fn new(session: DesktopSession<S, C, P>, defaults: SessionDefaults) -> Self {
        Self {
            session,
            defaults,
            node: None,
        }
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

        self.node = Some(node);
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
            Self::Secret(_) => "secret_store_failed",
            Self::Session(_) => "session_failed",
        }
    }
}
