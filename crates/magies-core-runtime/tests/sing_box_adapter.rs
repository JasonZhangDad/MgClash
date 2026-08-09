mod common;

use std::fs::canonicalize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use magies_core_runtime::{
    CoreRuntime, CoreState, SingBoxAdapter, SingBoxAdapterError, SingBoxOperation,
    ValidatedCoreBinary,
};

use common::{TemporaryPath, compile_fixture, unique_temporary_path, validated_binary};

static FAKE_SING_BOX: OnceLock<PathBuf> = OnceLock::new();

#[test]
fn reads_the_sing_box_version() {
    let adapter = SingBoxAdapter::new(validated_fake_sing_box());

    let version = adapter.version().unwrap();

    assert_eq!(version.as_str(), "1.13.18");
}

#[test]
fn validates_a_config_and_builds_a_running_process_spec() {
    let adapter = SingBoxAdapter::new(validated_fake_sing_box());
    let config = TemporaryPath::file("valid-sing-box-config.json", br#"{"valid": true}"#);

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
fn reports_an_invalid_config_with_sing_box_stderr() {
    let adapter = SingBoxAdapter::new(validated_fake_sing_box());
    let config = TemporaryPath::file("invalid-sing-box-config.json", br#"{"valid": false}"#);

    let error = adapter.validate_config(config.path()).unwrap_err();

    assert!(matches!(
        error,
        SingBoxAdapterError::CommandFailed {
            operation: SingBoxOperation::ValidateConfig,
            status: Some(33),
            stderr,
        } if stderr.contains("invalid config")
    ));
}

#[test]
fn rejects_a_missing_or_non_file_config_before_running_sing_box() {
    let adapter = SingBoxAdapter::new(validated_fake_sing_box());
    let missing = unique_temporary_path("missing-sing-box-config.json");
    let directory = TemporaryPath::directory("sing-box-config-directory");
    let resolved_directory = canonicalize(directory.path()).unwrap();

    assert!(matches!(
        adapter.validate_config(&missing),
        Err(SingBoxAdapterError::ConfigNotFound { path }) if path == missing
    ));
    assert!(matches!(
        adapter.validate_config(directory.path()),
        Err(SingBoxAdapterError::ConfigNotAFile { path }) if path == resolved_directory
    ));
}

#[test]
fn rejects_output_that_is_not_a_sing_box_version() {
    let temporary_binary = TemporaryPath::copy_of("invalid-version-sing-box", fake_sing_box_path());
    let adapter = SingBoxAdapter::new(validated_binary(temporary_binary.path()));

    let error = adapter.version().unwrap_err();

    assert!(matches!(
        error,
        SingBoxAdapterError::InvalidVersionOutput { stdout } if stdout == "not sing-box\n"
    ));
}

#[test]
fn reports_when_a_validated_sing_box_can_no_longer_be_started() {
    let temporary_binary = TemporaryPath::copy_of("removed-sing-box", fake_sing_box_path());
    let binary = validated_binary(temporary_binary.path());
    temporary_binary.remove();
    let adapter = SingBoxAdapter::new(binary);

    let error = adapter.version().unwrap_err();

    assert!(matches!(
        error,
        SingBoxAdapterError::CommandStartFailed {
            operation: SingBoxOperation::ReadVersion,
            ..
        }
    ));
}

fn validated_fake_sing_box() -> ValidatedCoreBinary {
    validated_binary(fake_sing_box_path())
}

fn fake_sing_box_path() -> &'static Path {
    FAKE_SING_BOX.get_or_init(|| compile_fixture("fake_sing_box.rs", "compiled-fake-sing-box"))
}
