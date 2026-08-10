use std::fs::write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::id;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use magies_core_runtime::Sha256Hash;
use magies_desktop_lib::core_control::{
    CoreSettings, CoreSettingsError, LazySingBoxControl, LazySingBoxError, describe,
};
use magies_session::CoreSessionControl;

const HEALTH_TIMEOUT: Duration = Duration::from_millis(5);

#[test]
fn stopping_before_the_first_start_leaves_the_core_untouched() {
    let mut control = control(Err(CoreSettingsError::MissingBinaryPath));

    control.stop().unwrap();
}

#[test]
fn unresolved_settings_are_reported_when_the_user_connects() {
    let mut control = control(Err(CoreSettingsError::MissingBinaryPath));

    assert!(matches!(
        control.start(&PathBuf::from("session.json")),
        Err(LazySingBoxError::Settings(
            CoreSettingsError::MissingBinaryPath
        ))
    ));
}

#[test]
fn a_core_binary_that_fails_its_pin_is_reported_when_the_user_connects() {
    let binary = Fixture::new("wrong-hash");
    let mut control = control(Ok(CoreSettings {
        binary: binary.path.clone(),
        sha256: Sha256Hash::digest(b"a different sing-box"),
    }));

    assert!(matches!(
        control.start(&PathBuf::from("session.json")),
        Err(LazySingBoxError::Binary(_))
    ));
}

#[test]
fn a_pinned_core_binary_is_resolved_once_and_then_drives_the_session() {
    let binary = std::env::current_exe().unwrap();
    let sha256 = Sha256Hash::digest(&std::fs::read(&binary).unwrap());
    let mut control = control(Ok(CoreSettings { binary, sha256 }));

    // The pin resolves, so the failure comes from sing-box rejecting the
    // config rather than from binary validation.
    assert!(matches!(
        control.start(&PathBuf::from("missing-session.json")),
        Err(LazySingBoxError::Session(_))
    ));
    control.stop().unwrap();
}

#[test]
fn missing_environment_variables_are_reported_as_typed_settings_errors() {
    assert!(matches!(
        CoreSettings::from_values(None, None),
        Err(CoreSettingsError::MissingBinaryPath)
    ));
    assert!(matches!(
        CoreSettings::from_values(Some("/opt/sing-box".into()), None),
        Err(CoreSettingsError::MissingSha256)
    ));
    assert!(matches!(
        CoreSettings::from_values(Some("/opt/sing-box".into()), Some("not-a-hash".to_owned())),
        Err(CoreSettingsError::InvalidSha256(_))
    ));

    let settings = CoreSettings::from_values(
        Some("/opt/sing-box".into()),
        Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_owned()),
    )
    .unwrap();

    assert_eq!(settings.binary, PathBuf::from("/opt/sing-box"));
    assert_eq!(settings.sha256, Sha256Hash::digest(b"abc"));
}

#[test]
fn every_core_failure_carries_a_stable_code_and_a_readable_cause() {
    let mut unconfigured = control(Err(CoreSettingsError::MissingSha256));
    let error = unconfigured
        .start(&PathBuf::from("session.json"))
        .unwrap_err();

    assert_eq!(error.code(), "core_not_configured");
    assert_eq!(
        describe(&error),
        "the pinned sing-box binary is not configured: \
         MAGIES_SING_BOX_SHA256 must contain the pinned sing-box SHA-256"
    );

    let binary = Fixture::new("code-check");
    let mut rejected = control(Ok(CoreSettings {
        binary: binary.path.clone(),
        sha256: Sha256Hash::digest(b"a different sing-box"),
    }));

    assert_eq!(
        rejected
            .start(&PathBuf::from("session.json"))
            .unwrap_err()
            .code(),
        "core_binary_rejected"
    );
}

fn control(settings: Result<CoreSettings, CoreSettingsError>) -> LazySingBoxControl {
    LazySingBoxControl::new(
        settings,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        HEALTH_TIMEOUT,
    )
}

static FIXTURE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    path: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mgclash-core-{name}-{}-{sequence}", id()));
        write(&path, b"not a real sing-box").unwrap();
        Self { path }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.path) {
            eprintln!("failed to remove fixture {}: {error}", self.path.display());
        }
    }
}
