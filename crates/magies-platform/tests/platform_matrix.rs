use magies_platform::{CpuArchitecture, OperatingSystem, PlatformError, TargetPlatform};

#[test]
fn supports_every_v01_target() {
    let cases = [
        (
            "macos",
            "x86_64",
            OperatingSystem::MacOs,
            CpuArchitecture::X86_64,
        ),
        (
            "macos",
            "aarch64",
            OperatingSystem::MacOs,
            CpuArchitecture::Aarch64,
        ),
        (
            "windows",
            "x86_64",
            OperatingSystem::Windows,
            CpuArchitecture::X86_64,
        ),
        (
            "linux",
            "x86_64",
            OperatingSystem::Linux,
            CpuArchitecture::X86_64,
        ),
    ];

    for (os, arch, expected_os, expected_arch) in cases {
        let target = TargetPlatform::parse(os, arch).expect("target should be supported");

        assert_eq!(target.os(), expected_os);
        assert_eq!(target.architecture(), expected_arch);
    }
}

#[test]
fn uses_stable_artifact_identifiers() {
    let cases = [
        ("macos", "x86_64", "macos-x86_64"),
        ("macos", "aarch64", "macos-aarch64"),
        ("windows", "x86_64", "windows-x86_64"),
        ("linux", "x86_64", "linux-x86_64"),
    ];

    for (os, arch, expected) in cases {
        let target = TargetPlatform::parse(os, arch).expect("target should be supported");

        assert_eq!(target.artifact_identifier(), expected);
    }
}

#[test]
fn rejects_unsupported_operating_system() {
    let error = TargetPlatform::parse("freebsd", "x86_64").unwrap_err();

    assert_eq!(
        error,
        PlatformError::UnsupportedTarget {
            os: "freebsd".to_owned(),
            architecture: "x86_64".to_owned(),
        }
    );
}

#[test]
fn rejects_unsupported_architecture() {
    let error = TargetPlatform::parse("windows", "aarch64").unwrap_err();

    assert_eq!(
        error,
        PlatformError::UnsupportedTarget {
            os: "windows".to_owned(),
            architecture: "aarch64".to_owned(),
        }
    );
    assert_eq!(error.to_string(), "unsupported target: windows-aarch64");
}
