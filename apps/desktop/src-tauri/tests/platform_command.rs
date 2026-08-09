use magies_desktop_lib::{PlatformCommandError, platform_summary_for};

#[test]
fn returns_an_unsigned_macos_intel_summary() {
    let summary = platform_summary_for("macos", "x86_64").expect("target should be supported");

    assert_eq!(summary.artifact_identifier, "macos-x86_64");
    assert_eq!(summary.tun_availability, "unavailableInUnsignedBuild");
}

#[test]
fn returns_a_typed_error_for_an_unsupported_target() {
    let error = platform_summary_for("windows", "aarch64").unwrap_err();

    assert_eq!(
        error,
        PlatformCommandError {
            code: "unsupported_target",
            message: "unsupported target: windows-aarch64".to_owned(),
        }
    );
}
