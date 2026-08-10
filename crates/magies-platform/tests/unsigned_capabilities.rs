use magies_platform::{TargetPlatform, TunAvailability};

#[test]
fn unsigned_macos_builds_disable_tun_before_startup() {
    for architecture in ["x86_64", "aarch64"] {
        let target =
            TargetPlatform::parse("macos", architecture).expect("macOS target should be supported");

        assert_eq!(
            target.unsigned_tun_availability(),
            TunAvailability::UnavailableInUnsignedBuild
        );
    }
}

#[test]
fn unsigned_windows_and_linux_tun_require_elevation() {
    for os in ["windows", "linux"] {
        let target =
            TargetPlatform::parse(os, "x86_64").expect("desktop target should be supported");

        assert_eq!(
            target.unsigned_tun_availability(),
            TunAvailability::RequiresElevation
        );
    }
}
