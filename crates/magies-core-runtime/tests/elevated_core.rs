#![cfg(unix)]
//! Covers the lifecycle around an elevated Core.
//!
//! The privilege escalation itself cannot run unattended — it asks for a
//! password — so these tests substitute a plain shell for the prompt and
//! exercise everything around it: the script that gets approved, the PID file
//! it leaves, liveness, and stopping.

use std::fs;
use std::net::SocketAddr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use std::sync::mpsc::RecvTimeoutError;

use magies_core_runtime::CoreOutputEvent;
use magies_core_runtime::elevated::{
    ElevatedCore, ElevatedCoreError, ElevationLauncher, elevation_script,
};

static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

fn scratch(name: &str) -> PathBuf {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "mgclash-elevated-{name}-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

/// Runs the approved script through `sh`, which is what the prompt does once
/// the user has typed their password.
struct ShellLauncher;

impl ElevationLauncher for ShellLauncher {
    fn launch(&self, script: &str) -> Result<(), ElevatedCoreError> {
        let status = Command::new("/bin/sh").arg("-c").arg(script).status();
        assert!(status.is_ok_and(|status| status.success()), "{script}");
        Ok(())
    }
}

/// Refuses, the way a dismissed prompt does.
struct DecliningLauncher;

impl ElevationLauncher for DecliningLauncher {
    fn launch(&self, _script: &str) -> Result<(), ElevatedCoreError> {
        Err(ElevatedCoreError::AuthorizationDeclined)
    }
}

/// A stand-in Core: the script always calls it as `<binary> run -c <config>`,
/// so the fake has to ignore those arguments rather than choke on them.
fn fake_core(directory: &Path, body: &str) -> PathBuf {
    let path = directory.join("fake-core");
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

fn wait_until(condition: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn the_approved_script_is_one_readable_line() {
    let script = elevation_script(
        Path::new("/cores/sing-box"),
        Path::new("/run/session.json"),
        Path::new("/run/core.pid"),
        Path::new("/run/core.log"),
    );

    // The prompt shows this to the user, so it has to say plainly what it runs.
    assert_eq!(
        script,
        "'/cores/sing-box' run -c '/run/session.json' >> '/run/core.log' 2>&1 & echo $! > '/run/core.pid'"
    );
}

#[test]
fn a_path_with_a_quote_cannot_break_out_of_the_script() {
    let script = elevation_script(
        Path::new("/cores/sing'box"),
        Path::new("/run/session.json"),
        Path::new("/run/core.pid"),
        Path::new("/run/core.log"),
    );

    assert!(script.starts_with(r"'/cores/sing'\''box' run"));
}

#[test]
fn starting_records_the_pid_and_stopping_ends_the_process() {
    let directory = scratch("lifecycle");
    let pid_file = directory.join("core.pid");
    let log_file = directory.join("core.log");
    let mut core = ElevatedCore::new(ShellLauncher, &pid_file, &log_file);

    let binary = fake_core(&directory, "sleep 30");
    let (pid, _output) = core
        .start(&binary, Path::new("session.json"))
        .unwrap_or_else(|error| panic!("start failed: {error}"));

    assert!(pid > 0);
    assert_eq!(core.pid(), Some(pid));
    assert!(core.is_running());

    core.stop().unwrap();

    assert_eq!(core.pid(), None);
    assert!(wait_until(|| !pid_file.exists()));
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn a_core_that_dies_immediately_is_reported_as_not_started() {
    let directory = scratch("dies");
    let mut core = ElevatedCore::new(
        ShellLauncher,
        directory.join("core.pid"),
        directory.join("core.log"),
    );

    // A Core that exits at once still gets a pid recorded, so the failure shows
    // up as a process that is no longer alive rather than a missing file.
    let binary = fake_core(&directory, "exit 1");
    core.start(&binary, Path::new("session.json")).unwrap();

    assert!(wait_until(|| !core.is_running()));
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn stopping_a_core_that_never_started_is_not_an_error() {
    let directory = scratch("never");
    let mut core = ElevatedCore::new(
        ShellLauncher,
        directory.join("core.pid"),
        directory.join("core.log"),
    );

    core.stop().unwrap();

    assert!(!core.is_running());
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn a_dismissed_prompt_is_a_typed_refusal() {
    let directory = scratch("declined");
    let mut core = ElevatedCore::new(
        DecliningLauncher,
        directory.join("core.pid"),
        directory.join("core.log"),
    );

    let error = core
        .start(Path::new("/does/not/matter"), Path::new("session.json"))
        .unwrap_err();

    assert_eq!(error.code(), "tun_authorization_declined");
    assert_eq!(core.pid(), None);
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn the_core_log_is_where_the_script_pointed_it() {
    let directory = scratch("log");
    let log_file = directory.join("core.log");
    let mut core = ElevatedCore::new(ShellLauncher, directory.join("core.pid"), &log_file);

    let binary = fake_core(&directory, "echo started");
    core.start(&binary, Path::new("session.json")).unwrap();

    assert!(wait_until(
        || fs::read_to_string(&log_file).is_ok_and(|text| text.contains("started"))
    ));
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn the_log_reaches_the_output_channel_and_ends_with_the_core() {
    let directory = scratch("output");
    let log_file = directory.join("core.log");
    let mut core = ElevatedCore::new(ShellLauncher, directory.join("core.pid"), &log_file);

    // A Core that keeps writing: the log panel has to show lines as they
    // arrive, not only once the process is gone.
    let binary = fake_core(
        &directory,
        "for i in 1 2 3; do echo line$i; sleep 0.2; done",
    );
    let (_pid, output) = core.start(&binary, Path::new("session.json")).unwrap();

    let mut text = String::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !text.contains("line3") && Instant::now() < deadline {
        if let Ok(CoreOutputEvent::Chunk { bytes, .. }) =
            output.recv_timeout(Duration::from_millis(500))
        {
            text.push_str(&String::from_utf8_lossy(&bytes));
        }
    }
    assert!(text.contains("line1"), "{text}");
    assert!(text.contains("line3"), "{text}");

    core.stop().unwrap();

    // Stopping the Core has to end the stream too, or the panel keeps a reader
    // alive against a log nothing writes to.
    assert!(matches!(
        output.recv_timeout(Duration::from_secs(2)),
        Err(RecvTimeoutError::Disconnected)
    ));
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn a_stale_log_from_the_last_session_is_not_replayed() {
    let directory = scratch("stale");
    let log_file = directory.join("core.log");
    fs::write(&log_file, "from an earlier session\n").unwrap();
    let mut core = ElevatedCore::new(ShellLauncher, directory.join("core.pid"), &log_file);

    let binary = fake_core(&directory, "echo fresh; sleep 5");
    let (_pid, output) = core.start(&binary, Path::new("session.json")).unwrap();

    let mut text = String::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !text.contains("fresh") && Instant::now() < deadline {
        if let Ok(CoreOutputEvent::Chunk { bytes, .. }) =
            output.recv_timeout(Duration::from_millis(500))
        {
            text.push_str(&String::from_utf8_lossy(&bytes));
        }
    }
    assert!(!text.contains("earlier session"), "{text}");

    core.stop().unwrap();
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn restarting_the_same_core_gets_a_working_output_again() {
    let directory = scratch("restart");
    let log_file = directory.join("core.log");
    let mut core = ElevatedCore::new(ShellLauncher, directory.join("core.pid"), &log_file);
    let binary = fake_core(&directory, "echo running; sleep 5");

    // Reconnecting after a routing change stops and starts the same control,
    // so the second run must stream its log like the first.
    let (_pid, first) = core.start(&binary, Path::new("session.json")).unwrap();
    drop(first);
    core.stop().unwrap();

    let (_pid, output) = core.start(&binary, Path::new("session.json")).unwrap();

    let mut text = String::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !text.contains("running") && Instant::now() < deadline {
        if let Ok(CoreOutputEvent::Chunk { bytes, .. }) =
            output.recv_timeout(Duration::from_millis(500))
        {
            text.push_str(&String::from_utf8_lossy(&bytes));
        }
    }
    assert!(text.contains("running"), "{text}");

    core.stop().unwrap();
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn health_waits_for_the_port_the_elevated_core_opens() {
    let directory = scratch("health");
    let mut core = ElevatedCore::new(
        ShellLauncher,
        directory.join("core.pid"),
        directory.join("core.log"),
    );
    let port = free_port();

    // An elevated Core is not a child process, so readiness can only be the
    // same thing the user cares about: the local port answering.
    let binary = fake_core(
        &directory,
        &format!("sleep 0.3; nc -l {port} > /dev/null 2>&1 || sleep 5"),
    );
    core.start(&binary, Path::new("session.json")).unwrap();

    let health = core
        .wait_for_tcp_health(
            SocketAddr::from(([127, 0, 0, 1], port)),
            Duration::from_secs(5),
        )
        .unwrap_or_else(|error| panic!("health failed: {error}"));

    assert!(health.ready_after <= Duration::from_secs(5));
    core.stop().unwrap();
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn health_gives_up_typed_when_the_elevated_core_dies() {
    let directory = scratch("health-dead");
    let mut core = ElevatedCore::new(
        ShellLauncher,
        directory.join("core.pid"),
        directory.join("core.log"),
    );

    // A Core that exits — a bad config under root, say — must not keep the
    // caller waiting for the full timeout.
    let binary = fake_core(&directory, "exit 1");
    core.start(&binary, Path::new("session.json")).unwrap();

    let error = core
        .wait_for_tcp_health(
            SocketAddr::from(([127, 0, 0, 1], free_port())),
            Duration::from_secs(30),
        )
        .unwrap_err();

    assert_eq!(error.code(), "tun_core_did_not_start");
    fs::remove_dir_all(&directory).unwrap();
}

/// A port nothing is listening on, released before the Core claims it.
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}
