use std::fs::write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::id;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use magies_core_runtime::Sha256Hash;
use magies_desktop_lib::core_control::{
    CoreSettings, CoreSettingsError, LazySingBoxControl, LazySingBoxError, bundled_core_in,
    describe,
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

#[test]
fn finds_a_core_shipped_beside_the_executable() {
    let layout = Layout::new("beside");
    let core = layout.write(&["sing-box"]);

    assert_eq!(bundled_core_in(layout.path()), Some(core));
}

#[test]
fn finds_a_core_in_the_macos_app_resources_directory() {
    // MgClash.app/Contents/MacOS/<exe> resolves the Core from ../Resources.
    let layout = Layout::new("resources");
    let core = layout.write(&["..", "Resources", "sing-box"]);

    assert_eq!(bundled_core_in(layout.path()), Some(core));
}

#[test]
fn reports_no_bundled_core_when_the_artifact_ships_without_one() {
    let layout = Layout::new("empty");

    assert_eq!(bundled_core_in(layout.path()), None);
}

#[test]
fn a_runtime_override_wins_over_the_bundled_core() {
    let layout = Layout::new("override");
    layout.write(&["sing-box"]);

    let settings = CoreSettings::resolve_from(
        Some(PathBuf::from("/opt/other-sing-box")),
        Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_owned()),
        layout.path(),
        None,
    )
    .unwrap();

    assert_eq!(settings.binary, PathBuf::from("/opt/other-sing-box"));
}

#[test]
fn a_bundled_core_uses_the_digest_pinned_at_build_time() {
    let layout = Layout::new("pinned");
    let core = layout.write(&["sing-box"]);

    let settings = CoreSettings::resolve_from(
        None,
        None,
        layout.path(),
        Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
    )
    .unwrap();

    assert_eq!(settings.binary, core);
    assert_eq!(settings.sha256, Sha256Hash::digest(b"abc"));
}

#[test]
fn a_bundled_core_without_a_pinned_digest_is_refused() {
    let layout = Layout::new("unpinned");
    layout.write(&["sing-box"]);

    assert!(matches!(
        CoreSettings::resolve_from(None, None, layout.path(), None),
        Err(CoreSettingsError::MissingSha256)
    ));
}

fn control(settings: Result<CoreSettings, CoreSettingsError>) -> LazySingBoxControl {
    LazySingBoxControl::new(
        settings,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        HEALTH_TIMEOUT,
    )
}

static FIXTURE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// A throwaway artifact layout used to exercise bundled-Core discovery.
///
/// The executable directory is nested one level inside the sandbox root, so the
/// `../Resources` candidate stays within this layout instead of reaching into
/// the shared temp directory and colliding with another test.
struct Layout {
    root: PathBuf,
    executable_directory: PathBuf,
}

impl Layout {
    fn new(name: &str) -> Self {
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("mgclash-layout-{name}-{}-{sequence}", id()));
        let executable_directory = root.join("MacOS");
        std::fs::create_dir_all(&executable_directory).unwrap();
        Self {
            root,
            executable_directory,
        }
    }

    fn path(&self) -> &PathBuf {
        &self.executable_directory
    }

    fn write(&self, segments: &[&str]) -> PathBuf {
        let mut path = self.executable_directory.clone();
        for segment in segments {
            path.push(segment);
        }
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        write(&path, b"not a real sing-box").unwrap();
        path
    }
}

impl Drop for Layout {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.root) {
            eprintln!("failed to remove layout {}: {error}", self.root.display());
        }
    }
}

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
