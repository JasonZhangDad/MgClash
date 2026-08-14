use std::fs::write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::id;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use magies_core_runtime::Sha256Hash;
use magies_desktop_lib::core_control::{
    CoreSettings, CoreSettingsError, HostCoreControl, HostCoreError, LazySingBoxControl,
    LazySingBoxError, bundled_core_in, describe,
};
use magies_domain::CoreType;
use magies_session::CoreSessionControl;

const HEALTH_TIMEOUT: Duration = Duration::from_millis(5);

/// The lookup under test appends `EXE_SUFFIX`, so a fixture named plain
/// `sing-box` is invisible to it on Windows.
fn core_file_name() -> String {
    format!("sing-box{}", std::env::consts::EXE_SUFFIX)
}

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
    let core = layout.write(&[&core_file_name()]);

    assert_eq!(bundled_core_in(layout.path()), Some(core));
}

#[test]
fn finds_a_core_in_the_macos_app_resources_directory() {
    // MgClash.app/Contents/MacOS/<exe> resolves the Core from ../Resources.
    let layout = Layout::new("resources");
    let core = layout.write(&["..", "Resources", &core_file_name()]);

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
    layout.write(&[&core_file_name()]);

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
    let core = layout.write(&[&core_file_name()]);

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
    layout.write(&[&core_file_name()]);

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

#[test]
fn the_host_control_defaults_to_sing_box() {
    let mut control = HostCoreControl::from_env(address(), Duration::from_millis(10));

    // Nothing is configured in the test environment, so the failure names the
    // Core that was tried — which is how the default is observed.
    let error = control.start(Path::new("/nonexistent.json")).unwrap_err();

    assert!(
        matches!(error, HostCoreError::SingBox(_)),
        "expected the sing-box control by default, got {error:?}"
    );
}

#[test]
fn selecting_xray_routes_the_start_to_the_xray_control() {
    let mut control = HostCoreControl::from_env(address(), Duration::from_millis(10));

    control.select_core(CoreType::Xray);
    let error = control.start(Path::new("/nonexistent.json")).unwrap_err();

    assert!(
        matches!(error, HostCoreError::Xray(_)),
        "expected the Xray control after selecting it, got {error:?}"
    );
    assert_eq!(error.code(), "xray_unavailable");
}

#[test]
fn stopping_an_unstarted_host_control_is_not_an_error() {
    let mut control = HostCoreControl::from_env(address(), Duration::from_millis(10));

    // Both Cores are stopped so one switched away from mid-session cannot be
    // left running; neither was started here.
    assert!(control.stop().is_ok());
}

fn address() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 59_999))
}

#[cfg(unix)]
#[test]
fn only_a_sing_box_tun_start_is_elevated() {
    let mut control = HostCoreControl::from_env(address(), Duration::from_millis(10));

    // A TUN device needs privileges the app does not have — root for a macOS
    // `utun`, `CAP_NET_ADMIN` on Linux — so that start goes behind an
    // authorization prompt. Nothing else does: a plain session needs no
    // password, and Xray has no TUN inbound for root to open.
    assert!(!control.selects_elevated_start());

    control.select_network_mode(true);
    assert!(control.selects_elevated_start());

    control.select_core(CoreType::Xray);
    assert!(!control.selects_elevated_start());

    control.select_core(CoreType::SingBox);
    control.select_network_mode(false);
    assert!(!control.selects_elevated_start());
}

#[cfg(unix)]
#[test]
fn an_unconfigured_core_fails_the_same_way_on_the_elevated_path() {
    let mut control = HostCoreControl::from_env(address(), Duration::from_millis(10));

    // The pin is resolved before the prompt: an unverified binary would be
    // worse as root, not better.
    control.select_network_mode(true);
    let error = control.start(Path::new("/nonexistent.json")).unwrap_err();

    assert_eq!(error.code(), "core_not_configured");
}

#[cfg(unix)]
#[test]
fn nothing_is_reclaimed_when_no_elevated_core_was_left_behind() {
    let mut control = HostCoreControl::from_env(address(), Duration::from_millis(10))
        .with_runtime_directory(
            std::env::temp_dir().join(format!("mgclash-empty-runtime-{}", std::process::id())),
        );

    // The normal case: a clean shutdown leaves no PID file, and startup must
    // not ask the user about a Core that is not there.
    assert_eq!(control.reclaim_elevated_core(), None);
}
