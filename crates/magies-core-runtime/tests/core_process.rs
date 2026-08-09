use std::env::current_exe;
use std::error::Error;
use std::fs::{copy, read, remove_file};
use std::io;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use magies_core_runtime::{
    CoreBinaryRequirement, CoreExit, CoreProcessSpec, CoreRuntime, CoreRuntimeError, CoreState,
    Sha256Hash, ValidatedCoreBinary, locate_core_binary,
};
use magies_platform::CpuArchitecture;

const TEST_TIMEOUT: Duration = Duration::from_secs(2);

fn helper_process_spec(test_name: &str) -> CoreProcessSpec {
    let binary =
        validated_binary(&current_exe().expect("the integration test executable must exist"));
    CoreProcessSpec::new(&binary, ["--ignored", "--exact", test_name])
}

fn validated_binary(path: &std::path::Path) -> ValidatedCoreBinary {
    let contents = read(path).expect("the integration test executable must be readable");
    locate_core_binary(
        path,
        CoreBinaryRequirement::new(build_architecture(), Sha256Hash::digest(&contents)),
    )
    .unwrap()
}

fn build_architecture() -> CpuArchitecture {
    match std::env::consts::ARCH {
        "x86_64" => CpuArchitecture::X86_64,
        "aarch64" => CpuArchitecture::Aarch64,
        architecture => panic!("unsupported test architecture: {architecture}"),
    }
}

#[test]
fn starts_reports_health_and_stops_a_core_process() {
    let mut runtime = CoreRuntime::default();
    let spec = helper_process_spec("helper_core_runs_until_stopped");

    assert_eq!(runtime.poll().unwrap(), CoreState::Stopped);
    runtime.start(&spec).unwrap();
    assert_eq!(runtime.poll().unwrap(), CoreState::Running);
    assert!(matches!(
        runtime.start(&spec),
        Err(CoreRuntimeError::AlreadyRunning)
    ));

    runtime.stop().unwrap();
    assert_eq!(runtime.poll().unwrap(), CoreState::Stopped);
    assert!(matches!(runtime.stop(), Err(CoreRuntimeError::NotRunning)));
    assert!(matches!(
        runtime.wait_for_exit(TEST_TIMEOUT),
        Err(CoreRuntimeError::NotRunning)
    ));
}

#[test]
fn reports_an_early_core_exit() {
    let mut runtime = CoreRuntime::default();
    let spec = helper_process_spec("helper_core_exits_immediately");

    runtime.start(&spec).unwrap();

    assert_eq!(
        runtime.wait_for_exit(TEST_TIMEOUT).unwrap(),
        CoreExit {
            success: true,
            code: Some(0),
        }
    );
    assert_eq!(
        runtime.poll().unwrap(),
        CoreState::Exited(CoreExit {
            success: true,
            code: Some(0),
        })
    );
}

#[test]
fn returns_a_typed_timeout_while_the_core_is_still_running() {
    let mut runtime = CoreRuntime::default();
    let spec = helper_process_spec("helper_core_runs_until_stopped");
    let timeout = Duration::from_millis(20);

    runtime.start(&spec).unwrap();

    assert!(matches!(
        runtime.wait_for_exit(timeout),
        Err(CoreRuntimeError::WaitTimedOut { timeout: actual }) if actual == timeout
    ));
    assert_eq!(runtime.poll().unwrap(), CoreState::Running);
    runtime.stop().unwrap();
}

#[test]
fn preserves_stopped_state_when_spawning_fails() {
    let mut runtime = CoreRuntime::default();
    let source = current_exe().unwrap();
    let missing_executable = std::env::temp_dir().join(format!(
        "definitely-missing-magies-core-{}",
        std::process::id()
    ));
    copy(&source, &missing_executable).unwrap();
    let binary = validated_binary(&missing_executable);
    let resolved_executable = binary.path().to_path_buf();
    let spec = CoreProcessSpec::new(&binary, std::iter::empty::<&str>());
    remove_file(&missing_executable).unwrap();

    let error = runtime.start(&spec).unwrap_err();

    assert!(matches!(
        error,
        CoreRuntimeError::SpawnFailed { executable, .. }
            if executable == resolved_executable
    ));
    assert_eq!(runtime.poll().unwrap(), CoreState::Stopped);
}

#[test]
fn exposes_actionable_messages_and_sources_for_typed_errors() {
    let errors = [
        (
            CoreRuntimeError::AlreadyRunning,
            "core process is already running",
            false,
        ),
        (
            CoreRuntimeError::NotRunning,
            "core process is not running",
            false,
        ),
        (
            CoreRuntimeError::SpawnFailed {
                executable: PathBuf::from("xray"),
                source: io::Error::new(io::ErrorKind::NotFound, "missing"),
            },
            "failed to start core process xray: missing",
            true,
        ),
        (
            CoreRuntimeError::PollFailed(io::Error::other("poll")),
            "failed to poll core process: poll",
            true,
        ),
        (
            CoreRuntimeError::TerminateFailed(io::Error::other("terminate")),
            "failed to terminate core process: terminate",
            true,
        ),
        (
            CoreRuntimeError::WaitFailed(io::Error::other("wait")),
            "failed to wait for core process: wait",
            true,
        ),
        (
            CoreRuntimeError::WaitTimedOut {
                timeout: Duration::from_secs(2),
            },
            "core process did not exit within 2s",
            false,
        ),
    ];

    for (error, expected_message, has_source) in errors {
        assert_eq!(error.to_string(), expected_message);
        assert_eq!(error.source().is_some(), has_source);
    }
}

#[test]
fn dropping_a_running_runtime_reaps_its_process() {
    let mut runtime = CoreRuntime::default();
    runtime
        .start(&helper_process_spec("helper_core_runs_until_stopped"))
        .unwrap();

    drop(runtime);
}

#[test]
#[ignore = "spawned by lifecycle tests as a long-running fake core"]
fn helper_core_runs_until_stopped() {
    sleep(Duration::from_secs(60));
}

#[test]
#[ignore = "spawned by lifecycle tests as a short-lived fake core"]
fn helper_core_exits_immediately() {}
