#[allow(dead_code)]
mod common;

use std::error::Error;
use std::fs::remove_file;
use std::net::{SocketAddr, TcpListener};
use std::thread::sleep;
use std::time::{Duration, Instant};

use magies_core_runtime::{
    CoreHealthError, CoreProcessSpec, CoreRuntimeError, TunFailure, TunRuntime, TunRuntimeError,
    TunState,
};
use magies_platform::{OperatingSystem, TargetPlatform};

const HEALTH_TIMEOUT: Duration = Duration::from_secs(2);
/// How long to wait for a Core that exits on its own to be reaped. Generous on
/// purpose: a loaded CI runner can take far longer than the fixture's lifetime.
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Polls until the Core is no longer running, or the deadline passes.
///
/// Sleeping for a fixed margin past the fixture's lifetime and asserting once
/// raced: the child's sleep is a lower bound, and reaping it costs more time
/// still, so a busy runner reported `Running` and failed the assertion.
fn poll_until_not_running(runtime: &mut TunRuntime) -> TunState {
    let deadline = Instant::now() + EXIT_TIMEOUT;
    loop {
        let state = runtime.poll().unwrap();
        if state != TunState::Running || Instant::now() >= deadline {
            return state;
        }
        sleep(POLL_INTERVAL);
    }
}

fn target(os: &str, architecture: &str) -> TargetPlatform {
    TargetPlatform::parse(os, architecture).unwrap()
}

fn fake_spec(mode: &str, port: u16, lifetime: Duration) -> (std::path::PathBuf, CoreProcessSpec) {
    let path = common::compile_fixture("fake_tun_core.rs", "fake-tun-core");
    let binary = common::validated_binary(&path);
    let spec = CoreProcessSpec::new(
        &binary,
        [
            mode.to_owned(),
            port.to_string(),
            lifetime.as_millis().to_string(),
        ],
    );
    (path, spec)
}

fn available_address() -> SocketAddr {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    address
}

#[test]
fn starts_waits_for_readiness_and_stops_idempotently() {
    let address = available_address();
    let (binary, spec) = fake_spec("listen", address.port(), Duration::from_secs(60));
    let mut runtime = TunRuntime::default();

    assert_eq!(runtime.state(), TunState::Stopped);
    assert_eq!(runtime.poll().unwrap(), TunState::Stopped);
    let started = runtime
        .start(target("windows", "x86_64"), &spec, address, HEALTH_TIMEOUT)
        .unwrap();

    assert!(started.health.ready_after <= HEALTH_TIMEOUT);
    assert_eq!(runtime.state(), TunState::Running);
    assert_eq!(runtime.poll().unwrap(), TunState::Running);
    assert!(matches!(
        runtime.start(target("windows", "x86_64"), &spec, address, HEALTH_TIMEOUT),
        Err(TunRuntimeError::AlreadyRunning)
    ));
    runtime.stop().unwrap();
    runtime.stop().unwrap();
    assert_eq!(runtime.state(), TunState::Stopped);
    remove_file(binary).unwrap();
}

#[test]
fn startup_validation_failure_enters_a_typed_failed_state() {
    let address = available_address();
    let (binary, spec) = fake_spec("run", address.port(), Duration::ZERO);
    remove_file(binary).unwrap();
    let mut runtime = TunRuntime::default();

    assert!(matches!(
        runtime.start(target("linux", "x86_64"), &spec, address, HEALTH_TIMEOUT),
        Err(TunRuntimeError::Start(
            CoreRuntimeError::BinaryValidationFailed(_)
        ))
    ));
    assert_eq!(runtime.state(), TunState::Failed(TunFailure::Start));
    runtime.stop().unwrap();
    assert_eq!(runtime.state(), TunState::Stopped);
}

#[test]
fn rejects_unsigned_macos_before_starting_a_core() {
    let address = available_address();
    let (binary, spec) = fake_spec("run", address.port(), Duration::ZERO);
    let mut runtime = TunRuntime::default();

    assert!(matches!(
        runtime.start(target("macos", "x86_64"), &spec, address, HEALTH_TIMEOUT),
        Err(TunRuntimeError::UnsupportedPlatform(OperatingSystem::MacOs))
    ));
    assert_eq!(runtime.state(), TunState::Stopped);
    assert_eq!(runtime.process_id(), None);
    remove_file(binary).unwrap();
}

#[test]
fn failed_readiness_cleans_up_the_core_and_can_be_reset() {
    let address = available_address();
    let (binary, spec) = fake_spec("run", address.port(), Duration::ZERO);
    let mut runtime = TunRuntime::default();
    let timeout = Duration::from_millis(30);

    let error = runtime
        .start(target("linux", "x86_64"), &spec, address, timeout)
        .unwrap_err();

    assert!(matches!(
        error,
        TunRuntimeError::Readiness(CoreHealthError::TimedOut {
            timeout: actual,
            ..
        }) if actual == timeout
    ));
    assert_eq!(runtime.state(), TunState::Failed(TunFailure::Readiness));
    assert_eq!(runtime.process_id(), None);
    runtime.stop().unwrap();
    assert_eq!(runtime.state(), TunState::Stopped);
    remove_file(binary).unwrap();
}

#[test]
fn polling_reports_a_core_that_exits_after_startup() {
    let address = available_address();
    let (binary, spec) = fake_spec("serve-once", address.port(), Duration::ZERO);
    let mut runtime = TunRuntime::default();
    runtime
        .start(target("linux", "x86_64"), &spec, address, HEALTH_TIMEOUT)
        .unwrap();

    let state = poll_until_not_running(&mut runtime);

    assert!(matches!(
        state,
        TunState::Failed(TunFailure::CoreExited(exit)) if exit.success
    ));
    runtime.stop().unwrap();
    assert_eq!(runtime.state(), TunState::Stopped);
    remove_file(binary).unwrap();
}

#[test]
fn startup_process_exit_is_a_readiness_failure() {
    let address = available_address();
    let (binary, spec) = fake_spec("exit", address.port(), Duration::ZERO);
    let mut runtime = TunRuntime::default();

    assert!(matches!(
        runtime.start(
            target("windows", "x86_64"),
            &spec,
            address,
            HEALTH_TIMEOUT
        ),
        Err(TunRuntimeError::Readiness(CoreHealthError::ProcessExited(exit)))
            if !exit.success && exit.code == Some(17)
    ));
    assert_eq!(runtime.state(), TunState::Failed(TunFailure::Readiness));
    runtime.stop().unwrap();
    remove_file(binary).unwrap();
}

#[test]
fn exposes_actionable_error_messages_and_sources() {
    let errors = [
        (
            TunRuntimeError::UnsupportedPlatform(OperatingSystem::MacOs),
            "TUN is unavailable for unsigned MacOs builds",
            false,
        ),
        (
            TunRuntimeError::AlreadyRunning,
            "TUN is already running",
            false,
        ),
        (
            TunRuntimeError::Start(CoreRuntimeError::NotRunning),
            "failed to start TUN Core: core process is not running",
            true,
        ),
        (
            TunRuntimeError::Readiness(CoreHealthError::NotRunning),
            "TUN did not become ready: cannot check health without a running Core",
            true,
        ),
        (
            TunRuntimeError::ReadinessCleanup {
                readiness: CoreHealthError::NotRunning,
                cleanup: Box::new(CoreRuntimeError::NotRunning),
            },
            "TUN did not become ready (cannot check health without a running Core) and its Core could not be stopped: core process is not running",
            true,
        ),
        (
            TunRuntimeError::Poll(CoreRuntimeError::NotRunning),
            "failed to inspect TUN Core: core process is not running",
            true,
        ),
        (
            TunRuntimeError::Stop(CoreRuntimeError::NotRunning),
            "failed to stop TUN Core: core process is not running",
            true,
        ),
    ];

    for (error, message, has_source) in errors {
        assert_eq!(error.to_string(), message);
        assert_eq!(error.source().is_some(), has_source);
    }
}
