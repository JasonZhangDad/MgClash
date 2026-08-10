//! G06 stability soak.
//!
//! Runs a real Core process through [`DesktopSession`] for a configurable
//! duration, probing health continuously and forcing periodic recovery cycles.
//! What it is looking for is drift that only appears over time: runtime configs
//! piling up in the session directory, a reconnect that stops working after N
//! cycles, or health that degrades without anything reporting it.
//!
//! The default Core is the `soak_core` fixture, which binds the loopback ports
//! the generated config declares. That keeps the harness self-contained — no
//! pinned binary, no reachable proxy server — while still exercising the real
//! process lifecycle, the real TCP health check, and the real recovery policy.
//! Point `MAGIES_SOAK_CORE_BIN` at an official sing-box to soak against it
//! instead; the generated config is the same one the app writes.
//!
//! ```sh
//! # quick check that the harness itself works
//! cargo test -p magies-session --test soak -- --ignored --nocapture
//!
//! # the PRD's 72 hours
//! MAGIES_SOAK_DURATION_SECS=259200 \
//!   cargo test -p magies-session --test soak -- --ignored --nocapture
//! ```
//!
//! System Proxy is deliberately left disabled: a 72-hour run must not hold the
//! host's proxy settings hostage. System Proxy save/restore has its own tests.

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use magies_core_runtime::{
    CoreBinaryRequirement, Sha256Hash, SingBoxAdapter, ValidatedCoreBinary, locate_core_binary,
};
use magies_domain::CredentialRef;
use magies_platform::CpuArchitecture;
use magies_platform::system_proxy::SystemProxyState;
use magies_profiles::{
    CredentialCodec, DnsProfile, DnsServer, DnsStrategy, LocalHttpProfile, LocalSocksProfile,
    ShadowsocksParser, StoredNodeCredential,
};
use magies_routing::{RouteOutbound, RouteProfile, RoutingMode};
use magies_session::{
    DesktopSession, DesktopSessionProfile, NetworkEvent, NetworkRecoveryPolicy, RecoveryOutcome,
    SessionHealthProbe, SingBoxCoreControl, SystemProxySessionControl, TcpHealthProbe,
};
use magies_storage::{MemorySecretStore, SecretStore};
use uuid::Uuid;

/// Kept short so an explicit run verifies the harness in seconds. The PRD's
/// 72 hours is `MAGIES_SOAK_DURATION_SECS=259200`.
const DEFAULT_DURATION: Duration = Duration::from_secs(20);
const PROBE_INTERVAL: Duration = Duration::from_millis(250);
const HEALTH_TIMEOUT: Duration = Duration::from_secs(10);
const PROBE_TIMEOUT: Duration = Duration::from_millis(500);
const DEBOUNCE: Duration = Duration::from_millis(50);
/// How many probe ticks pass between forced recovery cycles.
const RECOVERY_EVERY: u32 = 8;

#[test]
#[ignore = "long-running stability soak; set MAGIES_SOAK_DURATION_SECS for the PRD's 72 hours"]
fn the_session_survives_continuous_probing_and_repeated_recovery() {
    let duration = configured_duration();
    let core = core_binary();
    let ports = free_ports();
    let runtime = RuntimeDirectory::new("soak");
    let store = MemorySecretStore::default();
    let profile = soak_profile(&store, ports);
    let health_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), ports.socks);

    let mut session = DesktopSession::new(
        store,
        SingBoxCoreControl::new(SingBoxAdapter::new(core), health_address, HEALTH_TIMEOUT),
        NoSystemProxy,
        runtime.path(),
    );
    session
        .start(&profile)
        .expect("the soak Core must start before the soak begins");

    let probe = TcpHealthProbe::new(health_address, PROBE_TIMEOUT);
    let mut policy = NetworkRecoveryPolicy::new(DEBOUNCE);
    let mut report = SoakReport::default();
    let started_at = Instant::now();

    while started_at.elapsed() < duration {
        std::thread::sleep(PROBE_INTERVAL);
        report.ticks += 1;

        if probe.is_healthy() {
            report.healthy_probes += 1;
        } else {
            report.failed_probes += 1;
        }

        // Every run leaves exactly one config behind; anything else means the
        // atomic-write or cleanup path is leaking files over time.
        let configs = runtime.file_count();
        report.max_runtime_files = report.max_runtime_files.max(configs);

        if report.ticks % u64::from(RECOVERY_EVERY) == 0 {
            let now = Instant::now();
            policy.observe(NetworkEvent::PathChanged, now);
            match policy.recover(now + DEBOUNCE, &mut session, &ForceUnhealthy) {
                Ok(RecoveryOutcome::Reconnected { attempts }) => {
                    report.reconnects += 1;
                    report.max_reconnect_attempts = report.max_reconnect_attempts.max(attempts);
                }
                Ok(outcome) => panic!("a forced recovery must reconnect, got {outcome:?}"),
                Err(error) => panic!(
                    "recovery failed after {} cycles and {:?}: {error}",
                    report.reconnects,
                    started_at.elapsed()
                ),
            }
        }
    }

    println!("{report}");
    println!("soak duration: {:?}", started_at.elapsed());

    assert!(session.is_running(), "the session must survive the soak");
    assert!(
        probe.is_healthy(),
        "the Core must still answer after {:?}",
        started_at.elapsed()
    );
    assert_eq!(
        report.max_runtime_files, 1,
        "runtime configs accumulated: {report}"
    );
    assert_eq!(
        report.failed_probes, 0,
        "the Core stopped answering during the soak: {report}"
    );
    assert!(
        report.reconnects > 0,
        "the soak must exercise at least one recovery cycle: {report}"
    );

    session.stop().expect("the soak session must stop cleanly");
    assert_eq!(
        runtime.file_count(),
        0,
        "stopping must remove the runtime config"
    );
}

#[derive(Debug, Default)]
struct SoakReport {
    ticks: u64,
    healthy_probes: u64,
    failed_probes: u64,
    reconnects: u64,
    max_reconnect_attempts: u8,
    max_runtime_files: usize,
}

impl Display for SoakReport {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "soak: ticks={} healthy={} failed={} reconnects={} \
             max_reconnect_attempts={} max_runtime_files={}",
            self.ticks,
            self.healthy_probes,
            self.failed_probes,
            self.reconnects,
            self.max_reconnect_attempts,
            self.max_runtime_files
        )
    }
}

fn configured_duration() -> Duration {
    std::env::var("MAGIES_SOAK_DURATION_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map_or(DEFAULT_DURATION, Duration::from_secs)
}

fn core_binary() -> ValidatedCoreBinary {
    let path =
        std::env::var_os("MAGIES_SOAK_CORE_BIN").map_or_else(compile_soak_core, PathBuf::from);
    let contents = fs::read(&path).expect("the soak Core must be readable");
    locate_core_binary(
        &path,
        CoreBinaryRequirement::new(build_architecture(), Sha256Hash::digest(&contents)),
    )
    .expect("the soak Core must pass binary validation")
}

fn compile_soak_core() -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("soak_core.rs");
    let output = std::env::temp_dir().join(format!(
        "magies-soak-core-{}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    let status = Command::new("rustc")
        .args(["--edition=2024", "-O"])
        .arg(source)
        .arg("-o")
        .arg(&output)
        .status()
        .expect("rustc must be available to compile the soak Core");
    assert!(status.success(), "the soak Core fixture must compile");
    output
}

fn build_architecture() -> CpuArchitecture {
    match std::env::consts::ARCH {
        "x86_64" => CpuArchitecture::X86_64,
        "aarch64" => CpuArchitecture::Aarch64,
        architecture => panic!("unsupported soak architecture: {architecture}"),
    }
}

#[derive(Clone, Copy)]
struct Ports {
    socks: u16,
    http: u16,
}

/// Binds ephemeral ports, reads what the OS assigned, then releases them so the
/// Core can claim the same numbers.
fn free_ports() -> Ports {
    let sockets: Vec<_> = (0..2)
        .map(|_| TcpListener::bind(("127.0.0.1", 0)).expect("a loopback port must be available"))
        .collect();
    let numbers: Vec<_> = sockets
        .iter()
        .map(|socket| {
            socket
                .local_addr()
                .expect("a bound socket has an address")
                .port()
        })
        .collect();
    drop(sockets);
    Ports {
        socks: numbers[0],
        http: numbers[1],
    }
}

fn soak_profile(store: &MemorySecretStore, ports: Ports) -> DesktopSessionProfile {
    let parsed = ShadowsocksParser
        .parse("ss://aes-128-gcm:soak-secret@127.0.0.1:18388")
        .expect("the soak share link must parse");
    let credential_ref = CredentialRef::new("secret://nodes/soak").expect("a valid reference");
    store
        .put(
            &credential_ref,
            &CredentialCodec::encode(&StoredNodeCredential::from(parsed.credential()))
                .expect("the credential must encode"),
        )
        .expect("the credential must be stored");
    let node = parsed
        .into_proxy_node(Uuid::nil(), credential_ref)
        .expect("the soak node must be valid");
    let dns = DnsProfile::new(
        vec![DnsServer::system("system").expect("a valid tag")],
        Vec::new(),
        "system",
        DnsStrategy::PreferIpv4,
        false,
        false,
    )
    .expect("the soak DNS profile must be valid");
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy)
        .expect("the soak route must be valid");

    DesktopSessionProfile::new(node, dns, route)
        .with_local_proxies(
            LocalSocksProfile::new(u32::from(ports.socks)).expect("a valid SOCKS port"),
            LocalHttpProfile::new(u32::from(ports.http)).expect("a valid HTTP port"),
        )
        .with_system_proxy(false)
}

/// Forces the reconnect path so the soak exercises stop/start rather than
/// waiting for a real outage.
struct ForceUnhealthy;

impl SessionHealthProbe for ForceUnhealthy {
    fn is_healthy(&self) -> bool {
        false
    }
}

/// The soak must never touch the host's real System Proxy.
struct NoSystemProxy;

#[derive(Debug)]
struct NeverFails;

impl Display for NeverFails {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the soak System Proxy control never fails")
    }
}

impl Error for NeverFails {}

impl SystemProxySessionControl for NoSystemProxy {
    type Error = NeverFails;

    fn enable(&mut self, _state: &SystemProxyState) -> Result<(), Self::Error> {
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

struct RuntimeDirectory(PathBuf);

impl RuntimeDirectory {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("magies-session-{name}-{}", std::process::id()));
        fs::create_dir_all(&path).expect("the soak runtime directory must be creatable");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn file_count(&self) -> usize {
        fs::read_dir(&self.0)
            .expect("the soak runtime directory must be readable")
            .count()
    }
}

impl Drop for RuntimeDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            if error.kind() != ErrorKind::NotFound {
                eprintln!(
                    "failed to remove soak runtime directory {}: {error}",
                    self.0.display()
                );
            }
        }
    }
}
