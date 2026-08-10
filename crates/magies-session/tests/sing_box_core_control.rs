use std::fs::{self, File};
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use magies_core_runtime::{
    CoreBinaryRequirement, Sha256Hash, SingBoxAdapter, ValidatedCoreBinary, locate_core_binary,
};
use magies_platform::CpuArchitecture;
use magies_session::{CoreSessionControl, SingBoxCoreControl, SingBoxCoreSessionError};

static FAKE_SING_BOX: OnceLock<PathBuf> = OnceLock::new();

#[test]
fn rejects_invalid_config_before_starting_core() {
    let config = TemporaryConfig::new("invalid", br#"{"valid": false}"#);
    let mut control = control();

    assert!(matches!(
        control.start(config.path()),
        Err(SingBoxCoreSessionError::Validate(_))
    ));
}

#[test]
fn health_timeout_stops_the_started_core() {
    let config = TemporaryConfig::new("valid", br#"{"valid": true}"#);
    let mut control = control();

    assert!(matches!(
        control.start(config.path()),
        Err(SingBoxCoreSessionError::Health(_))
    ));
    control.stop().unwrap();
}

fn control() -> SingBoxCoreControl {
    SingBoxCoreControl::new(
        SingBoxAdapter::new(validated_fake_sing_box()),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        Duration::from_millis(5),
    )
}

fn validated_fake_sing_box() -> ValidatedCoreBinary {
    let path = FAKE_SING_BOX.get_or_init(|| {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../magies-core-runtime/tests/fixtures/fake_sing_box.rs");
        let output = std::env::temp_dir().join(format!(
            "magies-session-fake-sing-box-{}{}",
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
