use std::fmt::{Display, Formatter};
use std::fs::{canonicalize, read};
use std::io;
use std::path::{Path, PathBuf};

use magies_platform::CpuArchitecture;
use sha2::{Digest, Sha256};

const MACHO_X86_64: u32 = 0x0100_0007;
const MACHO_AARCH64: u32 = 0x0100_000c;
const PE_X86_64: u32 = 0x8664;
const PE_AARCH64: u32 = 0xaa64;
const ELF_X86_64: u32 = 62;
const ELF_AARCH64: u32 = 183;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreBinaryFormat {
    MachO,
    Pe,
    Elf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sha256Hash([u8; 32]);

impl Sha256Hash {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn digest(contents: &[u8]) -> Self {
        Self(Sha256::digest(contents).into())
    }
}

impl Display for Sha256Hash {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreBinaryRequirement {
    architecture: CpuArchitecture,
    sha256: Sha256Hash,
}

impl CoreBinaryRequirement {
    #[must_use]
    pub const fn new(architecture: CpuArchitecture, sha256: Sha256Hash) -> Self {
        Self {
            architecture,
            sha256,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedCoreBinary {
    path: PathBuf,
    format: CoreBinaryFormat,
    architecture: CpuArchitecture,
    sha256: Sha256Hash,
}

impl ValidatedCoreBinary {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn format(&self) -> CoreBinaryFormat {
        self.format
    }

    #[must_use]
    pub const fn architecture(&self) -> CpuArchitecture {
        self.architecture
    }

    #[must_use]
    pub const fn sha256(&self) -> Sha256Hash {
        self.sha256
    }

    pub(crate) fn revalidate(&self) -> Result<Self, CoreBinaryError> {
        locate_core_binary(
            &self.path,
            CoreBinaryRequirement::new(self.architecture, self.sha256),
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CoreBinaryError {
    #[error("Core binary not found: {path:?}")]
    NotFound { path: PathBuf },
    #[error("failed to resolve Core binary {path:?}: {source}")]
    ResolveFailed { path: PathBuf, source: io::Error },
    #[error("Core binary is not a file: {path:?}")]
    NotAFile { path: PathBuf },
    #[error("failed to read Core binary {path:?}: {source}")]
    ReadFailed { path: PathBuf, source: io::Error },
    #[error("unsupported Core binary format: {path:?}")]
    UnsupportedFormat { path: PathBuf },
    #[error("malformed {format:?} Core binary: {path:?}")]
    MalformedBinary {
        path: PathBuf,
        format: CoreBinaryFormat,
    },
    #[error("unsupported {format:?} machine 0x{machine:x}: {path:?}")]
    UnsupportedArchitecture {
        path: PathBuf,
        format: CoreBinaryFormat,
        machine: u32,
    },
    #[error(
        "Core binary architecture mismatch for {path:?}: expected {expected:?}, got {actual:?}"
    )]
    ArchitectureMismatch {
        path: PathBuf,
        expected: CpuArchitecture,
        actual: CpuArchitecture,
    },
    #[error("Core binary SHA-256 mismatch for {path:?}: expected {expected}, got {actual}")]
    HashMismatch {
        path: PathBuf,
        expected: Sha256Hash,
        actual: Sha256Hash,
    },
}

/// Resolves and validates a configured Core executable before startup.
///
/// # Errors
///
/// Returns a typed error when the path cannot be read, the file format or CPU
/// architecture is unsupported, or the architecture/SHA-256 requirement does
/// not match the executable.
pub fn locate_core_binary(
    configured_path: impl AsRef<Path>,
    requirement: CoreBinaryRequirement,
) -> Result<ValidatedCoreBinary, CoreBinaryError> {
    let configured_path = configured_path.as_ref();
    let path = canonicalize(configured_path).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            CoreBinaryError::NotFound {
                path: configured_path.to_path_buf(),
            }
        } else {
            CoreBinaryError::ResolveFailed {
                path: configured_path.to_path_buf(),
                source,
            }
        }
    })?;
    if !path.is_file() {
        return Err(CoreBinaryError::NotAFile { path });
    }

    let contents = read(&path).map_err(|source| CoreBinaryError::ReadFailed {
        path: path.clone(),
        source,
    })?;
    let (format, architecture) = inspect_binary(&path, &contents)?;
    if architecture != requirement.architecture {
        return Err(CoreBinaryError::ArchitectureMismatch {
            path,
            expected: requirement.architecture,
            actual: architecture,
        });
    }

    let sha256 = Sha256Hash::digest(&contents);
    if sha256 != requirement.sha256 {
        return Err(CoreBinaryError::HashMismatch {
            path,
            expected: requirement.sha256,
            actual: sha256,
        });
    }

    Ok(ValidatedCoreBinary {
        path,
        format,
        architecture,
        sha256,
    })
}

fn inspect_binary(
    path: &Path,
    contents: &[u8],
) -> Result<(CoreBinaryFormat, CpuArchitecture), CoreBinaryError> {
    if contents.starts_with(&[0xcf, 0xfa, 0xed, 0xfe]) {
        return inspect_macho(path, contents);
    }
    if contents.starts_with(b"MZ") {
        return inspect_pe(path, contents);
    }
    if contents.starts_with(b"\x7fELF") {
        return inspect_elf(path, contents);
    }

    Err(CoreBinaryError::UnsupportedFormat {
        path: path.to_path_buf(),
    })
}

fn inspect_macho(
    path: &Path,
    contents: &[u8],
) -> Result<(CoreBinaryFormat, CpuArchitecture), CoreBinaryError> {
    let machine_bytes: [u8; 4] = contents
        .get(4..8)
        .ok_or_else(|| malformed(path, CoreBinaryFormat::MachO))?
        .try_into()
        .map_err(|_| malformed(path, CoreBinaryFormat::MachO))?;
    let machine = u32::from_le_bytes(machine_bytes);
    architecture(path, CoreBinaryFormat::MachO, machine)
}

fn inspect_pe(
    path: &Path,
    contents: &[u8],
) -> Result<(CoreBinaryFormat, CpuArchitecture), CoreBinaryError> {
    let offset_bytes: [u8; 4] = contents
        .get(0x3c..0x40)
        .ok_or_else(|| malformed(path, CoreBinaryFormat::Pe))?
        .try_into()
        .map_err(|_| malformed(path, CoreBinaryFormat::Pe))?;
    let offset = u32::from_le_bytes(offset_bytes);
    let offset = usize::try_from(offset).map_err(|_| malformed(path, CoreBinaryFormat::Pe))?;
    let header_end = offset
        .checked_add(6)
        .ok_or_else(|| malformed(path, CoreBinaryFormat::Pe))?;
    let header = contents
        .get(offset..header_end)
        .ok_or_else(|| malformed(path, CoreBinaryFormat::Pe))?;
    if !header.starts_with(b"PE\0\0") {
        return Err(malformed(path, CoreBinaryFormat::Pe));
    }
    let machine_bytes: [u8; 2] = header[4..6]
        .try_into()
        .map_err(|_| malformed(path, CoreBinaryFormat::Pe))?;
    let machine = u16::from_le_bytes(machine_bytes);
    architecture(path, CoreBinaryFormat::Pe, u32::from(machine))
}

fn inspect_elf(
    path: &Path,
    contents: &[u8],
) -> Result<(CoreBinaryFormat, CpuArchitecture), CoreBinaryError> {
    let header = contents
        .get(..20)
        .ok_or_else(|| malformed(path, CoreBinaryFormat::Elf))?;
    if header[4] != 2 {
        return Err(unsupported_architecture(path, CoreBinaryFormat::Elf, 0));
    }
    let machine_bytes: [u8; 2] = header[18..20]
        .try_into()
        .map_err(|_| malformed(path, CoreBinaryFormat::Elf))?;
    if header[5] != 1 {
        return Err(malformed(path, CoreBinaryFormat::Elf));
    }
    let machine = u16::from_le_bytes(machine_bytes);
    architecture(path, CoreBinaryFormat::Elf, u32::from(machine))
}

fn architecture(
    path: &Path,
    format: CoreBinaryFormat,
    machine: u32,
) -> Result<(CoreBinaryFormat, CpuArchitecture), CoreBinaryError> {
    let architecture = match (format, machine) {
        (CoreBinaryFormat::MachO, MACHO_X86_64)
        | (CoreBinaryFormat::Pe, PE_X86_64)
        | (CoreBinaryFormat::Elf, ELF_X86_64) => CpuArchitecture::X86_64,
        (CoreBinaryFormat::MachO, MACHO_AARCH64)
        | (CoreBinaryFormat::Pe, PE_AARCH64)
        | (CoreBinaryFormat::Elf, ELF_AARCH64) => CpuArchitecture::Aarch64,
        _ => return Err(unsupported_architecture(path, format, machine)),
    };
    Ok((format, architecture))
}

fn malformed(path: &Path, format: CoreBinaryFormat) -> CoreBinaryError {
    CoreBinaryError::MalformedBinary {
        path: path.to_path_buf(),
        format,
    }
}

fn unsupported_architecture(
    path: &Path,
    format: CoreBinaryFormat,
    machine: u32,
) -> CoreBinaryError {
    CoreBinaryError::UnsupportedArchitecture {
        path: path.to_path_buf(),
        format,
        machine,
    }
}
