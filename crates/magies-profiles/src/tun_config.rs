use magies_domain::CoreType;
use magies_platform::OperatingSystem;
use serde_json::{Value, json};

use crate::GeneratedCoreConfig;

const IPV4_ADDRESS: &str = "172.19.0.1/30";
const IPV6_ADDRESS: &str = "fdfe:dcba:9876::1/126";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TunProfile {
    platform: OperatingSystem,
    ipv6_enabled: bool,
    mtu: u16,
    auto_route: bool,
    strict_route: bool,
}

impl TunProfile {
    /// Creates a validated Windows or Linux TUN profile.
    ///
    /// # Errors
    ///
    /// Returns a typed error for unsigned macOS, MTU outside `1280..=9000`,
    /// or strict routing without automatic routes.
    pub fn new(
        platform: OperatingSystem,
        ipv6_enabled: bool,
        mtu: u16,
        auto_route: bool,
        strict_route: bool,
    ) -> Result<Self, TunProfileError> {
        if platform == OperatingSystem::MacOs {
            return Err(TunProfileError::UnsupportedPlatform(platform));
        }
        if !(1280..=9000).contains(&mtu) {
            return Err(TunProfileError::InvalidMtu { mtu });
        }
        if strict_route && !auto_route {
            return Err(TunProfileError::StrictRouteRequiresAutoRoute);
        }
        Ok(Self {
            platform,
            ipv6_enabled,
            mtu,
            auto_route,
            strict_route,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SingBoxTunConfigGenerator;

impl SingBoxTunConfigGenerator {
    #[must_use]
    pub fn generate(profile: &TunProfile) -> GeneratedCoreConfig {
        let addresses = if profile.ipv6_enabled {
            vec![IPV4_ADDRESS, IPV6_ADDRESS]
        } else {
            vec![IPV4_ADDRESS]
        };
        let interface_name = match profile.platform {
            OperatingSystem::Windows => "MgClash",
            OperatingSystem::Linux => "mgclash0",
            OperatingSystem::MacOs => unreachable!("TunProfile rejects unsigned macOS"),
        };
        let mut inbound = json!({
            "type": "tun",
            "tag": "tun-in",
            "interface_name": interface_name,
            "address": addresses,
            "mtu": profile.mtu,
            "auto_route": profile.auto_route,
            "strict_route": profile.strict_route,
            "stack": "gvisor"
        });
        if profile.platform == OperatingSystem::Linux && profile.auto_route {
            inbound["auto_redirect"] = Value::Bool(true);
        }
        GeneratedCoreConfig::from_json(
            CoreType::SingBox,
            json!({
                "log": { "level": "warn" },
                "inbounds": [inbound],
                "outbounds": [{ "type": "direct", "tag": "direct" }],
                "route": { "auto_detect_interface": true, "final": "direct" }
            }),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TunProfileError {
    #[error("TUN is unavailable for unsigned {0:?} builds")]
    UnsupportedPlatform(OperatingSystem),
    #[error("TUN MTU must be between 1280 and 9000, got {mtu}")]
    InvalidMtu { mtu: u16 },
    #[error("strict TUN routing requires automatic routes")]
    StrictRouteRequiresAutoRoute,
}
