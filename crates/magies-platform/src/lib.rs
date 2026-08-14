//! Target platform and build capability model.

pub mod linux_system_proxy;
pub mod macos_system_proxy;
pub mod network_path;
pub mod pac;
pub mod release;
pub mod system_proxy;
pub mod system_proxy_recovery;
pub mod windows_system_proxy;

use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatingSystem {
    MacOs,
    Windows,
    Linux,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuArchitecture {
    X86_64,
    Aarch64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TunAvailability {
    UnavailableInUnsignedBuild,
    RequiresElevation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetPlatform {
    os: OperatingSystem,
    architecture: CpuArchitecture,
}

impl TargetPlatform {
    /// Resolves a supported V0.1 target from Rust OS and architecture names.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError::UnsupportedTarget`] when the OS/CPU pair is not
    /// part of the V0.1 support matrix.
    pub fn parse(os: &str, architecture: &str) -> Result<Self, PlatformError> {
        let target = match (os, architecture) {
            ("macos", "x86_64") => Self {
                os: OperatingSystem::MacOs,
                architecture: CpuArchitecture::X86_64,
            },
            ("macos", "aarch64") => Self {
                os: OperatingSystem::MacOs,
                architecture: CpuArchitecture::Aarch64,
            },
            ("windows", "x86_64") => Self {
                os: OperatingSystem::Windows,
                architecture: CpuArchitecture::X86_64,
            },
            ("linux", "x86_64") => Self {
                os: OperatingSystem::Linux,
                architecture: CpuArchitecture::X86_64,
            },
            _ => {
                return Err(PlatformError::UnsupportedTarget {
                    os: os.to_owned(),
                    architecture: architecture.to_owned(),
                });
            }
        };

        Ok(target)
    }

    #[must_use]
    pub const fn os(self) -> OperatingSystem {
        self.os
    }

    #[must_use]
    pub const fn architecture(self) -> CpuArchitecture {
        self.architecture
    }

    #[must_use]
    pub const fn artifact_identifier(self) -> &'static str {
        match (self.os, self.architecture) {
            (OperatingSystem::MacOs, CpuArchitecture::X86_64) => "macos-x86_64",
            (OperatingSystem::MacOs, CpuArchitecture::Aarch64) => "macos-aarch64",
            (OperatingSystem::Windows, CpuArchitecture::X86_64) => "windows-x86_64",
            (OperatingSystem::Linux, CpuArchitecture::X86_64) => "linux-x86_64",
            (OperatingSystem::Windows | OperatingSystem::Linux, CpuArchitecture::Aarch64) => {
                unreachable!()
            }
        }
    }

    /// What an unsigned build can offer for TUN.
    ///
    /// Every supported platform can, given privileges: macOS opens a `utun`
    /// for root, Windows needs an administrator for Wintun, and Linux needs
    /// `CAP_NET_ADMIN`. The variant that refuses outright is kept for a target
    /// where signing, not privilege, is the obstacle.
    #[must_use]
    pub const fn unsigned_tun_availability(self) -> TunAvailability {
        match self.os {
            OperatingSystem::MacOs | OperatingSystem::Windows | OperatingSystem::Linux => {
                TunAvailability::RequiresElevation
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlatformError {
    UnsupportedTarget { os: String, architecture: String },
}

impl Display for PlatformError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedTarget { os, architecture } => {
                write!(formatter, "unsupported target: {os}-{architecture}")
            }
        }
    }
}

impl Error for PlatformError {}
