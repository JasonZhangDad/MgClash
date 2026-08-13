use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpListener};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalProxyPortKind {
    Socks,
    Http,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LocalProxyPortChecker;

impl LocalProxyPortChecker {
    /// Checks that distinct SOCKS and HTTP ports can bind on IPv4 loopback.
    ///
    /// # Errors
    ///
    /// Returns a typed duplicate or bind error identifying the affected port.
    pub fn check(socks_port: u16, http_port: u16) -> Result<(), LocalProxyPortError> {
        Self::check_with_allow_lan(socks_port, http_port, false)
    }

    /// Checks that the ports can bind on loopback, or on all interfaces when
    /// Allow LAN is enabled.
    ///
    /// # Errors
    ///
    /// Returns a typed duplicate or bind error identifying the affected port.
    pub fn check_with_allow_lan(
        socks_port: u16,
        http_port: u16,
        allow_lan: bool,
    ) -> Result<(), LocalProxyPortError> {
        if socks_port == 0 {
            return Err(LocalProxyPortError::InvalidPort {
                kind: LocalProxyPortKind::Socks,
            });
        }
        if http_port == 0 {
            return Err(LocalProxyPortError::InvalidPort {
                kind: LocalProxyPortKind::Http,
            });
        }
        if socks_port == http_port {
            return Err(LocalProxyPortError::DuplicatePort { port: socks_port });
        }

        let host = if allow_lan {
            Ipv4Addr::UNSPECIFIED
        } else {
            Ipv4Addr::LOCALHOST
        };
        let _socks = bind(LocalProxyPortKind::Socks, host, socks_port)?;
        let _http = bind(LocalProxyPortKind::Http, host, http_port)?;
        Ok(())
    }
}

fn bind(
    kind: LocalProxyPortKind,
    host: Ipv4Addr,
    port: u16,
) -> Result<TcpListener, LocalProxyPortError> {
    TcpListener::bind(SocketAddr::from((host, port))).map_err(|source| {
        LocalProxyPortError::Unavailable {
            kind,
            port,
            host,
            source,
        }
    })
}

#[derive(Debug, thiserror::Error)]
pub enum LocalProxyPortError {
    #[error("local {kind:?} proxy port must be nonzero")]
    InvalidPort { kind: LocalProxyPortKind },
    #[error("SOCKS and HTTP local proxies cannot share port {port}")]
    DuplicatePort { port: u16 },
    #[error("local {kind:?} proxy port {host}:{port} is unavailable: {source}")]
    Unavailable {
        kind: LocalProxyPortKind,
        port: u16,
        host: Ipv4Addr,
        #[source]
        source: io::Error,
    },
}

impl LocalProxyPortError {
    #[must_use]
    pub const fn kind(&self) -> Option<LocalProxyPortKind> {
        match self {
            Self::DuplicatePort { .. } => None,
            Self::InvalidPort { kind } | Self::Unavailable { kind, .. } => Some(*kind),
        }
    }

    #[must_use]
    pub const fn port(&self) -> u16 {
        match self {
            Self::InvalidPort { .. } => 0,
            Self::DuplicatePort { port } | Self::Unavailable { port, .. } => *port,
        }
    }
}
