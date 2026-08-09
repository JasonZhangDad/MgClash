use std::fs::{canonicalize, copy, create_dir, read, remove_dir, remove_file, write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use magies_core_runtime::{
    CoreBinaryRequirement, CoreRuntime, CoreState, Sha256Hash, ValidatedCoreBinary, XrayAdapter,
    XrayAdapterError, XrayOperation, locate_core_binary,
};
use magies_platform::CpuArchitecture;

static TEMPORARY_PATH_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static FAKE_XRAY: OnceLock<PathBuf> = OnceLock::new();

#[test]
fn reads_the_xray_version() {
    let adapter = XrayAdapter::new(validated_fake_xray());

    let version = adapter.version().unwrap();

    assert_eq!(version.as_str(), "26.3.27");
}

#[test]
fn validates_a_config_and_builds_a_running_process_spec() {
    let adapter = XrayAdapter::new(validated_fake_xray());
    let config = TemporaryPath::file("valid-xray-config.json", br#"{"valid": true}"#);

    let validated_config = adapter.validate_config(config.path()).unwrap();

    assert_eq!(
        validated_config.path(),
        canonicalize(config.path()).unwrap()
    );
    let mut runtime = CoreRuntime::default();
    runtime
        .start(&adapter.process_spec(&validated_config))
        .unwrap();
    assert_eq!(runtime.poll().unwrap(), CoreState::Running);
    runtime.stop().unwrap();
}

#[test]
fn reports_an_invalid_config_with_xray_stderr() {
    let adapter = XrayAdapter::new(validated_fake_xray());
    let config = TemporaryPath::file("invalid-xray-config.json", br#"{"valid": false}"#);

    let error = adapter.validate_config(config.path()).unwrap_err();

    assert!(matches!(
        error,
        XrayAdapterError::CommandFailed {
            operation: XrayOperation::ValidateConfig,
            status: Some(23),
            stderr,
        } if stderr.contains("invalid config")
    ));
}

#[test]
fn rejects_a_missing_or_non_file_config_before_running_xray() {
    let adapter = XrayAdapter::new(validated_fake_xray());
    let missing = unique_temporary_path("missing-xray-config.json");
    let directory = TemporaryPath::directory("xray-config-directory");
    let resolved_directory = canonicalize(directory.path()).unwrap();

    assert!(matches!(
        adapter.validate_config(&missing),
        Err(XrayAdapterError::ConfigNotFound { path }) if path == missing
    ));
    assert!(matches!(
        adapter.validate_config(directory.path()),
        Err(XrayAdapterError::ConfigNotAFile { path }) if path == resolved_directory
    ));
}

#[test]
fn rejects_output_that_is_not_an_xray_version() {
    let temporary_binary = TemporaryPath::copy_of("invalid-version-xray", fake_xray_path());
    let adapter = XrayAdapter::new(validated_binary(temporary_binary.path()));

    let error = adapter.version().unwrap_err();

    assert!(matches!(
        error,
        XrayAdapterError::InvalidVersionOutput { stdout } if stdout == "not Xray\n"
    ));
}

#[test]
fn reports_when_a_validated_xray_can_no_longer_be_started() {
    let temporary_binary = TemporaryPath::copy_of("removed-xray", fake_xray_path());
    let binary = validated_binary(temporary_binary.path());
    temporary_binary.remove();
    let adapter = XrayAdapter::new(binary);

    let error = adapter.version().unwrap_err();

    assert!(matches!(
        error,
        XrayAdapterError::CommandStartFailed {
            operation: XrayOperation::ReadVersion,
            ..
        }
    ));
}

fn validated_fake_xray() -> ValidatedCoreBinary {
    validated_binary(fake_xray_path())
}

fn validated_binary(path: &Path) -> ValidatedCoreBinary {
    let contents = read(path).expect("the fake Xray executable must be readable");
    locate_core_binary(
        path,
        CoreBinaryRequirement::new(build_architecture(), Sha256Hash::digest(&contents)),
    )
    .unwrap()
}

fn fake_xray_path() -> &'static Path {
    FAKE_XRAY.get_or_init(|| {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("fake_xray.rs");
        let output = unique_temporary_path(&format!(
            "compiled-fake-xray{}",
            std::env::consts::EXE_SUFFIX
        ));
        let status = Command::new("rustc")
            .args(["--edition=2024"])
            .arg(source)
            .arg("-o")
            .arg(&output)
            .status()
            .expect("rustc must be available to compile the fake Xray fixture");
        assert!(status.success(), "fake Xray fixture must compile");
        output
    })
}

fn build_architecture() -> CpuArchitecture {
    match std::env::consts::ARCH {
        "x86_64" => CpuArchitecture::X86_64,
        "aarch64" => CpuArchitecture::Aarch64,
        architecture => panic!("unsupported test architecture: {architecture}"),
    }
}

fn unique_temporary_path(name: &str) -> PathBuf {
    let sequence = TEMPORARY_PATH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "magies-xray-adapter-{}-{sequence}-{name}",
        std::process::id()
    ))
}

struct TemporaryPath {
    path: PathBuf,
    is_directory: bool,
}

impl TemporaryPath {
    fn file(name: &str, contents: &[u8]) -> Self {
        let path = unique_temporary_path(name);
        write(&path, contents).unwrap();
        Self {
            path,
            is_directory: false,
        }
    }

    fn copy_of(name: &str, source: &Path) -> Self {
        let path = unique_temporary_path(&format!("{name}{}", std::env::consts::EXE_SUFFIX));
        copy(source, &path).unwrap();
        Self {
            path,
            is_directory: false,
        }
    }

    fn directory(name: &str) -> Self {
        let path = unique_temporary_path(name);
        create_dir(&path).unwrap();
        Self {
            path,
            is_directory: true,
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn remove(mut self) {
        remove_file(&self.path).unwrap();
        self.path = PathBuf::new();
    }
}

impl Drop for TemporaryPath {
    fn drop(&mut self) {
        if self.path.as_os_str().is_empty() {
            return;
        }
        let result = if self.is_directory {
            remove_dir(&self.path)
        } else {
            remove_file(&self.path)
        };
        if let Err(error) = result {
            eprintln!(
                "failed to remove test path {}: {error}",
                self.path.display()
            );
        }
    }
}
