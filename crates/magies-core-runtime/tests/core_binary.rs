use std::fs::{create_dir, remove_dir, remove_file, write};
use std::path::PathBuf;
use std::process::id;
use std::sync::atomic::{AtomicUsize, Ordering};

use magies_core_runtime::{
    CoreBinaryError, CoreBinaryFormat, CoreBinaryRequirement, Sha256Hash, locate_core_binary,
};
use magies_platform::CpuArchitecture;

static FIXTURE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    path: PathBuf,
    is_directory: bool,
}

impl Fixture {
    fn file(name: &str, contents: &[u8]) -> Self {
        let path = fixture_path(name);
        write(&path, contents).expect("binary fixture must be writable");
        Self {
            path,
            is_directory: false,
        }
    }

    fn directory(name: &str) -> Self {
        let path = fixture_path(name);
        create_dir(&path).expect("directory fixture must be creatable");
        Self {
            path,
            is_directory: true,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let result = if self.is_directory {
            remove_dir(&self.path)
        } else {
            remove_file(&self.path)
        };
        if let Err(error) = result {
            eprintln!("failed to remove fixture {}: {error}", self.path.display());
        }
    }
}

fn fixture_path(name: &str) -> PathBuf {
    let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("mgclash-core-{name}-{}-{sequence}", id()))
}

#[test]
fn recognizes_supported_desktop_binary_formats_and_architectures() {
    let cases = [
        (
            "macho-x86_64",
            macho(0x0100_0007),
            CoreBinaryFormat::MachO,
            CpuArchitecture::X86_64,
        ),
        (
            "macho-aarch64",
            macho(0x0100_000c),
            CoreBinaryFormat::MachO,
            CpuArchitecture::Aarch64,
        ),
        (
            "pe-x86_64",
            pe(0x8664),
            CoreBinaryFormat::Pe,
            CpuArchitecture::X86_64,
        ),
        (
            "pe-aarch64",
            pe(0xaa64),
            CoreBinaryFormat::Pe,
            CpuArchitecture::Aarch64,
        ),
        (
            "elf-x86_64",
            elf(62),
            CoreBinaryFormat::Elf,
            CpuArchitecture::X86_64,
        ),
        (
            "elf-aarch64",
            elf(183),
            CoreBinaryFormat::Elf,
            CpuArchitecture::Aarch64,
        ),
    ];

    for (name, contents, expected_format, expected_architecture) in cases {
        let fixture = Fixture::file(name, &contents);
        let expected_hash = Sha256Hash::digest(&contents);

        let binary = locate_core_binary(
            &fixture.path,
            CoreBinaryRequirement::new(expected_architecture, expected_hash),
        )
        .unwrap();

        assert_eq!(binary.path(), fixture.path.canonicalize().unwrap());
        assert_eq!(binary.format(), expected_format);
        assert_eq!(binary.architecture(), expected_architecture);
        assert_eq!(binary.sha256(), expected_hash);
    }
}

#[test]
fn rejects_a_binary_for_the_wrong_architecture() {
    let contents = macho(0x0100_0007);
    let fixture = Fixture::file("wrong-architecture", &contents);

    let error = locate_core_binary(
        &fixture.path,
        CoreBinaryRequirement::new(CpuArchitecture::Aarch64, Sha256Hash::digest(&contents)),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        CoreBinaryError::ArchitectureMismatch {
            expected: CpuArchitecture::Aarch64,
            actual: CpuArchitecture::X86_64,
            ..
        }
    ));
}

#[test]
fn rejects_a_binary_with_the_wrong_hash() {
    let contents = elf(62);
    let fixture = Fixture::file("wrong-hash", &contents);
    let expected = Sha256Hash::digest(b"different binary");

    let error = locate_core_binary(
        &fixture.path,
        CoreBinaryRequirement::new(CpuArchitecture::X86_64, expected),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        CoreBinaryError::HashMismatch { expected: actual_expected, actual, .. }
            if actual_expected == expected && actual == Sha256Hash::digest(&contents)
    ));
}

#[test]
fn rejects_missing_paths_and_directories() {
    let missing = fixture_path("missing");
    let directory = Fixture::directory("directory");
    let requirement =
        CoreBinaryRequirement::new(CpuArchitecture::X86_64, Sha256Hash::digest(b"irrelevant"));

    assert!(matches!(
        locate_core_binary(&missing, requirement),
        Err(CoreBinaryError::NotFound { path }) if path == missing
    ));
    assert!(matches!(
        locate_core_binary(&directory.path, requirement),
        Err(CoreBinaryError::NotAFile { path }) if path == directory.path.canonicalize().unwrap()
    ));
}

#[test]
fn rejects_unknown_malformed_and_unsupported_binaries() {
    let unknown_contents = b"not a desktop executable";
    let unknown = Fixture::file("unknown", unknown_contents);
    let malformed_contents = b"MZ";
    let malformed = Fixture::file("malformed", malformed_contents);
    let unsupported_contents = pe(0x014c);
    let unsupported = Fixture::file("unsupported-architecture", &unsupported_contents);

    assert!(matches!(
        locate_core_binary(
            &unknown.path,
            requirement(CpuArchitecture::X86_64, unknown_contents)
        ),
        Err(CoreBinaryError::UnsupportedFormat { .. })
    ));
    assert!(matches!(
        locate_core_binary(
            &malformed.path,
            requirement(CpuArchitecture::X86_64, malformed_contents)
        ),
        Err(CoreBinaryError::MalformedBinary {
            format: CoreBinaryFormat::Pe,
            ..
        })
    ));
    assert!(matches!(
        locate_core_binary(
            &unsupported.path,
            requirement(CpuArchitecture::X86_64, &unsupported_contents)
        ),
        Err(CoreBinaryError::UnsupportedArchitecture {
            format: CoreBinaryFormat::Pe,
            machine: 0x014c,
            ..
        })
    ));
}

#[test]
fn formats_sha256_as_lowercase_hex() {
    assert_eq!(
        Sha256Hash::digest(b"abc").to_string(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        Sha256Hash::from_bytes([0xab; 32]).to_string(),
        "abababababababababababababababababababababababababababababababab"
    );
}

fn requirement(architecture: CpuArchitecture, contents: &[u8]) -> CoreBinaryRequirement {
    CoreBinaryRequirement::new(architecture, Sha256Hash::digest(contents))
}

fn macho(machine: u32) -> Vec<u8> {
    let mut contents = vec![0_u8; 32];
    contents[0..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
    contents[4..8].copy_from_slice(&machine.to_le_bytes());
    contents
}

fn pe(machine: u16) -> Vec<u8> {
    const PE_OFFSET: usize = 64;
    let mut contents = vec![0_u8; 128];
    contents[0..2].copy_from_slice(b"MZ");
    contents[0x3c..0x40].copy_from_slice(&64_u32.to_le_bytes());
    contents[PE_OFFSET..PE_OFFSET + 4].copy_from_slice(b"PE\0\0");
    contents[PE_OFFSET + 4..PE_OFFSET + 6].copy_from_slice(&machine.to_le_bytes());
    contents
}

fn elf(machine: u16) -> Vec<u8> {
    let mut contents = vec![0_u8; 64];
    contents[0..4].copy_from_slice(b"\x7fELF");
    contents[4] = 2;
    contents[5] = 1;
    contents[18..20].copy_from_slice(&machine.to_le_bytes());
    contents
}
