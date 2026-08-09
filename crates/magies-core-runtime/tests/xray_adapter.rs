mod common;

use std::fs::canonicalize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use magies_core_runtime::{
    CoreRuntime, CoreState, ValidatedCoreBinary, XrayAdapter, XrayAdapterError, XrayOperation,
};

use common::{TemporaryPath, compile_fixture, unique_temporary_path, validated_binary};

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

fn fake_xray_path() -> &'static Path {
    FAKE_XRAY.get_or_init(|| compile_fixture("fake_xray.rs", "compiled-fake-xray"))
}
