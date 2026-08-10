#[allow(dead_code)]
mod common;

use std::fs;
use std::path::PathBuf;

use common::{TemporaryPath, unique_temporary_path};
use magies_core_runtime::{AtomicRuntimeConfig, RuntimeConfigFileError};

const SECRET_CONFIG: &[u8] = br#"{"password":"never-print-this"}"#;

#[test]
fn atomically_publishes_and_removes_a_runtime_config() {
    let directory = TemporaryPath::directory("atomic-runtime-config");
    let path = directory.path().join("config.json");

    let config = AtomicRuntimeConfig::write(&path, SECRET_CONFIG).unwrap();

    assert_eq!(config.path(), path);
    assert_eq!(fs::read(config.path()).unwrap(), SECRET_CONFIG);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    drop(config);
    assert!(!path.exists());
}

#[cfg(unix)]
#[test]
fn limits_runtime_config_to_the_current_user_on_unix() {
    use std::os::unix::fs::PermissionsExt;

    let directory = TemporaryPath::directory("private-runtime-config");
    let config =
        AtomicRuntimeConfig::write(directory.path().join("config.json"), SECRET_CONFIG).unwrap();
    let mode = fs::metadata(config.path()).unwrap().permissions().mode() & 0o777;

    assert_eq!(mode, 0o600);
}

#[test]
fn creates_the_runtime_directory_when_missing() {
    let directory = unique_temporary_path("created-runtime-directory");
    let path = directory.join("config.json");

    let config = AtomicRuntimeConfig::write(&path, SECRET_CONFIG).unwrap();

    assert_eq!(fs::read(config.path()).unwrap(), SECRET_CONFIG);
    drop(config);
    fs::remove_dir(directory).unwrap();
}

#[test]
fn refuses_to_overwrite_an_existing_config() {
    let existing = TemporaryPath::file("existing-runtime-config.json", b"known-good-config");

    let error = AtomicRuntimeConfig::write(existing.path(), SECRET_CONFIG).unwrap_err();

    assert!(matches!(
        error,
        RuntimeConfigFileError::AlreadyExists { .. }
    ));
    assert_eq!(fs::read(existing.path()).unwrap(), b"known-good-config");
    assert!(!format!("{error:?} {error}").contains("never-print-this"));
}

#[test]
fn rejects_empty_config_before_touching_the_file_system() {
    let path = unique_temporary_path("empty-runtime-config.json");

    assert!(matches!(
        AtomicRuntimeConfig::write(&path, &[]),
        Err(RuntimeConfigFileError::EmptyConfig)
    ));
    assert!(!path.exists());
}

#[test]
fn reports_an_invalid_runtime_directory_without_secret_data() {
    let parent_file = TemporaryPath::file("runtime-parent-file", b"not-a-directory");
    let path = parent_file.path().join("config.json");

    let error = AtomicRuntimeConfig::write(path, SECRET_CONFIG).unwrap_err();
    let diagnostics = format!("{error:?} {error}");

    assert!(matches!(
        error,
        RuntimeConfigFileError::CreateDirectory { .. }
    ));
    assert!(!diagnostics.contains("never-print-this"));
}

#[test]
fn explicit_removal_disarms_drop_cleanup() {
    let directory = TemporaryPath::directory("explicit-runtime-config-removal");
    let path = directory.path().join("config.json");
    let config = AtomicRuntimeConfig::write(&path, SECRET_CONFIG).unwrap();

    config.remove().unwrap();

    assert!(!path.exists());
}

#[test]
fn runtime_config_debug_output_contains_no_config_data() {
    let directory = TemporaryPath::directory("runtime-config-debug");
    let config =
        AtomicRuntimeConfig::write(directory.path().join("config.json"), SECRET_CONFIG).unwrap();
    let debug = format!("{config:?}");

    assert!(debug.contains("config.json"));
    assert!(!debug.contains("never-print-this"));
}

#[test]
fn drop_accepts_a_runtime_config_already_removed_by_the_core() {
    let directory = TemporaryPath::directory("runtime-config-removed-by-core");
    let config =
        AtomicRuntimeConfig::write(directory.path().join("config.json"), SECRET_CONFIG).unwrap();
    fs::remove_file(config.path()).unwrap();

    drop(config);

    assert!(fs::read_dir(directory.path()).unwrap().next().is_none());
}

#[test]
fn reports_publish_failures_without_leaving_a_temporary_file() {
    let directory = TemporaryPath::directory("runtime-config-publish-failure");
    let path = directory.path().join("x".repeat(300));

    let error = AtomicRuntimeConfig::write(path, SECRET_CONFIG).unwrap_err();

    assert!(matches!(error, RuntimeConfigFileError::Publish { .. }));
    assert!(fs::read_dir(directory.path()).unwrap().next().is_none());
}

#[test]
fn explicit_removal_surfaces_file_system_errors() {
    let directory = TemporaryPath::directory("runtime-config-removal-failure");
    let config_path = directory.path().join("config.json");
    let config = AtomicRuntimeConfig::write(&config_path, SECRET_CONFIG).unwrap();
    fs::remove_file(&config_path).unwrap();
    fs::create_dir(&config_path).unwrap();

    let error = config.remove().unwrap_err();

    assert!(matches!(error, RuntimeConfigFileError::Remove { .. }));
    fs::remove_dir(config_path).unwrap();
}

#[test]
fn accepts_a_config_path_relative_to_the_current_directory() {
    let path = PathBuf::from(format!(
        ".magies-relative-runtime-config-{}.json",
        std::process::id()
    ));

    let config = AtomicRuntimeConfig::write(&path, SECRET_CONFIG).unwrap();

    assert_eq!(config.path(), path);
}

#[cfg(unix)]
#[test]
fn reports_when_the_runtime_directory_is_not_writable() {
    use std::os::unix::fs::PermissionsExt;

    let directory = TemporaryPath::directory("unwritable-runtime-directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o500)).unwrap();
    let result = AtomicRuntimeConfig::write(directory.path().join("config.json"), SECRET_CONFIG);
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();

    assert!(matches!(
        result,
        Err(RuntimeConfigFileError::CreateTemporary { .. })
    ));
}
