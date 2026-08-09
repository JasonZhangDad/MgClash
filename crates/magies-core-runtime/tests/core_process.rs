use std::env::current_exe;
use std::error::Error;
use std::fs::{copy, read, remove_file, write};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::mpsc::RecvTimeoutError;
use std::thread::sleep;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use magies_core_runtime::{
    CoreBinaryError, CoreBinaryRequirement, CoreExit, CoreOutputEvent, CoreOutputStream,
    CoreProcessSpec, CoreRuntime, CoreRuntimeError, CoreState, Sha256Hash, ValidatedCoreBinary,
    locate_core_binary,
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

    runtime.start(&spec).unwrap();
    assert_eq!(runtime.poll().unwrap(), CoreState::Running);
    runtime.stop().unwrap();
}

#[test]
fn streams_raw_stdout_and_stderr_while_the_core_is_running() {
    let mut runtime = CoreRuntime::default();
    let spec = helper_process_spec("helper_core_writes_output_then_runs");

    let output = runtime.start(&spec).unwrap();
    let deadline = Instant::now() + TEST_TIMEOUT;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    while !contains_bytes(&stdout, b"stdout:\xff\n") || !contains_bytes(&stderr, b"stderr:\xfe\n") {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "timed out waiting for Core output");
        match output.recv_timeout(remaining).unwrap() {
            CoreOutputEvent::Chunk { stream, bytes } => match stream {
                CoreOutputStream::Stdout => stdout.extend(bytes),
                CoreOutputStream::Stderr => stderr.extend(bytes),
            },
            CoreOutputEvent::ReadFailed { stream, source } => {
                panic!("failed to read {stream:?}: {source}")
            }
        }
    }

    assert_eq!(runtime.poll().unwrap(), CoreState::Running);
    runtime.stop().unwrap();
}

#[test]
fn closes_the_output_stream_after_a_natural_exit() {
    let mut runtime = CoreRuntime::default();
    let spec = helper_process_spec("helper_core_writes_output_then_exits");

    let output = runtime.start(&spec).unwrap();
    assert_eq!(
        runtime.wait_for_exit(TEST_TIMEOUT).unwrap(),
        CoreExit {
            success: true,
            code: Some(0),
        }
    );

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    loop {
        match output.recv_timeout(TEST_TIMEOUT) {
            Ok(CoreOutputEvent::Chunk { stream, bytes }) => match stream {
                CoreOutputStream::Stdout => stdout.extend(bytes),
                CoreOutputStream::Stderr => stderr.extend(bytes),
            },
            Ok(CoreOutputEvent::ReadFailed { stream, source }) => {
                panic!("failed to read {stream:?}: {source}")
            }
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => panic!("Core output stream remained open"),
        }
    }

    assert!(contains_bytes(&stdout, b"finished stdout\n"));
    assert!(contains_bytes(&stderr, b"finished stderr\n"));
}

#[test]
fn keeps_draining_output_after_the_receiver_is_dropped() {
    let mut runtime = CoreRuntime::default();
    let spec = helper_process_spec("helper_core_writes_large_output_then_exits");

    drop(runtime.start(&spec).unwrap());

    assert_eq!(
        runtime.wait_for_exit(TEST_TIMEOUT).unwrap(),
        CoreExit {
            success: true,
            code: Some(0),
        }
    );
}

fn contains_bytes(contents: &[u8], expected: &[u8]) -> bool {
    contents
        .windows(expected.len())
        .any(|window| window == expected)
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
fn refuses_a_validated_core_binary_deleted_before_startup() {
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
        CoreRuntimeError::BinaryValidationFailed(CoreBinaryError::NotFound { path })
            if path == resolved_executable
    ));
    assert_eq!(runtime.poll().unwrap(), CoreState::Stopped);
}

#[cfg(unix)]
#[test]
fn preserves_stopped_state_when_spawning_fails() {
    let source = current_exe().unwrap();
    let executable =
        std::env::temp_dir().join(format!("non-executable-magies-core-{}", std::process::id()));
    copy(&source, &executable).unwrap();
    let binary = validated_binary(&executable);
    let resolved_executable = binary.path().to_path_buf();
    let spec = CoreProcessSpec::new(&binary, std::iter::empty::<&str>());
    let mut permissions = executable.metadata().unwrap().permissions();
    permissions.set_mode(0o600);
    std::fs::set_permissions(&executable, permissions).unwrap();
    let mut runtime = CoreRuntime::default();

    let error = runtime.start(&spec).unwrap_err();

    assert!(matches!(
        error,
        CoreRuntimeError::SpawnFailed { executable, .. }
            if executable == resolved_executable
    ));
    assert_eq!(runtime.poll().unwrap(), CoreState::Stopped);
    remove_file(executable).unwrap();
}

#[test]
fn refuses_a_core_binary_changed_after_the_process_spec_was_created() {
    let source = current_exe().unwrap();
    let executable = std::env::temp_dir().join(format!(
        "changed-magies-core-{}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    copy(&source, &executable).unwrap();
    let binary = validated_binary(&executable);
    let resolved_executable = binary.path().to_path_buf();
    let spec = CoreProcessSpec::new(&binary, std::iter::empty::<&str>());
    let mut contents = read(&executable).unwrap();
    *contents.last_mut().unwrap() ^= 0xff;
    write(&executable, contents).unwrap();
    let mut runtime = CoreRuntime::default();

    let error = runtime.start(&spec).unwrap_err();

    assert!(matches!(
        error,
        CoreRuntimeError::BinaryValidationFailed(CoreBinaryError::HashMismatch { path, .. })
            if path == resolved_executable
    ));
    assert_eq!(runtime.poll().unwrap(), CoreState::Stopped);
    remove_file(executable).unwrap();
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
            CoreRuntimeError::BinaryValidationFailed(CoreBinaryError::NotFound {
                path: PathBuf::from("xray"),
            }),
            "Core binary failed launch validation: Core binary not found: \"xray\"",
            true,
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
            CoreRuntimeError::OutputReaderSpawnFailed {
                stream: CoreOutputStream::Stdout,
                source: io::Error::other("reader spawn"),
            },
            "failed to start Core Stdout reader: reader spawn",
            true,
        ),
        (
            CoreRuntimeError::OutputPipeUnavailable(CoreOutputStream::Stdout),
            "Core Stdout pipe is unavailable",
            false,
        ),
        (
            CoreRuntimeError::OutputReaderPanicked(CoreOutputStream::Stderr),
            "Core Stderr reader panicked",
            false,
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

#[test]
#[ignore = "spawned by output tests as a long-running fake core"]
fn helper_core_writes_output_then_runs() {
    io::stdout().write_all(b"stdout:\xff\n").unwrap();
    io::stdout().flush().unwrap();
    io::stderr().write_all(b"stderr:\xfe\n").unwrap();
    io::stderr().flush().unwrap();
    sleep(Duration::from_secs(60));
}

#[test]
#[ignore = "spawned by output tests as a short-lived fake core"]
fn helper_core_writes_output_then_exits() {
    io::stdout().write_all(b"finished stdout\n").unwrap();
    io::stderr().write_all(b"finished stderr\n").unwrap();
}

#[test]
#[ignore = "spawned by output tests as a high-output fake core"]
fn helper_core_writes_large_output_then_exits() {
    let output = vec![b'x'; 1024 * 1024];
    io::stdout().write_all(&output).unwrap();
    io::stderr().write_all(&output).unwrap();
}
