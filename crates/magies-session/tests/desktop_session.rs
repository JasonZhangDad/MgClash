use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use magies_domain::{CoreType, CredentialRef, ProxyProtocol};
use magies_platform::{OperatingSystem, system_proxy::SystemProxyState};
use magies_profiles::{
    CredentialCodec, DnsProfile, DnsServer, DnsStrategy, LocalHttpProfile, LocalSocksProfile,
    ShadowsocksParser, StoredNodeCredential, TunProfile,
};
use magies_routing::{RouteOutbound, RouteProfile, RoutingMode};
use magies_session::{
    CoreSessionControl, DesktopSession, DesktopSessionError, DesktopSessionProfile,
    SystemProxyMode, SystemProxySessionControl,
};
use magies_storage::{MemorySecretStore, SecretStore};
use uuid::Uuid;

#[test]
fn starts_core_before_system_proxy_and_restores_in_reverse_order() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let core = FakeCore::new(events.clone());
    let proxy = FakeProxy::new(events.clone());
    let store = MemorySecretStore::default();
    let profile = profile_with_stored_credential(&store).with_system_proxy(true);
    let runtime = RuntimeDirectory::new("happy-session");
    let mut session = DesktopSession::new(store, core, proxy, runtime.path());

    let output = session.start(&profile).unwrap();

    assert_eq!(output, "core-output");
    assert!(session.is_running());
    let config_path = session.config_path().unwrap().to_path_buf();
    assert!(
        fs::read_to_string(&config_path)
            .unwrap()
            .contains("runtime-secret")
    );
    session.stop().unwrap();
    assert!(!session.is_running());
    assert!(!config_path.exists());
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["core_start", "proxy_enable", "proxy_stop", "core_stop"]
    );
}

#[test]
fn missing_secret_stops_before_config_or_core_changes() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let runtime = RuntimeDirectory::new("missing-secret");
    let store = MemorySecretStore::default();
    let profile = profile_without_stored_credential();
    let mut session = DesktopSession::new(
        store,
        FakeCore::new(events.clone()),
        FakeProxy::new(events.clone()),
        runtime.path(),
    );

    assert!(matches!(
        session.start(&profile),
        Err(DesktopSessionError::Secret { .. })
    ));
    assert!(events.lock().unwrap().is_empty());
    assert!(fs::read_dir(runtime.path()).unwrap().next().is_none());
}

#[test]
fn core_start_failure_removes_runtime_config_without_touching_proxy() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut core = FakeCore::new(events.clone());
    core.fail_start = true;
    let store = MemorySecretStore::default();
    let profile = profile_with_stored_credential(&store).with_system_proxy(true);
    let runtime = RuntimeDirectory::new("core-start-failure");
    let mut session =
        DesktopSession::new(store, core, FakeProxy::new(events.clone()), runtime.path());

    assert!(matches!(
        session.start(&profile),
        Err(DesktopSessionError::CoreStart { .. })
    ));
    assert_eq!(events.lock().unwrap().as_slice(), ["core_start"]);
    assert!(fs::read_dir(runtime.path()).unwrap().next().is_none());
}

#[test]
fn proxy_enable_failure_stops_core_and_removes_runtime_config() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut proxy = FakeProxy::new(events.clone());
    proxy.fail_enable = true;
    let store = MemorySecretStore::default();
    let profile = profile_with_stored_credential(&store).with_system_proxy(true);
    let runtime = RuntimeDirectory::new("proxy-enable-failure");
    let mut session =
        DesktopSession::new(store, FakeCore::new(events.clone()), proxy, runtime.path());

    assert!(matches!(
        session.start(&profile),
        Err(DesktopSessionError::ProxyEnable { .. })
    ));
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["core_start", "proxy_enable", "core_stop"]
    );
    assert!(!session.is_running());
    assert!(fs::read_dir(runtime.path()).unwrap().next().is_none());
}

#[test]
fn failed_core_rollback_keeps_a_stoppable_active_session() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut core = FakeCore::new(events.clone());
    core.fail_stop_count = 1;
    let mut proxy = FakeProxy::new(events.clone());
    proxy.fail_enable = true;
    let store = MemorySecretStore::default();
    let profile = profile_with_stored_credential(&store).with_system_proxy(true);
    let runtime = RuntimeDirectory::new("rollback-failure");
    let mut session = DesktopSession::new(store, core, proxy, runtime.path());

    assert!(matches!(
        session.start(&profile),
        Err(DesktopSessionError::ProxyEnableAndCoreRollback { .. })
    ));
    assert!(session.is_running());
    session.stop().unwrap();
    assert!(!session.is_running());
    assert_eq!(
        events.lock().unwrap().as_slice(),
        [
            "core_start",
            "proxy_enable",
            "core_stop",
            "proxy_stop",
            "core_stop"
        ]
    );
}

#[test]
fn failed_proxy_restore_keeps_core_running_for_a_retry() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut proxy = FakeProxy::new(events.clone());
    proxy.fail_stop_count = 1;
    let store = MemorySecretStore::default();
    let profile = profile_with_stored_credential(&store).with_system_proxy(true);
    let runtime = RuntimeDirectory::new("proxy-restore-failure");
    let mut session =
        DesktopSession::new(store, FakeCore::new(events.clone()), proxy, runtime.path());
    session.start(&profile).unwrap();

    assert!(matches!(
        session.stop(),
        Err(DesktopSessionError::ProxyStop { .. })
    ));
    assert!(session.is_running());
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["core_start", "proxy_enable", "proxy_stop"]
    );
    session.stop().unwrap();
    assert!(!session.is_running());
}

#[test]
fn rejects_duplicate_start_and_conflicting_network_modes() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = MemorySecretStore::default();
    let profile = profile_with_stored_credential(&store);
    let runtime = RuntimeDirectory::new("invalid-session-state");
    let mut session = DesktopSession::new(
        store,
        FakeCore::new(events),
        FakeProxy::default(),
        runtime.path(),
    );
    session.start(&profile).unwrap();
    assert!(matches!(
        session.start(&profile),
        Err(DesktopSessionError::AlreadyRunning)
    ));
    session.stop().unwrap();

    let tun = TunProfile::new(OperatingSystem::Windows, false, 1_500, true, true).unwrap();
    let conflicting = profile.with_system_proxy(true).with_tun(tun, true);
    assert!(matches!(
        session.start(&conflicting),
        Err(DesktopSessionError::ConflictingNetworkModes)
    ));
}

#[test]
fn custom_local_ports_work_without_enabling_system_proxy() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = MemorySecretStore::default();
    let profile = profile_with_stored_credential(&store).with_local_proxies(
        LocalSocksProfile::new(12_080).unwrap(),
        LocalHttpProfile::new(12_081).unwrap(),
    );
    let runtime = RuntimeDirectory::new("custom-local-ports");
    let mut session = DesktopSession::new(
        store,
        FakeCore::new(events.clone()),
        FakeProxy::new(events.clone()),
        runtime.path(),
    );

    session.start(&profile).unwrap();
    let config = fs::read_to_string(session.config_path().unwrap()).unwrap();
    assert!(config.contains("12080"));
    assert!(config.contains("12081"));
    session.stop().unwrap();
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["core_start", "core_stop"]
    );
}

#[test]
fn the_core_choice_decides_which_document_is_written() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = MemorySecretStore::default();
    let profile = profile_with_stored_credential(&store).with_core(CoreType::Xray);
    let runtime = RuntimeDirectory::new("xray-core-choice");
    let mut session = DesktopSession::new(
        store,
        FakeCore::new(events.clone()),
        FakeProxy::new(events.clone()),
        runtime.path(),
    );

    session.start(&profile).unwrap();

    let config = fs::read_to_string(session.config_path().unwrap()).unwrap();
    // Xray names its sections differently, so these are unambiguous markers.
    assert!(
        config.contains("\"routing\""),
        "expected an Xray document: {config}"
    );
    assert!(config.contains("freedom"));
    assert!(!config.contains("\"route\""));
    session.stop().unwrap();
}

#[test]
fn sing_box_remains_the_default_core() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = MemorySecretStore::default();
    let profile = profile_with_stored_credential(&store);
    let runtime = RuntimeDirectory::new("default-core");
    let mut session = DesktopSession::new(
        store,
        FakeCore::new(events.clone()),
        FakeProxy::new(events.clone()),
        runtime.path(),
    );

    session.start(&profile).unwrap();

    let config = fs::read_to_string(session.config_path().unwrap()).unwrap();
    assert!(
        config.contains("\"route\""),
        "expected a sing-box document: {config}"
    );
    assert!(!config.contains("freedom"));
    session.stop().unwrap();
}

#[test]
fn xray_refuses_a_tun_session_instead_of_dropping_the_setting() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = MemorySecretStore::default();
    let profile = profile_with_stored_credential(&store)
        .with_core(CoreType::Xray)
        .with_system_proxy(false)
        .with_tun(
            TunProfile::new(OperatingSystem::Windows, false, 1_500, true, true).unwrap(),
            false,
        );
    let runtime = RuntimeDirectory::new("xray-tun-refused");
    let mut session = DesktopSession::new(
        store,
        FakeCore::new(events.clone()),
        FakeProxy::new(events.clone()),
        runtime.path(),
    );

    let error = session.start(&profile).unwrap_err();

    assert_eq!(
        error.to_string(),
        "Xray cannot provide TUN mode",
        "a dropped TUN setting would look like a working session"
    );
    // Nothing started, so nothing has to be rolled back.
    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn failed_core_stop_retains_the_session_for_a_retry() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut core = FakeCore::new(events.clone());
    core.fail_stop_count = 1;
    let store = MemorySecretStore::default();
    let profile = profile_with_stored_credential(&store);
    let runtime = RuntimeDirectory::new("core-stop-retry");
    let mut session = DesktopSession::new(store, core, FakeProxy::default(), runtime.path());
    session.start(&profile).unwrap();

    assert!(matches!(
        session.stop(),
        Err(DesktopSessionError::CoreStop { .. })
    ));
    assert!(session.is_running());
    session.stop().unwrap();
    assert!(!session.is_running());
}

fn profile_with_stored_credential(store: &MemorySecretStore) -> DesktopSessionProfile {
    let parsed = ShadowsocksParser
        .parse("ss://aes-128-gcm:runtime-secret@edge.example.com:443")
        .unwrap();
    let credential_ref = CredentialRef::new("secret://nodes/session-test").unwrap();
    store
        .put(
            &credential_ref,
            &CredentialCodec::encode(&StoredNodeCredential::from(parsed.credential())).unwrap(),
        )
        .unwrap();
    let node = parsed.into_proxy_node(Uuid::nil(), credential_ref).unwrap();
    DesktopSessionProfile::new(node, system_dns(), global_route())
}

fn profile_without_stored_credential() -> DesktopSessionProfile {
    let parsed = ShadowsocksParser
        .parse("ss://aes-128-gcm:missing-secret@edge.example.com:443")
        .unwrap();
    let node = parsed
        .into_proxy_node(
            Uuid::nil(),
            CredentialRef::new("secret://nodes/missing").unwrap(),
        )
        .unwrap();
    DesktopSessionProfile::new(node, system_dns(), global_route())
}

#[test]
fn writes_a_custom_document_verbatim_without_generators() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = MemorySecretStore::default();
    let document = r#"{"inbounds":[],"outbounds":[{"type":"direct","tag":"direct"}],"marker":"custom-verbatim"}"#;
    let credential_ref = CredentialRef::new("secret://nodes/custom").unwrap();
    store
        .put(
            &credential_ref,
            &CredentialCodec::encode(&StoredNodeCredential::Custom(
                magies_profiles::CustomCredential {
                    core: CoreType::SingBox,
                    document: document.to_owned(),
                },
            ))
            .unwrap(),
        )
        .unwrap();
    let node = magies_domain::ProxyNode::new(
        Uuid::nil(),
        "Custom",
        ProxyProtocol::Custom,
        "127.0.0.1",
        443,
        Some(credential_ref),
    )
    .unwrap();
    let profile =
        DesktopSessionProfile::new(node, system_dns(), global_route()).with_core(CoreType::SingBox);
    let runtime = RuntimeDirectory::new("custom-session");
    let mut session = DesktopSession::new(
        store,
        FakeCoreForCustom::new(events.clone()),
        FakeProxy::new(events.clone()),
        runtime.path(),
    );

    session.start(&profile).unwrap();
    let config = fs::read_to_string(session.config_path().unwrap()).unwrap();
    assert!(config.contains("custom-verbatim"));
    assert!(!config.contains("runtime-secret"));
    session.stop().unwrap();
}

#[test]
fn rejects_custom_core_mismatch() {
    let store = MemorySecretStore::default();
    let credential_ref = CredentialRef::new("secret://nodes/custom-xray").unwrap();
    store
        .put(
            &credential_ref,
            &CredentialCodec::encode(&StoredNodeCredential::Custom(
                magies_profiles::CustomCredential {
                    core: CoreType::Xray,
                    document: r#"{"outbounds":[]}"#.to_owned(),
                },
            ))
            .unwrap(),
        )
        .unwrap();
    let node = magies_domain::ProxyNode::new(
        Uuid::nil(),
        "Custom",
        ProxyProtocol::Custom,
        "127.0.0.1",
        443,
        Some(credential_ref),
    )
    .unwrap();
    let profile =
        DesktopSessionProfile::new(node, system_dns(), global_route()).with_core(CoreType::SingBox);
    let runtime = RuntimeDirectory::new("custom-mismatch");
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut session = DesktopSession::new(
        store,
        FakeCore::new(events.clone()),
        FakeProxy::new(events.clone()),
        runtime.path(),
    );

    assert!(matches!(
        session.start(&profile),
        Err(DesktopSessionError::CustomCoreMismatch { .. })
    ));
    assert!(events.lock().unwrap().is_empty());
}

fn system_dns() -> DnsProfile {
    DnsProfile::new(
        vec![DnsServer::system("system").unwrap()],
        Vec::new(),
        "system",
        DnsStrategy::PreferIpv4,
        false,
        false,
    )
    .unwrap()
}

fn global_route() -> RouteProfile {
    RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap()
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
    fail_stop_count: usize,
}

impl FakeCore {
    fn new(events: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self {
            events,
            fail_start: false,
            fail_stop_count: 0,
        }
    }
}

impl CoreSessionControl for FakeCore {
    type Error = FakeError;
    type Output = &'static str;

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
            Ok("core-output")
        }
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push("core_stop");
        if self.fail_stop_count > 0 {
            self.fail_stop_count -= 1;
            Err(FakeError("core stop failed"))
        } else {
            Ok(())
        }
    }
}

/// Like [`FakeCore`], but does not assert Shadowsocks-specific config content.
struct FakeCoreForCustom {
    events: Arc<Mutex<Vec<&'static str>>>,
}

impl FakeCoreForCustom {
    fn new(events: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self { events }
    }
}

impl CoreSessionControl for FakeCoreForCustom {
    type Error = FakeError;
    type Output = &'static str;

    fn start(&mut self, config_path: &Path) -> Result<Self::Output, Self::Error> {
        self.events.lock().unwrap().push("core_start");
        assert!(config_path.exists());
        Ok("core-output")
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push("core_stop");
        Ok(())
    }
}

#[derive(Default)]
struct FakeProxy {
    events: Arc<Mutex<Vec<&'static str>>>,
    fail_enable: bool,
    fail_stop_count: usize,
}

impl FakeProxy {
    fn new(events: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self {
            events,
            fail_enable: false,
            fail_stop_count: 0,
        }
    }
}

impl SystemProxySessionControl for FakeProxy {
    type Error = FakeError;

    fn enable(&mut self, state: &SystemProxyState) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push("proxy_enable");
        // A managed state points at the session's local proxies; a cleared one
        // carries no endpoint at all.
        if let Some(socks) = state.socks().endpoint() {
            assert_eq!(state.http().endpoint().unwrap().port(), 10_809);
            assert_eq!(state.https().endpoint().unwrap().port(), 10_809);
            assert_eq!(socks.port(), 10_808);
        } else if state.pac().enabled() {
            // A PAC state names the script and nothing else.
            assert!(!state.http().enabled());
            assert!(state.pac().url().is_some());
        } else {
            assert!(!state.http().enabled());
            assert!(!state.https().enabled());
            assert!(!state.pac().enabled());
        }
        if self.fail_enable {
            Err(FakeError("proxy enable failed"))
        } else {
            Ok(())
        }
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push("proxy_stop");
        if self.fail_stop_count > 0 {
            self.fail_stop_count -= 1;
            Err(FakeError("proxy stop failed"))
        } else {
            Ok(())
        }
    }
}

struct RuntimeDirectory(PathBuf);

impl RuntimeDirectory {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("magies-session-{}-{name}", std::process::id()));
        fs::create_dir(&path).unwrap();
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
                "failed to remove session test directory {}: {error}",
                self.0.display()
            );
        }
    }
}

#[test]
fn profile_protocol_is_the_expected_test_protocol() {
    assert_eq!(
        profile_without_stored_credential().node().protocol_type,
        ProxyProtocol::Shadowsocks
    );
}

#[test]
fn the_cleared_mode_writes_a_disabled_state_and_restores_it_on_stop() {
    let store = MemorySecretStore::default();
    let profile =
        profile_with_stored_credential(&store).with_system_proxy_mode(SystemProxyMode::Cleared);
    let events = Arc::new(Mutex::new(Vec::new()));
    let runtime = RuntimeDirectory::new("proxy-cleared");
    let mut session = DesktopSession::new(
        store,
        FakeCore::new(events.clone()),
        FakeProxy::new(events.clone()),
        runtime.path(),
    );

    session.start(&profile).unwrap();
    session.stop().unwrap();

    // Clearing goes through the same snapshot path as managing, so the user's
    // own proxy comes back when the session ends.
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["core_start", "proxy_enable", "proxy_stop", "core_stop"]
    );
}

#[test]
fn the_unchanged_mode_never_touches_the_host() {
    let store = MemorySecretStore::default();
    let profile =
        profile_with_stored_credential(&store).with_system_proxy_mode(SystemProxyMode::Unchanged);
    let events = Arc::new(Mutex::new(Vec::new()));
    let runtime = RuntimeDirectory::new("proxy-unchanged");
    let mut session = DesktopSession::new(
        store,
        FakeCore::new(events.clone()),
        FakeProxy::new(events.clone()),
        runtime.path(),
    );

    session.start(&profile).unwrap();
    session.stop().unwrap();

    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["core_start", "core_stop"]
    );
}

#[test]
fn clearing_the_host_proxy_is_compatible_with_tun() {
    let store = MemorySecretStore::default();
    let tun = TunProfile::new(OperatingSystem::Windows, false, 1_500, true, true).unwrap();
    let profile = profile_with_stored_credential(&store)
        .with_system_proxy_mode(SystemProxyMode::Cleared)
        .with_tun(tun, true);
    let events = Arc::new(Mutex::new(Vec::new()));
    let runtime = RuntimeDirectory::new("proxy-cleared-tun");
    let mut session = DesktopSession::new(
        store,
        FakeCore::new(events.clone()),
        FakeProxy::new(events.clone()),
        runtime.path(),
    );

    // Only the managed mode conflicts: clearing the host proxy while TUN carries
    // the traffic is what a TUN user wants, not a contradiction.
    assert!(session.start(&profile).is_ok());
}

#[test]
fn the_pac_mode_names_the_script_url_and_no_fixed_endpoint() {
    let store = MemorySecretStore::default();
    let profile = profile_with_stored_credential(&store).with_system_proxy_mode(
        SystemProxyMode::Pac("http://127.0.0.1:9/proxy.pac".to_owned()),
    );
    let events = Arc::new(Mutex::new(Vec::new()));
    let runtime = RuntimeDirectory::new("proxy-pac");
    let mut session = DesktopSession::new(
        store,
        FakeCore::new(events.clone()),
        FakeProxy::new(events.clone()),
        runtime.path(),
    );

    session.start(&profile).unwrap();
    session.stop().unwrap();

    // A host given both a PAC file and a fixed proxy applies them in an order
    // that differs per platform, so only one of them is ever named.
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["core_start", "proxy_enable", "proxy_stop", "core_stop"]
    );
}

#[test]
fn an_empty_pac_url_is_refused_before_the_host_is_touched() {
    let store = MemorySecretStore::default();
    let profile = profile_with_stored_credential(&store)
        .with_system_proxy_mode(SystemProxyMode::Pac("   ".to_owned()));
    let events = Arc::new(Mutex::new(Vec::new()));
    let runtime = RuntimeDirectory::new("proxy-pac-empty");
    let mut session = DesktopSession::new(
        store,
        FakeCore::new(events.clone()),
        FakeProxy::new(events.clone()),
        runtime.path(),
    );

    let error = session.start(&profile).unwrap_err();

    assert!(matches!(error, DesktopSessionError::InvalidPacUrl { .. }));
    // The Core started before the proxy step, so it has to be left stoppable
    // rather than the session pretending nothing happened.
    assert!(events.lock().unwrap().contains(&"core_start"));
}
