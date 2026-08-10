use std::num::NonZeroU16;

use magies_domain::CoreType;
use serde_json::{Value, json};

pub const DEFAULT_SOCKS_PORT: u16 = 10_808;

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
        let port = u16::try_from(port)
            .ok()
            .and_then(NonZeroU16::new)
            .ok_or(LocalProxyConfigError::InvalidPort { port })?;
        Ok(Self { port })
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
            CoreType::Xray => xray_config(*profile),
            CoreType::SingBox => sing_box_config(*profile),
        };
        GeneratedCoreConfig { core_type, json }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LocalProxyConfigError {
    #[error("local proxy port must be between 1 and 65535, got {port}")]
    InvalidPort { port: u32 },
}

fn xray_config(profile: LocalSocksProfile) -> Value {
    json!({
        "log": { "loglevel": "warning" },
        "inbounds": [{
            "tag": "socks-in",
            "listen": "127.0.0.1",
            "port": profile.port().get(),
            "protocol": "socks",
            "settings": {
                "auth": "noauth",
                "udp": profile.udp_enabled(),
                "ip": "127.0.0.1"
            }
        }],
        "outbounds": [{
            "tag": "direct",
            "protocol": "freedom",
            "settings": {}
        }]
    })
}

fn sing_box_config(profile: LocalSocksProfile) -> Value {
    json!({
        "log": { "level": "warn" },
        "inbounds": [{
            "type": "socks",
            "tag": "socks-in",
            "listen": "127.0.0.1",
            "listen_port": profile.port().get()
        }],
        "outbounds": [{
            "type": "direct",
            "tag": "direct"
        }]
    })
}
