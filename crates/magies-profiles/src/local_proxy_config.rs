use std::num::NonZeroU16;

use magies_domain::CoreType;
use serde_json::{Value, json};

pub const DEFAULT_SOCKS_PORT: u16 = 10_808;
pub const DEFAULT_HTTP_PORT: u16 = 10_809;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalSocksProfile {
    port: NonZeroU16,
}

impl LocalSocksProfile {
    /// Creates a loopback-only local SOCKS profile.
    ///
    /// # Errors
    ///
    /// Returns a typed error when `port` is outside `1..=65535`.
    pub fn new(port: u32) -> Result<Self, LocalProxyConfigError> {
        parse_port(port).map(|port| Self { port })
    }

    #[must_use]
    pub const fn port(self) -> NonZeroU16 {
        self.port
    }

    #[must_use]
    pub const fn udp_enabled(self) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalHttpProfile {
    port: NonZeroU16,
}

impl LocalHttpProfile {
    /// Creates a loopback-only local HTTP proxy profile.
    ///
    /// # Errors
    ///
    /// Returns a typed error when `port` is outside `1..=65535`.
    pub fn new(port: u32) -> Result<Self, LocalProxyConfigError> {
        parse_port(port).map(|port| Self { port })
    }

    #[must_use]
    pub const fn port(self) -> NonZeroU16 {
        self.port
    }
}

impl Default for LocalHttpProfile {
    fn default() -> Self {
        Self {
            port: NonZeroU16::new(DEFAULT_HTTP_PORT).expect("default HTTP port is non-zero"),
        }
    }
}

impl Default for LocalSocksProfile {
    fn default() -> Self {
        Self {
            port: NonZeroU16::new(DEFAULT_SOCKS_PORT).expect("default SOCKS port is non-zero"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedCoreConfig {
    core_type: CoreType,
    json: Value,
}

impl GeneratedCoreConfig {
    pub(crate) const fn from_json(core_type: CoreType, json: Value) -> Self {
        Self { core_type, json }
    }

    #[must_use]
    pub const fn core_type(&self) -> CoreType {
        self.core_type
    }

    #[must_use]
    pub const fn json(&self) -> &Value {
        &self.json
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LocalSocksConfigGenerator;

impl LocalSocksConfigGenerator {
    #[must_use]
    pub fn generate(core_type: CoreType, profile: &LocalSocksProfile) -> GeneratedCoreConfig {
        let json = match core_type {
            CoreType::Xray => xray_config(&xray_socks_inbound(*profile)),
            CoreType::SingBox => sing_box_config(&sing_box_socks_inbound(*profile)),
        };
        GeneratedCoreConfig::from_json(core_type, json)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LocalHttpConfigGenerator;

impl LocalHttpConfigGenerator {
    #[must_use]
    pub fn generate(core_type: CoreType, profile: &LocalHttpProfile) -> GeneratedCoreConfig {
        let json = match core_type {
            CoreType::Xray => xray_config(&xray_http_inbound(*profile)),
            CoreType::SingBox => sing_box_config(&sing_box_http_inbound(*profile)),
        };
        GeneratedCoreConfig::from_json(core_type, json)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LocalProxyConfigError {
    #[error("local proxy port must be between 1 and 65535, got {port}")]
    InvalidPort { port: u32 },
}

fn parse_port(port: u32) -> Result<NonZeroU16, LocalProxyConfigError> {
    u16::try_from(port)
        .ok()
        .and_then(NonZeroU16::new)
        .ok_or(LocalProxyConfigError::InvalidPort { port })
}

fn xray_config(inbound: &Value) -> Value {
    json!({
        "log": { "loglevel": "warning" },
        "inbounds": [inbound],
        "outbounds": [{
            "tag": "direct",
            "protocol": "freedom",
            "settings": {}
        }]
    })
}

fn sing_box_config(inbound: &Value) -> Value {
    json!({
        "log": { "level": "warn" },
        "inbounds": [inbound],
        "outbounds": [{
            "type": "direct",
            "tag": "direct"
        }]
    })
}

fn xray_socks_inbound(profile: LocalSocksProfile) -> Value {
    json!({
        "tag": "socks-in",
        "listen": "127.0.0.1",
        "port": profile.port().get(),
        "protocol": "socks",
        "settings": {
            "auth": "noauth",
            "udp": profile.udp_enabled(),
            "ip": "127.0.0.1"
        }
    })
}

fn sing_box_socks_inbound(profile: LocalSocksProfile) -> Value {
    json!({
        "type": "socks",
        "tag": "socks-in",
        "listen": "127.0.0.1",
        "listen_port": profile.port().get()
    })
}

fn xray_http_inbound(profile: LocalHttpProfile) -> Value {
    json!({
        "tag": "http-in",
        "listen": "127.0.0.1",
        "port": profile.port().get(),
        "protocol": "http",
        "settings": {
            "allowTransparent": false
        }
    })
}

fn sing_box_http_inbound(profile: LocalHttpProfile) -> Value {
    json!({
        "type": "http",
        "tag": "http-in",
        "listen": "127.0.0.1",
        "listen_port": profile.port().get(),
        "set_system_proxy": false
    })
}
