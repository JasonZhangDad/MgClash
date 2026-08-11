use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use magies_domain::CredentialRef;
use magies_platform::system_proxy::SystemProxyState;
use magies_profiles::{
    CredentialCodec, DnsProfile, DnsServer, DnsStrategy, ShadowsocksParser, StoredNodeCredential,
};
use magies_routing::{RouteOutbound, RouteProfile, RoutingMode};
use magies_session::{
    CoreSessionControl, DesktopSession, DesktopSessionProfile, MAX_RECOVERY_ATTEMPTS, NetworkEvent,
    NetworkRecoveryPolicy, RecoveryError, RecoveryOutcome, SessionHealthProbe,
    SystemProxySessionControl,
};
use magies_storage::{MemorySecretStore, SecretStore};
use uuid::Uuid;

const DEBOUNCE: Duration = Duration::from_millis(800);

#[test]
fn coalesces_a_burst_of_events_into_one_debounced_action() {
    let start = Instant::now();
    let mut policy = NetworkRecoveryPolicy::new(DEBOUNCE);

    assert_eq!(policy.due_at(), None);

    policy.observe(NetworkEvent::PathChanged, start);
    assert_eq!(policy.due_at(), Some(start + DEBOUNCE));

    // A second event inside the window pushes the deadline out instead of
    // queuing another recovery.
    policy.observe(
        NetworkEvent::PathChanged,
        start + Duration::from_millis(300),
    );
    assert_eq!(
        policy.due_at(),
        Some(start + Duration::from_millis(300) + DEBOUNCE)
    );
}

#[test]
fn stays_idle_until_the_debounce_window_elapses() {
    let start = Instant::now();
    let (mut session, _runtime, _fail_start) = session();
    let probe = FakeProbe::healthy();
    let mut policy = NetworkRecoveryPolicy::new(DEBOUNCE);
    policy.observe(NetworkEvent::PathChanged, start);

    assert_eq!(
        policy
            .recover(start + Duration::from_millis(799), &mut session, &probe)
            .unwrap(),
        RecoveryOutcome::Idle
    );
    assert_eq!(probe.checks(), 0);
}

#[test]
fn a_healthy_core_is_never_restarted_after_a_path_change() {
    let start = Instant::now();
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut session, _runtime, _fail_start) = session_with_events(&events);
    session.start(&profile(session.secret_store())).unwrap();
    events.lock().unwrap().clear();

    let probe = FakeProbe::healthy();
    let mut policy = NetworkRecoveryPolicy::new(DEBOUNCE);
    policy.observe(NetworkEvent::PathChanged, start);

    assert_eq!(
        policy
            .recover(start + DEBOUNCE, &mut session, &probe)
            .unwrap(),
        RecoveryOutcome::Healthy
    );
    assert_eq!(probe.checks(), 1);
    assert!(events.lock().unwrap().is_empty());
    assert!(session.is_running());
    assert_eq!(policy.due_at(), None);
}

#[test]
fn periodic_monitor_leaves_a_healthy_core_alone_without_a_network_event() {
    let start = Instant::now();
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut session, _runtime, _fail_start) = session_with_events(&events);
    session.start(&profile(session.secret_store())).unwrap();
    events.lock().unwrap().clear();

    let probe = FakeProbe::healthy();
    let mut policy = NetworkRecoveryPolicy::new(DEBOUNCE);

    assert_eq!(
        policy.monitor(start, &mut session, &probe).unwrap(),
        RecoveryOutcome::Idle
    );
    assert_eq!(probe.checks(), 1);
    assert!(events.lock().unwrap().is_empty());
    assert!(session.is_running());
}

#[test]
fn periodic_monitor_recovers_an_unhealthy_core_without_a_network_event() {
    let start = Instant::now();
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut session, _runtime, _fail_start) = session_with_events(&events);
    session.start(&profile(session.secret_store())).unwrap();
    events.lock().unwrap().clear();

    let probe = FakeProbe::unhealthy();
    let mut policy = NetworkRecoveryPolicy::new(DEBOUNCE);

    assert_eq!(
        policy.monitor(start, &mut session, &probe).unwrap(),
        RecoveryOutcome::Reconnected { attempts: 1 }
    );
    assert_eq!(probe.checks(), 2);
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["proxy_stop", "core_stop", "core_start", "proxy_enable"]
    );
    assert!(session.is_running());
}

#[test]
fn periodic_monitor_never_resurrects_a_user_stopped_session() {
    let start = Instant::now();
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut session, _runtime, _fail_start) = session_with_events(&events);
    session.start(&profile(session.secret_store())).unwrap();
    session.stop().unwrap();
    events.lock().unwrap().clear();

    let probe = FakeProbe::unhealthy();
    let mut policy = NetworkRecoveryPolicy::new(DEBOUNCE);

    assert_eq!(
        policy.monitor(start, &mut session, &probe).unwrap(),
        RecoveryOutcome::Idle
    );
    assert_eq!(probe.checks(), 0);
    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn an_unhealthy_core_is_restarted_in_the_prd_order() {
    let start = Instant::now();
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut session, _runtime, _fail_start) = session_with_events(&events);
    session.start(&profile(session.secret_store())).unwrap();
    events.lock().unwrap().clear();

    let probe = FakeProbe::unhealthy();
    let mut policy = NetworkRecoveryPolicy::new(DEBOUNCE);
    policy.observe(NetworkEvent::Woke, start);

    assert_eq!(
        policy
            .recover(start + DEBOUNCE, &mut session, &probe)
            .unwrap(),
        RecoveryOutcome::Reconnected { attempts: 1 }
    );
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["proxy_stop", "core_stop", "core_start", "proxy_enable"]
    );
    assert!(session.is_running());
}

#[test]
fn recovering_without_a_running_session_does_nothing() {
    let start = Instant::now();
    let (mut session, _runtime, _fail_start) = session();
    let probe = FakeProbe::unhealthy();
    let mut policy = NetworkRecoveryPolicy::new(DEBOUNCE);
    policy.observe(NetworkEvent::PathChanged, start);

    assert_eq!(
        policy
            .recover(start + DEBOUNCE, &mut session, &probe)
            .unwrap(),
        RecoveryOutcome::Idle
    );
    assert_eq!(probe.checks(), 0);
    assert_eq!(policy.due_at(), None);
}

#[test]
fn a_repeatedly_failing_restart_stops_at_the_attempt_budget() {
    let start = Instant::now();
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut session, _runtime, fail_start) = session_with_events(&events);
    session.start(&profile(session.secret_store())).unwrap();

    fail_start.store(true, Ordering::Relaxed);
    let probe = FakeProbe::unhealthy();
    let mut policy = NetworkRecoveryPolicy::new(DEBOUNCE);
    policy.observe(NetworkEvent::PathChanged, start);

    assert!(matches!(
        policy.recover(start + DEBOUNCE, &mut session, &probe),
        Err(RecoveryError::AttemptsExhausted {
            attempts: MAX_RECOVERY_ATTEMPTS,
            ..
        })
    ));
    assert!(!session.is_running());
    assert_eq!(policy.due_at(), None);
}

#[test]
fn a_later_event_can_still_recover_after_an_exhausted_burst() {
    let start = Instant::now();
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut session, _runtime, fail_start) = session_with_events(&events);
    session.start(&profile(session.secret_store())).unwrap();
    fail_start.store(true, Ordering::Relaxed);

    let probe = FakeProbe::unhealthy();
    let mut policy = NetworkRecoveryPolicy::new(DEBOUNCE);
    policy.observe(NetworkEvent::PathChanged, start);
    policy
        .recover(start + DEBOUNCE, &mut session, &probe)
        .unwrap_err();

    fail_start.store(false, Ordering::Relaxed);
    let later = start + Duration::from_secs(60);
    policy.observe(NetworkEvent::Woke, later);

    assert_eq!(
        policy
            .recover(later + DEBOUNCE, &mut session, &probe)
            .unwrap(),
        RecoveryOutcome::Reconnected { attempts: 1 }
    );
}

type TestSession = DesktopSession<MemorySecretStore, FakeCore, FakeProxy>;

#[test]
fn a_user_requested_disconnect_is_never_undone_by_recovery() {
    let start = Instant::now();
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut session, _runtime, _fail_start) = session_with_events(&events);
    session.start(&profile(session.secret_store())).unwrap();
    session.stop().unwrap();
    events.lock().unwrap().clear();

    let probe = FakeProbe::unhealthy();
    let mut policy = NetworkRecoveryPolicy::new(DEBOUNCE);
    policy.observe(NetworkEvent::Woke, start);

    assert_eq!(
        policy
            .recover(start + DEBOUNCE, &mut session, &probe)
            .unwrap(),
        RecoveryOutcome::Idle
    );
    assert!(!session.is_running());
    assert!(events.lock().unwrap().is_empty());
}

fn session() -> (TestSession, RuntimeDirectory, Arc<AtomicBool>) {
    session_with_events(&Arc::new(Mutex::new(Vec::new())))
}

fn session_with_events(
    events: &Arc<Mutex<Vec<&'static str>>>,
) -> (TestSession, RuntimeDirectory, Arc<AtomicBool>) {
    let runtime = RuntimeDirectory::new("network-recovery");
    let fail_start = Arc::new(AtomicBool::new(false));
    let session = DesktopSession::new(
        MemorySecretStore::default(),
        FakeCore::new(events.clone(), fail_start.clone()),
        FakeProxy::new(events.clone()),
        runtime.path(),
    );
    (session, runtime, fail_start)
}

fn profile(store: &MemorySecretStore) -> DesktopSessionProfile {
    let parsed = ShadowsocksParser
        .parse("ss://aes-128-gcm:runtime-secret@edge.example.com:443")
        .unwrap();
    let credential_ref = CredentialRef::new("secret://nodes/recovery-test").unwrap();
    store
        .put(
            &credential_ref,
            &CredentialCodec::encode(&StoredNodeCredential::from(parsed.credential())).unwrap(),
        )
        .unwrap();
    let node = parsed.into_proxy_node(Uuid::nil(), credential_ref).unwrap();
    let dns = DnsProfile::new(
        vec![DnsServer::system("system").unwrap()],
        Vec::new(),
        "system",
        DnsStrategy::PreferIpv4,
        false,
        false,
    )
    .unwrap();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    DesktopSessionProfile::new(node, dns, route).with_system_proxy(true)
}

struct FakeProbe {
    healthy: bool,
    checks: Mutex<usize>,
}

impl FakeProbe {
    fn healthy() -> Self {
        Self {
            healthy: true,
            checks: Mutex::new(0),
        }
    }

    fn unhealthy() -> Self {
        Self {
            healthy: false,
            checks: Mutex::new(0),
        }
    }

    fn checks(&self) -> usize {
        *self.checks.lock().unwrap()
    }
}

impl SessionHealthProbe for FakeProbe {
    fn is_healthy(&self) -> bool {
        *self.checks.lock().unwrap() += 1;
        self.healthy
    }
}

static RUNTIME_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct RuntimeDirectory(PathBuf);

impl RuntimeDirectory {
    fn new(name: &str) -> Self {
        let sequence = RUNTIME_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "magies-session-{name}-{}-{sequence}",
            std::process::id()
        ));
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
    fail_start: Arc<AtomicBool>,
}

impl FakeCore {
    fn new(events: Arc<Mutex<Vec<&'static str>>>, fail_start: Arc<AtomicBool>) -> Self {
        Self { events, fail_start }
    }
}

impl CoreSessionControl for FakeCore {
    type Error = FakeError;
    type Output = ();

    fn start(&mut self, _config_path: &Path) -> Result<Self::Output, Self::Error> {
        self.events.lock().unwrap().push("core_start");
        if self.fail_start.load(Ordering::Relaxed) {
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
