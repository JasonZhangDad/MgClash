use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProxyEndpoint {
    host: String,
    port: u16,
}

impl ProxyEndpoint {
    /// Creates a validated system proxy endpoint.
    ///
    /// # Errors
    ///
    /// Returns an error when `host` is empty or `port` is zero.
    pub fn new(host: &str, port: u16) -> Result<Self, SystemProxyModelError> {
        let host = host.trim();
        if host.is_empty() {
            return Err(SystemProxyModelError::EmptyHost);
        }
        if port == 0 {
            return Err(SystemProxyModelError::InvalidPort);
        }

        Ok(Self {
            host: host.to_owned(),
            port,
        })
    }

    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProxySetting {
    enabled: bool,
    endpoint: Option<ProxyEndpoint>,
}

impl ProxySetting {
    /// Creates a system proxy setting while preserving disabled endpoints.
    ///
    /// # Errors
    ///
    /// Returns an error when an enabled setting has no endpoint.
    pub fn new(
        enabled: bool,
        endpoint: Option<ProxyEndpoint>,
    ) -> Result<Self, SystemProxyModelError> {
        if enabled && endpoint.is_none() {
            return Err(SystemProxyModelError::MissingEndpoint);
        }

        Ok(Self { enabled, endpoint })
    }

    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            endpoint: None,
        }
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub const fn endpoint(&self) -> Option<&ProxyEndpoint> {
        self.endpoint.as_ref()
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct PacSetting {
    enabled: bool,
    url: Option<String>,
}

impl PacSetting {
    /// Creates a proxy auto-configuration setting.
    ///
    /// # Errors
    ///
    /// Returns an error when an enabled setting has no non-empty URL.
    pub fn new(enabled: bool, url: Option<String>) -> Result<Self, SystemProxyModelError> {
        let url = url.and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        });
        if enabled && url.is_none() {
            return Err(SystemProxyModelError::MissingPacUrl);
        }

        Ok(Self { enabled, url })
    }

    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            url: None,
        }
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }
}

impl Debug for PacSetting {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PacSetting")
            .field("enabled", &self.enabled)
            .field("url", &self.url.as_ref().map(|_| "configured"))
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SystemProxyState {
    http: ProxySetting,
    https: ProxySetting,
    socks: ProxySetting,
    pac: PacSetting,
}

impl SystemProxyState {
    #[must_use]
    pub const fn new(
        http: ProxySetting,
        https: ProxySetting,
        socks: ProxySetting,
        pac: PacSetting,
    ) -> Self {
        Self {
            http,
            https,
            socks,
            pac,
        }
    }

    #[must_use]
    pub const fn http(&self) -> &ProxySetting {
        &self.http
    }

    #[must_use]
    pub const fn https(&self) -> &ProxySetting {
        &self.https
    }

    #[must_use]
    pub const fn socks(&self) -> &ProxySetting {
        &self.socks
    }

    #[must_use]
    pub const fn pac(&self) -> &PacSetting {
        &self.pac
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemProxyModelError {
    EmptyHost,
    InvalidPort,
    MissingEndpoint,
    MissingPacUrl,
}

impl Display for SystemProxyModelError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::EmptyHost => "system proxy host cannot be empty",
            Self::InvalidPort => "system proxy port must be nonzero",
            Self::MissingEndpoint => "enabled system proxy requires an endpoint",
            Self::MissingPacUrl => "enabled proxy auto-configuration requires a URL",
        };
        formatter.write_str(message)
    }
}

impl Error for SystemProxyModelError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_requires_a_host_and_nonzero_port() {
        assert_eq!(
            ProxyEndpoint::new("  ", 1080),
            Err(SystemProxyModelError::EmptyHost)
        );
        assert_eq!(
            ProxyEndpoint::new("127.0.0.1", 0),
            Err(SystemProxyModelError::InvalidPort)
        );
    }

    #[test]
    fn endpoint_trims_the_host() {
        let endpoint = ProxyEndpoint::new(" 127.0.0.1 ", 1080).unwrap();

        assert_eq!(endpoint.host(), "127.0.0.1");
        assert_eq!(endpoint.port(), 1080);
    }

    #[test]
    fn enabled_proxy_requires_an_endpoint() {
        assert_eq!(
            ProxySetting::new(true, None),
            Err(SystemProxyModelError::MissingEndpoint)
        );
        assert_eq!(ProxySetting::new(false, None), Ok(ProxySetting::disabled()));
    }

    #[test]
    fn enabled_pac_requires_a_nonempty_url() {
        assert_eq!(
            PacSetting::new(true, None),
            Err(SystemProxyModelError::MissingPacUrl)
        );
        assert_eq!(
            PacSetting::new(true, Some("  ".to_owned())),
            Err(SystemProxyModelError::MissingPacUrl)
        );
    }

    #[test]
    fn pac_debug_output_never_contains_its_url() {
        let pac = PacSetting::new(
            false,
            Some("https://user:secret@example.test/proxy.pac".to_owned()),
        )
        .unwrap();

        let debug = format!("{pac:?}");
        assert!(debug.contains("configured"));
        assert!(!debug.contains("secret"));
        assert!(!debug.contains("example.test"));
    }
}
