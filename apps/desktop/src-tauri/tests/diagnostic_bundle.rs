use std::fs;
use std::path::{Path, PathBuf};
use std::process::id;
use std::sync::atomic::{AtomicUsize, Ordering};

use magies_desktop_lib::diagnostics::{DiagnosticBundle, DiagnosticBundleError};
use magies_desktop_lib::dns_settings::DnsSettings;
use magies_desktop_lib::route_settings::RouteSettings;
use magies_desktop_lib::session::{NodeSummary, SessionStatus};
use magies_domain::ProxyProtocol;
use magies_profiles::REDACTED;
use serde_json::Value;
use uuid::Uuid;

fn status() -> SessionStatus {
    SessionStatus {
        connected: true,
        node: Some(NodeSummary {
            deletable: true,
            id: Uuid::nil(),
            name: "Tokyo Edge".to_owned(),
            protocol: ProxyProtocol::Shadowsocks,
            server: "edge.example.com".to_owned(),
            port: 8388,
            latency_ms: None,
            last_tested_at: None,
        }),
        core: "sing-box",
        dns: DnsSettings::default(),
        mode: "global",
        route: RouteSettings::default(),
        system_proxy: true,
        socks_port: 10_808,
        http_port: 10_809,
    }
}

#[test]
fn describes_the_platform_and_the_session_without_a_config() {
    let bundle = DiagnosticBundle::collect(&status(), None);
    let report = bundle.to_json();

    assert_eq!(report["session"]["connected"], true);
    assert_eq!(report["session"]["core"], "sing-box");
    assert_eq!(report["session"]["node"]["name"], "Tokyo Edge");
    assert_eq!(report["session"]["node"]["server"], "edge.example.com");
    assert!(report["platform"]["artifactIdentifier"].is_string());
    assert!(report["platform"]["tunAvailability"].is_string());
    assert_eq!(report["runtimeConfig"], Value::Null);
}

#[test]
fn includes_the_runtime_config_with_every_credential_masked() {
    let config = Fixture::new(
        "with-credential",
        br#"{"outbounds":[{"type":"shadowsocks","server":"edge.example.com",
             "method":"aes-128-gcm","password":"runtime-secret"}]}"#,
    );

    let bundle = DiagnosticBundle::collect(&status(), Some(config.path()));
    let report = bundle.to_json();

    let outbound = &report["runtimeConfig"]["outbounds"][0];
    assert_eq!(outbound["password"], REDACTED);
    assert_eq!(outbound["server"], "edge.example.com");
    assert_eq!(outbound["method"], "aes-128-gcm");

    let text = serde_json::to_string(report).unwrap();
    assert!(
        !text.contains("runtime-secret"),
        "credential leaked: {text}"
    );
}

#[test]
fn an_unreadable_config_is_reported_instead_of_failing_the_export() {
    let bundle = DiagnosticBundle::collect(&status(), Some(Path::new("/no/such/session.json")));
    let report = bundle.to_json();

    assert_eq!(report["runtimeConfig"], Value::Null);
    assert!(
        report["runtimeConfigError"]
            .as_str()
            .expect("the reason must be reported")
            .contains("session.json")
    );
}

#[test]
fn a_malformed_config_is_reported_rather_than_copied_verbatim() {
    let config = Fixture::new("malformed", b"{not json, may hold secrets");

    let bundle = DiagnosticBundle::collect(&status(), Some(config.path()));
    let report = bundle.to_json();

    assert_eq!(report["runtimeConfig"], Value::Null);
    assert!(report["runtimeConfigError"].is_string());
    let text = serde_json::to_string(report).unwrap();
    assert!(
        !text.contains("may hold secrets"),
        "unparsed config content must never be copied into the bundle: {text}"
    );
}

#[test]
fn writes_a_readable_bundle_to_disk() {
    let directory = Directory::new("export");
    let bundle = DiagnosticBundle::collect(&status(), None);

    let path = bundle.write_to(directory.path()).unwrap();

    assert!(path.starts_with(directory.path()));
    assert_eq!(
        path.extension().and_then(|value| value.to_str()),
        Some("json")
    );
    let written: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(written["session"]["core"], "sing-box");
}

#[test]
fn a_missing_export_directory_is_a_typed_error() {
    let bundle = DiagnosticBundle::collect(&status(), None);

    assert!(matches!(
        bundle.write_to(Path::new("/no/such/directory")),
        Err(DiagnosticBundleError::Write { .. })
    ));
}

static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(name: &str, contents: &[u8]) -> Self {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mgclash-diag-{name}-{}-{sequence}", id()));
        fs::write(&path, contents).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.0) {
            eprintln!("failed to remove fixture {}: {error}", self.0.display());
        }
    }
}

struct Directory(PathBuf);

impl Directory {
    fn new(name: &str) -> Self {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mgclash-diag-{name}-{}-{sequence}", id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("failed to remove directory {}: {error}", self.0.display());
        }
    }
}
