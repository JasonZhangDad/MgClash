#![cfg(unix)]
//! The Core control a TUN session uses where the app cannot open the device
//! itself. The prompt is the only part not exercised: the launcher here runs
//! the approved script directly, the way the prompt does once it is approved.

use std::fs::{self, File};
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use magies_core_runtime::elevated::{ElevatedCore, ElevatedCoreError, ElevationLauncher};
use magies_core_runtime::{
    CoreBinaryRequirement, Sha256Hash, ValidatedCoreBinary, locate_core_binary,
};
use magies_platform::CpuArchitecture;
use magies_session::{CoreSessionControl, ElevatedSingBoxControl, ElevatedSingBoxSessionError};

static FAKE_SING_BOX: OnceLock<PathBuf> = OnceLock::new();
static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// Runs the script the prompt would have run, and counts how often it was asked.
struct ShellLauncher(AtomicUsize);

impl ElevationLauncher for ShellLauncher {
    fn launch(&self, script: &str) -> Result<(), ElevatedCoreError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        let status = Command::new("/bin/sh").arg("-c").arg(script).status();
        assert!(status.is_ok_and(|status| status.success()), "{script}");
        Ok(())
    }
}

#[test]
fn an_invalid_config_is_rejected_before_anyone_is_asked_for_a_password() {
    let config = TemporaryConfig::new("elevated-invalid", br#"{"valid": false}"#);
    let (mut control, _directory) = control();

    let error = control.start(config.path()).unwrap_err();

    assert!(matches!(error, ElevatedSingBoxSessionError::Validate(_)));
    // Asking for a password to run a config sing-box already rejected would
    // train the user to approve prompts that cannot work.
    assert_eq!(error.code(), "sing_box_config_invalid");
}

#[test]
fn a_core_that_never_opens_its_port_is_stopped_again() {
    let config = TemporaryConfig::new("elevated-valid", br#"{"valid": true}"#);
    let (mut control, directory) = control();

    // The fake Core runs but listens on nothing, so health times out and the
    // elevated process has to be cleaned up rather than left running as root.
    let error = control.start(config.path()).unwrap_err();

    assert!(matches!(error, ElevatedSingBoxSessionError::Health(_)));
    assert_eq!(error.code(), "tun_core_unhealthy");
    assert!(!directory.join("core.pid").exists());
    fs::remove_dir_all(&directory).unwrap();
}

fn control() -> (ElevatedSingBoxControl<ShellLauncher>, PathBuf) {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "mgclash-elevated-control-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let control = ElevatedSingBoxControl::new(
        validated_fake_sing_box(),
        ElevatedCore::new(
            ShellLauncher(AtomicUsize::new(0)),
            directory.join("core.pid"),
            directory.join("core.log"),
        ),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), unused_port()),
        Duration::from_millis(50),
    );
    (control, directory)
}

fn unused_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn validated_fake_sing_box() -> ValidatedCoreBinary {
    let path = FAKE_SING_BOX.get_or_init(|| {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../magies-core-runtime/tests/fixtures/fake_sing_box.rs");
        let output = std::env::temp_dir().join(format!(
            "magies-session-elevated-fake-sing-box-{}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        let status = Command::new("rustc")
            .args(["--edition=2024"])
            .arg(source)
            .arg("-o")
            .arg(&output)
            .status()
            .unwrap();
        assert!(status.success());
        output
    });
    let contents = fs::read(path).unwrap();
    locate_core_binary(
        path,
        CoreBinaryRequirement::new(build_architecture(), Sha256Hash::digest(&contents)),
    )
    .unwrap()
}

fn build_architecture() -> CpuArchitecture {
    match std::env::consts::ARCH {
        "x86_64" => CpuArchitecture::X86_64,
        "aarch64" => CpuArchitecture::Aarch64,
        architecture => panic!("unsupported test architecture: {architecture}"),
    }
}

struct TemporaryConfig(PathBuf);

impl TemporaryConfig {
    fn new(name: &str, contents: &[u8]) -> Self {
        let path =
            std::env::temp_dir().join(format!("magies-session-{}-{name}.json", std::process::id()));
        let mut file = File::create(&path).unwrap();
        file.write_all(contents).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryConfig {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.0) {
            eprintln!(
                "failed to remove session Core test config {}: {error}",
                self.0.display()
            );
        }
    }
}
