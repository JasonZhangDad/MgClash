//! Which Core can serve which node, and which one to use.
//!
//! PRD section 14 requires one internal capability matrix and forbids scattering
//! `if core == xray` checks through the UI. Everything that depends on a Core's
//! abilities is answered here, so callers ask *what is possible* rather than
//! *which Core is this*.

use std::fmt::{Display, Formatter};

use magies_domain::{CoreType, ProxyProtocol};
use magies_platform::CpuArchitecture;
use thiserror::Error;

/// What one Core can do with one protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "PRD 14.2 names these four capability flags; collapsing them into a bitset would hide the spec"
)]
pub struct CoreCapability {
    pub protocol: ProxyProtocol,
    pub supports_tun: bool,
    pub supports_udp: bool,
    pub supports_reality: bool,
    pub supports_mux: bool,
    /// Whether the Core can verify a server certificate by SHA-256 digest.
    pub supports_certificate_pin: bool,
    /// The architectures an official build exists for.
    pub architectures: &'static [CpuArchitecture],
}

/// Both V0.1 architectures; neither Core ships for only one of them.
const ALL_ARCHITECTURES: &[CpuArchitecture] = &[CpuArchitecture::X86_64, CpuArchitecture::Aarch64];

/// sing-box carries the general case: every V0.1 protocol, plus the TUN inbound
/// the desktop TUN mode needs.
const SING_BOX_CAPABILITIES: &[CoreCapability] = &[
    capability(ProxyProtocol::Vless, true, false),
    capability(ProxyProtocol::Vmess, true, false),
    capability(ProxyProtocol::Trojan, true, false),
    capability(ProxyProtocol::Shadowsocks, true, false),
    capability(ProxyProtocol::Hysteria2, true, false),
    capability(ProxyProtocol::Tuic, true, false),
    capability(ProxyProtocol::Socks, true, false),
    capability(ProxyProtocol::Http, true, false),
    capability(ProxyProtocol::WireGuard, true, false),
    // Added in sing-box 1.12.0, carried by the pinned 1.13.18. Xray ships no
    // AnyTLS outbound at all, so this stays sing-box-only.
    capability(ProxyProtocol::AnyTls, true, false),
    // Added in sing-box 1.13.0, carried by the pinned 1.13.18. Xray ships no
    // Naive outbound at all, so this stays sing-box-only.
    capability(ProxyProtocol::Naive, true, false),
];

/// Xray covers the stream protocols but has no Hysteria2 or TUIC outbound and
/// no TUN inbound of its own — TUN there needs an external tun2socks.
///
/// `WireGuard` is deliberately absent too: Xray does ship a `wireguard`
/// outbound, but nothing in this codebase's Xray patterns pins a verified
/// field-for-field shape for it the way the other outbounds are pinned
/// against upstream, so it stays sing-box-only rather than guessing.
const XRAY_CAPABILITIES: &[CoreCapability] = &[
    capability(ProxyProtocol::Vless, false, true),
    capability(ProxyProtocol::Vmess, false, true),
    capability(ProxyProtocol::Trojan, false, true),
    capability(ProxyProtocol::Shadowsocks, false, true),
    capability(ProxyProtocol::Socks, false, true),
    capability(ProxyProtocol::Http, false, true),
];

const fn capability(
    protocol: ProxyProtocol,
    supports_tun: bool,
    supports_certificate_pin: bool,
) -> CoreCapability {
    CoreCapability {
        protocol,
        supports_tun,
        supports_udp: true,
        supports_reality: true,
        supports_mux: true,
        supports_certificate_pin,
        architectures: ALL_ARCHITECTURES,
    }
}

/// What a session needs from a Core before it can start.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreRequirements {
    pub protocol: ProxyProtocol,
    pub tun: bool,
    /// Whether the node pins its server certificate by SHA-256.
    pub certificate_pin: bool,
    /// Whether the node uses XHTTP (Xray-only on the pinned Core versions).
    pub xhttp: bool,
    pub architecture: CpuArchitecture,
}

impl CoreRequirements {
    #[must_use]
    pub const fn new(protocol: ProxyProtocol, tun: bool, architecture: CpuArchitecture) -> Self {
        Self {
            protocol,
            tun,
            certificate_pin: false,
            xhttp: false,
            architecture,
        }
    }

    /// The same requirements for a node that pins its server certificate.
    #[must_use]
    pub const fn with_certificate_pin(self) -> Self {
        Self {
            certificate_pin: true,
            ..self
        }
    }

    /// The same requirements for a node that speaks XHTTP.
    #[must_use]
    pub const fn with_xhttp(self) -> Self {
        Self {
            xhttp: true,
            ..self
        }
    }
}

/// What the user asked for in settings.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CorePreference {
    /// Let the matrix decide from the node and the enabled features.
    #[default]
    Auto,
    /// Use this Core, or fail with the reason it cannot serve the node.
    Fixed(CoreType),
}

/// The single source of truth for Core abilities.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CoreCapabilityMatrix;

impl CoreCapabilityMatrix {
    /// Every capability entry for one Core.
    #[must_use]
    pub const fn capabilities(core: CoreType) -> &'static [CoreCapability] {
        match core {
            CoreType::SingBox => SING_BOX_CAPABILITIES,
            CoreType::Xray => XRAY_CAPABILITIES,
        }
    }

    /// The entry for one Core and protocol, if the Core supports it at all.
    #[must_use]
    pub fn capability(core: CoreType, protocol: ProxyProtocol) -> Option<&'static CoreCapability> {
        Self::capabilities(core)
            .iter()
            .find(|capability| capability.protocol == protocol)
    }

    /// Whether `core` can serve exactly these requirements.
    #[must_use]
    pub fn supports(core: CoreType, requirements: CoreRequirements) -> bool {
        Self::rejection(core, requirements).is_none()
    }

    /// Why `core` cannot serve these requirements, or `None` when it can.
    ///
    /// Callers surface this instead of a bare boolean so the UI can explain the
    /// refusal without knowing which Core is involved.
    #[must_use]
    pub fn rejection(core: CoreType, requirements: CoreRequirements) -> Option<CoreRejection> {
        let Some(capability) = Self::capability(core, requirements.protocol) else {
            return Some(CoreRejection::ProtocolUnsupported {
                core,
                protocol: requirements.protocol,
            });
        };
        if requirements.tun && !capability.supports_tun {
            return Some(CoreRejection::TunUnsupported { core });
        }
        if requirements.certificate_pin && !capability.supports_certificate_pin {
            return Some(CoreRejection::CertificatePinUnsupported { core });
        }
        // XHTTP lives outside the per-protocol capability table: every Xray
        // stream protocol can carry it, and the pinned sing-box build cannot.
        if requirements.xhttp && matches!(core, CoreType::SingBox) {
            return Some(CoreRejection::XhttpUnsupported { core });
        }
        if !capability
            .architectures
            .contains(&requirements.architecture)
        {
            return Some(CoreRejection::ArchitectureUnsupported {
                core,
                architecture: requirements.architecture,
            });
        }
        None
    }

    /// Picks the Core to run, honouring an explicit choice.
    ///
    /// Auto prefers sing-box, which PRD 14.1 makes responsible for the general
    /// case; Xray is only chosen automatically when sing-box cannot serve the
    /// requirements. A fixed choice is never silently replaced — a Core the user
    /// picked that cannot run the node is an error they need to see.
    ///
    /// # Errors
    ///
    /// Returns the reason no usable Core exists, or why the chosen one cannot
    /// serve the node.
    pub fn select(
        preference: CorePreference,
        requirements: CoreRequirements,
    ) -> Result<CoreType, CoreSelectionError> {
        match preference {
            CorePreference::Fixed(core) => match Self::rejection(core, requirements) {
                None => Ok(core),
                Some(rejection) => Err(CoreSelectionError::ChosenCoreUnusable { rejection }),
            },
            CorePreference::Auto => [CoreType::SingBox, CoreType::Xray]
                .into_iter()
                .find(|core| Self::supports(*core, requirements))
                .ok_or(CoreSelectionError::NoUsableCore {
                    protocol: requirements.protocol,
                    tun: requirements.tun,
                    certificate_pin: requirements.certificate_pin,
                    xhttp: requirements.xhttp,
                }),
        }
    }
}

/// Why one Core was ruled out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreRejection {
    ProtocolUnsupported {
        core: CoreType,
        protocol: ProxyProtocol,
    },
    TunUnsupported {
        core: CoreType,
    },
    CertificatePinUnsupported {
        core: CoreType,
    },
    XhttpUnsupported {
        core: CoreType,
    },
    ArchitectureUnsupported {
        core: CoreType,
        architecture: CpuArchitecture,
    },
}

impl Display for CoreRejection {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProtocolUnsupported { core, protocol } => write!(
                formatter,
                "{} does not support {}",
                core_name(*core),
                protocol_name(*protocol)
            ),
            Self::TunUnsupported { core } => {
                write!(formatter, "{} cannot provide TUN mode", core_name(*core))
            }
            Self::CertificatePinUnsupported { core } => write!(
                formatter,
                "{} cannot verify a pinned certificate digest",
                core_name(*core)
            ),
            Self::XhttpUnsupported { core } => write!(
                formatter,
                "{} does not support XHTTP transport",
                core_name(*core)
            ),
            Self::ArchitectureUnsupported { core, architecture } => write!(
                formatter,
                "{} has no build for {}",
                core_name(*core),
                architecture_name(*architecture)
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum CoreSelectionError {
    #[error("the selected Core cannot run this node: {rejection}")]
    ChosenCoreUnusable { rejection: CoreRejection },
    #[error(
        "no available Core supports {} with TUN {}{}{}",
        protocol_name(*protocol),
        if *tun { "on" } else { "off" },
        if *certificate_pin { " and a pinned certificate" } else { "" },
        if *xhttp { " and XHTTP transport" } else { "" }
    )]
    NoUsableCore {
        protocol: ProxyProtocol,
        tun: bool,
        certificate_pin: bool,
        xhttp: bool,
    },
}

/// The stable name shown in the UI and stored in settings.
#[must_use]
pub const fn core_name(core: CoreType) -> &'static str {
    match core {
        CoreType::SingBox => "sing-box",
        CoreType::Xray => "xray",
    }
}

/// Parses the stable name exchanged with the desktop webview.
#[must_use]
pub fn parse_core_name(value: &str) -> Option<CoreType> {
    match value {
        "sing-box" => Some(CoreType::SingBox),
        "xray" => Some(CoreType::Xray),
        _ => None,
    }
}

#[must_use]
pub const fn protocol_name(protocol: ProxyProtocol) -> &'static str {
    match protocol {
        ProxyProtocol::Vless => "VLESS",
        ProxyProtocol::Vmess => "VMess",
        ProxyProtocol::Trojan => "Trojan",
        ProxyProtocol::Shadowsocks => "Shadowsocks",
        ProxyProtocol::Hysteria2 => "Hysteria2",
        ProxyProtocol::Tuic => "TUIC",
        ProxyProtocol::Socks => "SOCKS5",
        ProxyProtocol::Http => "HTTP",
        ProxyProtocol::WireGuard => "WireGuard",
        ProxyProtocol::AnyTls => "AnyTLS",
        ProxyProtocol::Naive => "Naive",
    }
}

#[must_use]
pub const fn architecture_name(architecture: CpuArchitecture) -> &'static str {
    match architecture {
        CpuArchitecture::X86_64 => "x86_64",
        CpuArchitecture::Aarch64 => "aarch64",
    }
}
