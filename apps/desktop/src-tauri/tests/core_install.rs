use std::fs;
use std::path::PathBuf;
use std::process::id;
use std::sync::atomic::{AtomicUsize, Ordering};

use magies_desktop_lib::core_install::{
    CoreInstallError, CoreInstallStore, CoreKind, InstalledCoreEntry,
};

static DIRECTORY_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

fn store_directory(name: &str) -> PathBuf {
    let sequence = DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("mgclash-core-install-{name}-{}-{sequence}", id()))
}

fn entry(version: &str, binary: &str) -> InstalledCoreEntry {
    InstalledCoreEntry {
        version: version.to_owned(),
        sha256: "a".repeat(64),
        binary: binary.to_owned(),
        previous_version: None,
    }
}

#[test]
fn parses_the_core_names_the_window_sends() {
    assert_eq!(CoreKind::parse("sing-box").unwrap(), CoreKind::SingBox);
    assert_eq!(CoreKind::parse("singBox").unwrap(), CoreKind::SingBox);
    assert_eq!(CoreKind::parse("xray").unwrap(), CoreKind::Xray);
    assert_eq!(CoreKind::parse("Xray").unwrap(), CoreKind::Xray);
    assert!(matches!(
        CoreKind::parse("clash"),
        Err(CoreInstallError::UnknownCore { name }) if name == "clash"
    ));
}

#[test]
fn opens_the_install_directory_by_creating_it() {
    let directory = store_directory("creates");
    assert!(!directory.is_dir());

    let store = CoreInstallStore::open(&directory).unwrap();

    assert!(directory.is_dir());
    assert_eq!(store.directory(), directory);
    // Opening an existing directory is not an error: it happens on every launch.
    assert!(CoreInstallStore::open(&directory).is_ok());
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn reports_no_installed_core_before_anything_is_downloaded() {
    let directory = store_directory("empty");
    let store = CoreInstallStore::open(&directory).unwrap();

    assert_eq!(store.load_manifest().unwrap(), None);
    let status = store.status().unwrap();
    assert_eq!(status.directory, directory.display().to_string());
    assert_eq!(status.sing_box, None);
    assert_eq!(status.xray, None);
    assert!(store.sing_box_settings().is_none());
    assert!(store.xray_settings().is_none());
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn reads_back_a_manifest_written_by_an_install() {
    let directory = store_directory("manifest");
    let store = CoreInstallStore::open(&directory).unwrap();
    let binary = directory.join("sing-box");
    fs::write(&binary, b"binary").unwrap();
    fs::write(
        directory.join("manifest.json"),
        serde_json::to_string(&serde_json::json!({
            "singBox": {
                "version": "1.13.18",
                "sha256": "a".repeat(64),
                "binary": binary.display().to_string(),
                "previousVersion": "1.13.17"
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let manifest = store.load_manifest().unwrap().unwrap();
    let sing_box = manifest.sing_box.unwrap();
    assert_eq!(sing_box.version, "1.13.18");
    assert_eq!(sing_box.previous_version.as_deref(), Some("1.13.17"));
    assert!(manifest.xray.is_none());

    let status = store.status().unwrap();
    assert_eq!(status.sing_box.unwrap().version, "1.13.18");
    // The recorded binary and digest become the Core settings the session runs.
    assert!(store.sing_box_settings().is_some());
    assert!(store.xray_settings().is_none());
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn a_corrupt_manifest_is_a_typed_error_rather_than_a_silent_reset() {
    let directory = store_directory("corrupt");
    let store = CoreInstallStore::open(&directory).unwrap();
    fs::write(directory.join("manifest.json"), b"{ not json").unwrap();

    assert!(matches!(
        store.load_manifest(),
        Err(CoreInstallError::ParseManifest { .. })
    ));
    assert!(store.status().is_err());
    // A manifest that cannot be parsed must not look like a usable install.
    assert!(store.sing_box_settings().is_none());
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn an_entry_with_an_invalid_digest_reports_why_rather_than_running_it() {
    let directory = store_directory("digest");
    let store = CoreInstallStore::open(&directory).unwrap();
    let mut broken = entry("1.13.18", &directory.join("sing-box").display().to_string());
    broken.sha256 = "not-a-digest".to_owned();
    fs::write(
        directory.join("manifest.json"),
        serde_json::to_string(&serde_json::json!({ "singBox": broken })).unwrap(),
    )
    .unwrap();

    let settings = store.sing_box_settings().expect("the entry is recorded");
    assert!(settings.is_err());
    fs::remove_dir_all(&directory).unwrap();
}
