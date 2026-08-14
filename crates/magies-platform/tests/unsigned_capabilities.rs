use magies_platform::{TargetPlatform, TunAvailability};

#[test]
fn unsigned_macos_builds_offer_tun_behind_an_authorization_prompt() {
    for architecture in ["x86_64", "aarch64"] {
        let target =
            TargetPlatform::parse("macos", architecture).expect("macOS target should be supported");

        // Measured against official sing-box 1.13.18: creating a `utun` fails
        // as a plain user and succeeds under `sudo`. Signing is not what the
        // device wants — privileges are.
        assert_eq!(
            target.unsigned_tun_availability(),
            TunAvailability::RequiresElevation
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
