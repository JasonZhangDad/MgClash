use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::id;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use magies_desktop_lib::session::{SessionCommandError, SessionDefaults, SessionService};
use magies_domain::ProxyProtocol;
use magies_platform::system_proxy::SystemProxyState;
use magies_profiles::{LocalHttpProfile, LocalSocksProfile};
use magies_session::{CoreSessionControl, DesktopSession, SystemProxySessionControl};
use magies_storage::MemorySecretStore;

const SHADOWSOCKS_LINK: &str = "ss://aes-128-gcm:runtime-secret@edge.example.com:8388#Tokyo%20Edge";

#[test]
fn reports_an_idle_status_before_a_node_is_imported() {
    let (service, _runtime) = service();
    let status = service.status();

    assert!(!status.connected);
    assert!(status.node.is_none());
    assert_eq!(status.core, "sing-box");
    assert_eq!(status.mode, "global");
    assert!(status.system_proxy);
    assert_eq!(status.socks_port, 10_808);
    assert_eq!(status.http_port, 10_809);
}

#[test]
fn importing_a_share_link_stores_the_credential_and_selects_the_node() {
    let (mut service, _runtime) = service();

    let status = service.import_node(SHADOWSOCKS_LINK).unwrap();

    let node = status.node.as_ref().unwrap();
    assert_eq!(node.name, "Tokyo Edge");
    assert_eq!(node.protocol, ProxyProtocol::Shadowsocks);
    assert_eq!(node.server, "edge.example.com");
    assert_eq!(node.port, 8388);
    assert!(!status.connected);
}

#[test]
fn importing_a_second_link_replaces_the_selected_node() {
    let (mut service, _runtime) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();

    let status = service
        .import_node("ss://aes-128-gcm:runtime-secret@edge.example.com:9000#Osaka")
        .unwrap();

    assert_eq!(status.node.as_ref().unwrap().name, "Osaka");
    assert_eq!(status.node.as_ref().unwrap().port, 9000);
}

#[test]
fn rejects_an_unsupported_share_link_without_selecting_a_node() {
    let (mut service, _runtime) = service();

    assert!(matches!(
        service.import_node("tuic://token@edge.example.com:443"),
        Err(SessionCommandError::ShareLink(_))
    ));
    assert!(service.status().node.is_none());
}

#[test]
fn connecting_starts_the_core_and_the_system_proxy() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut service, runtime) = service_with_events(&events);
    service.import_node(SHADOWSOCKS_LINK).unwrap();

    let status = service.connect().unwrap();

    assert!(status.connected);
    assert_eq!(status.node.as_ref().unwrap().name, "Tokyo Edge");
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["core_start", "proxy_enable"]
    );

    let status = service.disconnect().unwrap();

    assert!(!status.connected);
    assert!(status.node.is_some());
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["core_start", "proxy_enable", "proxy_stop", "core_stop"]
    );
    assert!(fs::read_dir(runtime.path()).unwrap().next().is_none());
}

#[test]
fn connecting_without_an_imported_node_fails_before_touching_the_core() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut service, _runtime) = service_with_events(&events);

    assert!(matches!(
        service.connect(),
        Err(SessionCommandError::NoSelectedNode)
    ));
    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn surfaces_a_failing_core_start_as_a_session_error() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let runtime = RuntimeDirectory::new("core-start-failure");
    let mut core = FakeCore::new(events.clone());
    core.fail_start = true;
    let mut service = SessionService::new(
        DesktopSession::new(
            MemorySecretStore::default(),
            core,
            FakeProxy::new(events.clone()),
            runtime.path(),
        ),
        SessionDefaults::v01(),
    );
    service.import_node(SHADOWSOCKS_LINK).unwrap();

    assert!(matches!(
        service.connect(),
        Err(SessionCommandError::Session(_))
    ));
    assert!(!service.status().connected);
    assert_eq!(events.lock().unwrap().as_slice(), ["core_start"]);
}

#[test]
fn disconnecting_an_idle_session_reports_a_session_error() {
    let (mut service, _runtime) = service();

    assert!(matches!(
        service.disconnect(),
        Err(SessionCommandError::Session(_))
    ));
}

fn service() -> (
    SessionService<MemorySecretStore, FakeCore, FakeProxy>,
    RuntimeDirectory,
) {
    service_with_events(&Arc::new(Mutex::new(Vec::new())))
}

fn service_with_events(
    events: &Arc<Mutex<Vec<&'static str>>>,
) -> (
    SessionService<MemorySecretStore, FakeCore, FakeProxy>,
    RuntimeDirectory,
) {
    let runtime = RuntimeDirectory::new("session-service");
    let service = SessionService::new(
        DesktopSession::new(
            MemorySecretStore::default(),
            FakeCore::new(events.clone()),
            FakeProxy::new(events.clone()),
            runtime.path(),
        ),
        SessionDefaults::v01(),
    );
    (service, runtime)
}

#[test]
fn every_session_failure_carries_a_stable_code_for_the_ui() {
    let (mut service, _runtime) = service();

    assert_eq!(service.connect().unwrap_err().code(), "no_selected_node");
    assert_eq!(
        service
            .import_node("tuic://token@edge.example.com")
            .unwrap_err()
            .code(),
        "invalid_share_link"
    );
    assert_eq!(service.disconnect().unwrap_err().code(), "session_failed");
}

#[test]
fn v01_defaults_use_the_documented_loopback_ports() {
    let defaults = SessionDefaults::v01();

    assert_eq!(defaults.socks, LocalSocksProfile::default());
    assert_eq!(defaults.http, LocalHttpProfile::default());
    assert!(defaults.system_proxy);
}

static RUNTIME_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct RuntimeDirectory(PathBuf);

impl RuntimeDirectory {
    fn new(name: &str) -> Self {
        let sequence = RUNTIME_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mgclash-desktop-{name}-{}-{sequence}", id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for RuntimeDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!(
                "failed to remove runtime directory {}: {error}",
                self.0.display()
            );
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FakeError(&'static str);

impl Display for FakeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for FakeError {}

struct FakeCore {
    events: Arc<Mutex<Vec<&'static str>>>,
    fail_start: bool,
}

impl FakeCore {
    fn new(events: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self {
            events,
            fail_start: false,
        }
    }
}

impl CoreSessionControl for FakeCore {
    type Error = FakeError;
    type Output = ();

    fn start(&mut self, config_path: &Path) -> Result<Self::Output, Self::Error> {
        self.events.lock().unwrap().push("core_start");
        assert!(
            fs::read_to_string(config_path)
                .unwrap()
                .contains("runtime-secret")
        );
        if self.fail_start {
            Err(FakeError("core start failed"))
        } else {
            Ok(())
        }
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push("core_stop");
        Ok(())
    }
}

struct FakeProxy {
    events: Arc<Mutex<Vec<&'static str>>>,
}

impl FakeProxy {
    fn new(events: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self { events }
    }
}

impl SystemProxySessionControl for FakeProxy {
    type Error = FakeError;

    fn enable(&mut self, _state: &SystemProxyState) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push("proxy_enable");
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push("proxy_stop");
        Ok(())
    }
}
