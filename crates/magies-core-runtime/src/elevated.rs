//! Running a Core with the privileges a TUN device needs.
//!
//! macOS creates a `utun` through a socket the caller must be root to open —
//! verified against official sing-box 1.13.18, which reports
//! `configure tun interface: Connect: operation not permitted` as a plain user
//! and `inbound/tun[tun-in]: started at utun4` under `sudo`. No code signing
//! and no Network Extension entitlement is involved.
//!
//! Linux wants the same thing for a different reason — the device needs
//! `CAP_NET_ADMIN` — and Windows needs an administrator for Wintun. All three
//! ask the desktop's own prompt (`osascript`, polkit, UAC) rather than
//! collecting a password themselves.
//!
//! An elevated Core cannot be a child process the app owns: the prompt runs the
//! Core under its own privileged shell, so the app tracks it by PID file and
//! stops it by asking the OS about that PID.

use std::fs::{self, File};
use std::io::{self, Read};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::health::CoreHealth;
use crate::output::{self, CoreOutput, CoreOutputStream};

/// The shell the elevation prompt runs, and what it leaves behind for polling.
///
/// Written as one line because the authorization prompt shows it to the user:
/// a script they cannot read is a script they should not approve.
#[must_use]
pub fn elevation_script(binary: &Path, config: &Path, pid_file: &Path, log_file: &Path) -> String {
    format!(
        "{} run -c {} >> {} 2>&1 & echo $! > {}",
        shell_quote(binary),
        shell_quote(config),
        shell_quote(log_file),
        shell_quote(pid_file),
    )
}

/// Quotes one path for `sh`, which is what both the prompt and the tests run.
fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', r"'\''"))
}

/// How the elevated shell is launched.
///
/// Production asks macOS for authorization; the tests run the same script
/// through a plain shell, so the lifecycle around it is exercised for real and
/// only the privilege escalation is left unproven.
pub trait ElevationLauncher {
    /// Runs `script` with administrator privileges, returning once it is
    /// launched — not once the Core exits.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the prompt is refused or the shell fails.
    fn launch(&self, script: &str) -> Result<(), ElevatedCoreError>;
}

/// The macOS authorization prompt.
pub struct OsascriptLauncher;

#[cfg(unix)]
impl ElevationLauncher for OsascriptLauncher {
    fn launch(&self, script: &str) -> Result<(), ElevatedCoreError> {
        // The inner script is embedded in AppleScript source, so its quotes and
        // backslashes have to survive one more round of escaping.
        let applescript = format!(
            "do shell script \"{}\" with administrator privileges",
            script.replace('\\', r"\\").replace('"', "\\\"")
        );
        let output = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(&applescript)
            .output()
            .map_err(|source| ElevatedCoreError::LaunchFailed { source })?;
        if output.status.success() {
            return Ok(());
        }
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if message.contains("-128") {
            // What macOS reports when the user dismisses the prompt.
            ElevatedCoreError::AuthorizationDeclined
        } else {
            ElevatedCoreError::LaunchRejected { message }
        })
    }
}

/// What the UAC prompt runs, as one PowerShell line.
///
/// Windows has no equivalent of the Unix script above: the Core is started by
/// `Start-Process`, which reports the PID the app then tracks. Standard error
/// is the stream sing-box logs to, so that is the file the log panel follows;
/// standard output goes beside it, because `Start-Process` refuses to point
/// both redirections at one file.
#[must_use]
pub fn windows_elevation_script(
    binary: &Path,
    config: &Path,
    pid_file: &Path,
    log_file: &Path,
) -> String {
    let mut standard_output = log_file.as_os_str().to_owned();
    standard_output.push(".out");
    format!(
        "$p = Start-Process -FilePath {} -ArgumentList 'run','-c',{} -RedirectStandardError {} -RedirectStandardOutput {} -WindowStyle Hidden -PassThru; $p.Id | Out-File -Encoding ascii {}",
        powershell_quote(binary),
        powershell_quote(config),
        powershell_quote(log_file),
        powershell_quote(Path::new(&standard_output)),
        powershell_quote(pid_file),
    )
}

/// Quotes one path for PowerShell, which escapes a single quote by doubling it.
fn powershell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

/// What `taskkill` is asked to do.
///
/// Asked politely first: a forced kill skips the Core's own shutdown, and on
/// Windows that shutdown is what takes the TUN routes back out.
#[must_use]
pub fn taskkill_arguments(pid: u32, force: bool) -> Vec<String> {
    let mut arguments = vec!["/PID".to_owned(), pid.to_string(), "/T".to_owned()];
    if force {
        arguments.push("/F".to_owned());
    }
    arguments
}

/// What `tasklist` is asked, to learn whether a PID is still a process.
#[must_use]
pub fn tasklist_arguments(pid: u32) -> Vec<String> {
    vec![
        "/FI".to_owned(),
        format!("PID eq {pid}"),
        "/NH".to_owned(),
        "/FO".to_owned(),
        "CSV".to_owned(),
    ]
}

/// Whether `tasklist` answered with the process rather than with its
/// "no tasks" sentence.
///
/// The PID is read out of its own CSV column: a substring search would also
/// match a memory figure that happens to contain the digits.
#[must_use]
pub fn parse_tasklist_output(output: &str, pid: u32) -> bool {
    output.lines().any(|line| {
        line.split(',')
            .nth(1)
            .is_some_and(|column| column.trim().trim_matches('"').parse() == Ok(pid))
    })
}

/// Maps what PowerShell says about a refused elevation.
#[must_use]
pub fn runas_error(message: &str) -> ElevatedCoreError {
    if message.contains("canceled by the user") || message.contains("cancelled by the user") {
        ElevatedCoreError::AuthorizationDeclined
    } else {
        ElevatedCoreError::LaunchRejected {
            message: message.trim().to_owned(),
        }
    }
}

/// The UAC prompt.
///
/// The script is written to a file and run by an elevated PowerShell rather
/// than passed as a nested command line: UAC shows the user a program, not a
/// command, so a script they can open and read is the only readable form of
/// what they are approving.
#[cfg(windows)]
pub struct RunAsLauncher {
    script_file: PathBuf,
}

#[cfg(windows)]
impl RunAsLauncher {
    #[must_use]
    pub fn new(script_file: impl Into<PathBuf>) -> Self {
        Self {
            script_file: script_file.into(),
        }
    }
}

#[cfg(windows)]
impl ElevationLauncher for RunAsLauncher {
    fn launch(&self, script: &str) -> Result<(), ElevatedCoreError> {
        fs::write(&self.script_file, script).map_err(|source| {
            ElevatedCoreError::LogUnreadable {
                path: self.script_file.clone(),
                source,
            }
        })?;
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!(
                    "Start-Process -Verb RunAs -WindowStyle Hidden -Wait -FilePath 'powershell' \
-ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',{}",
                    powershell_quote(&self.script_file)
                ),
            ])
            .output()
            .map_err(|source| ElevatedCoreError::LaunchFailed { source })?;
        if output.status.success() {
            return Ok(());
        }
        Err(runas_error(&String::from_utf8_lossy(&output.stderr)))
    }
}

/// The polkit prompt, which is how a Linux desktop asks.
///
/// The password is typed into the desktop's own authentication agent, never
/// into this app — the same reason macOS goes through `osascript` rather than
/// collecting a password and piping it to `sudo`.
pub struct PkexecLauncher;

/// What `pkexec` is asked to run.
///
/// It takes a program and arguments rather than a shell line, so the shell is
/// named explicitly and by absolute path: `pkexec` will not resolve a program
/// against a `PATH` it does not trust.
#[must_use]
pub fn pkexec_arguments(script: &str) -> Vec<String> {
    vec!["/bin/sh".to_owned(), "-c".to_owned(), script.to_owned()]
}

/// Maps a `pkexec` exit into the same refusals every platform reports.
#[must_use]
pub fn pkexec_error(code: Option<i32>, message: &str) -> ElevatedCoreError {
    match code {
        // The user dismissed the authentication dialog.
        Some(126) => ElevatedCoreError::AuthorizationDeclined,
        _ => ElevatedCoreError::LaunchRejected {
            message: message.trim().to_owned(),
        },
    }
}

#[cfg(unix)]
impl ElevationLauncher for PkexecLauncher {
    fn launch(&self, script: &str) -> Result<(), ElevatedCoreError> {
        let output = Command::new("pkexec")
            .args(pkexec_arguments(script))
            .output()
            .map_err(|source| ElevatedCoreError::LaunchFailed { source })?;
        if output.status.success() {
            return Ok(());
        }
        Err(pkexec_error(
            output.status.code(),
            &String::from_utf8_lossy(&output.stderr),
        ))
    }
}

/// A Core running under privileges the app itself does not have.
pub struct ElevatedCore<L: ElevationLauncher> {
    launcher: L,
    pid_file: PathBuf,
    log_file: PathBuf,
    pid: Option<u32>,
    /// Ends the log reader. Shared with the reader thread, which has no other
    /// way to learn that the Core it follows is gone.
    stopped: Arc<AtomicBool>,
}

impl<L: ElevationLauncher> ElevatedCore<L> {
    pub fn new(launcher: L, pid_file: impl Into<PathBuf>, log_file: impl Into<PathBuf>) -> Self {
        Self {
            launcher,
            pid_file: pid_file.into(),
            log_file: log_file.into(),
            pid: None,
            stopped: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The PID the elevated shell reported, once it has started.
    #[must_use]
    pub const fn pid(&self) -> Option<u32> {
        self.pid
    }

    #[must_use]
    pub fn log_file(&self) -> &Path {
        &self.log_file
    }

    /// Asks for privileges and starts the Core.
    ///
    /// Returns the PID and the Core's output. An elevated Core writes to a log
    /// file rather than to a pipe the app owns, so the output is that file
    /// followed as it grows — the log panel cannot tell the difference.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the prompt is declined, the shell fails, the
    /// log cannot be opened, or no PID appears — the last means the Core died
    /// immediately, and its log says why.
    pub fn start(
        &mut self,
        binary: &Path,
        config: &Path,
    ) -> Result<(u32, CoreOutput), ElevatedCoreError> {
        // Whatever is running now is about to become untrackable: the PID file
        // is the only record, and this start overwrites it.
        self.stop()?;
        // A PID left by an earlier run would otherwise be read back as this one.
        let _ = fs::remove_file(&self.pid_file);
        // A restart gets its own flag: raising the old one ends the previous
        // reader, and reusing it would end the new one before it read a line.
        self.stopped.store(true, Ordering::Relaxed);
        self.stopped = Arc::new(AtomicBool::new(false));
        // Truncated before the shell appends to it, so the panel shows this
        // session rather than replaying the last one.
        let tail = LogTail::open(&self.log_file, Arc::clone(&self.stopped))?;
        #[cfg(unix)]
        let script = elevation_script(binary, config, &self.pid_file, &self.log_file);
        #[cfg(windows)]
        let script = windows_elevation_script(binary, config, &self.pid_file, &self.log_file);
        self.launcher.launch(&script)?;
        let pid = self.read_pid()?;
        self.pid = Some(pid);
        let (sender, output) = output::output_channel();
        // Merged: the script redirects stderr into the same log, so there is
        // one stream to report and stdout is the honest name for it.
        drop(
            output::spawn_output_reader(CoreOutputStream::Stdout, tail, sender).map_err(
                |source| ElevatedCoreError::LogUnreadable {
                    path: self.log_file.clone(),
                    source,
                },
            )?,
        );
        Ok((pid, output))
    }

    /// Adopts a Core left behind by an app that did not stop it.
    ///
    /// The PID file is the only record of a Core the app does not own, so a
    /// crash leaves a root process still holding the TUN device. Returns the
    /// PID it adopted, or `None` — a file naming a process that is gone, or
    /// naming no process at all, is removed rather than trusted.
    pub fn reclaim(&mut self) -> Option<u32> {
        let pid = self
            .read_pid()
            .ok()
            .filter(|pid| process_is_alive(*pid))
            .inspect(|pid| self.pid = Some(*pid));
        if pid.is_none() {
            let _ = fs::remove_file(&self.pid_file);
        }
        pid
    }

    /// Whether the Core is still running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.pid.is_some_and(process_is_alive)
    }

    /// Waits until the elevated Core accepts a TCP connection at `address`.
    ///
    /// Deliberately not [`crate::CoreRuntime::wait_for_tcp_health`]: that one learns
    /// the Core died by reaping a child process, and an elevated Core is not
    /// ours to reap. Liveness here is the PID still existing.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the Core exits before the port opens, or
    /// when it never opens before `timeout`.
    pub fn wait_for_tcp_health(
        &self,
        address: SocketAddr,
        timeout: Duration,
    ) -> Result<CoreHealth, ElevatedCoreError> {
        let started_at = Instant::now();
        loop {
            if !self.is_running() {
                return Err(ElevatedCoreError::ExitedBeforeReady);
            }
            let remaining = timeout.saturating_sub(started_at.elapsed());
            if remaining.is_zero() {
                return Err(ElevatedCoreError::HealthTimedOut { address, timeout });
            }
            if TcpStream::connect_timeout(&address, HEALTH_ATTEMPT.min(remaining)).is_ok() {
                return Ok(CoreHealth {
                    ready_after: started_at.elapsed(),
                });
            }
            sleep(HEALTH_RETRY.min(timeout.saturating_sub(started_at.elapsed())));
        }
    }

    /// Stops the Core, asking for privileges again when the app cannot signal
    /// it directly.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the second authorization fails.
    pub fn stop(&mut self) -> Result<(), ElevatedCoreError> {
        self.stopped.store(true, Ordering::Relaxed);
        let Some(pid) = self.pid.take() else {
            return Ok(());
        };
        if !process_is_alive(pid) {
            let _ = fs::remove_file(&self.pid_file);
            return Ok(());
        }
        // Tried unprivileged first: a Core the app started itself, or one whose
        // privileges were dropped, needs no second prompt. A signalled process
        // takes a moment to go, and returning before it has gone would be a
        // stop that did not stop.
        if signal_terminate(pid) && wait_for_exit(pid) {
            let _ = fs::remove_file(&self.pid_file);
            return Ok(());
        }
        self.launcher.launch(&stop_script(pid))?;
        let _ = fs::remove_file(&self.pid_file);
        Ok(())
    }

    fn read_pid(&self) -> Result<u32, ElevatedCoreError> {
        let text = fs::read_to_string(&self.pid_file).map_err(|source| {
            ElevatedCoreError::PidUnreadable {
                path: self.pid_file.clone(),
                source,
            }
        })?;
        text.trim()
            .parse()
            .map_err(|_| ElevatedCoreError::PidMalformed {
                value: text.trim().to_owned(),
            })
    }
}

impl<L: ElevationLauncher> Drop for ElevatedCore<L> {
    fn drop(&mut self) {
        // Without this a caller that drops the Core instead of stopping it
        // leaves the reader thread parked on a log nobody writes to.
        self.stopped.store(true, Ordering::Relaxed);
    }
}

/// The Core's log, read as it grows.
///
/// A plain file reader would report end-of-file the moment it caught up with
/// the Core, which for a running Core is not the end of anything.
struct LogTail {
    file: File,
    stopped: Arc<AtomicBool>,
}

/// How long the reader waits before looking for more log.
const TAIL_POLL: Duration = Duration::from_millis(100);

impl LogTail {
    fn open(path: &Path, stopped: Arc<AtomicBool>) -> Result<Self, ElevatedCoreError> {
        // Created and truncated here rather than by the elevated shell: the
        // reader needs the file to exist, and last session's log is not this
        // session's output.
        File::create(path).map_err(|source| ElevatedCoreError::LogUnreadable {
            path: path.to_path_buf(),
            source,
        })?;
        let file = File::open(path).map_err(|source| ElevatedCoreError::LogUnreadable {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Self { file, stopped })
    }
}

impl Read for LogTail {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            let length = self.file.read(buffer)?;
            if length > 0 {
                return Ok(length);
            }
            if self.stopped.load(Ordering::Relaxed) {
                // The only real end-of-file: the Core is gone, so whatever is
                // in the log is all there will be.
                return Ok(0);
            }
            sleep(TAIL_POLL);
        }
    }
}

/// How long one connection attempt is given, and how long before the next.
const HEALTH_ATTEMPT: Duration = Duration::from_millis(25);
const HEALTH_RETRY: Duration = Duration::from_millis(20);

/// How long a signalled process is given to go before privileges are asked for.
const EXIT_GRACE: Duration = Duration::from_secs(2);

/// Waits for a signalled process to actually exit.
fn wait_for_exit(pid: u32) -> bool {
    let deadline = Instant::now() + EXIT_GRACE;
    while Instant::now() < deadline {
        if !process_is_alive(pid) {
            return true;
        }
        sleep(Duration::from_millis(20));
    }
    false
}

/// Whether a PID names a live process, without disturbing it.
#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .output()
        .is_ok_and(|output| output.status.success())
}

#[cfg(windows)]
fn process_is_alive(pid: u32) -> bool {
    Command::new("tasklist")
        .args(tasklist_arguments(pid))
        .output()
        .is_ok_and(|output| parse_tasklist_output(&String::from_utf8_lossy(&output.stdout), pid))
}

/// Asks the Core to stop, without forcing it.
#[cfg(unix)]
fn signal_terminate(pid: u32) -> bool {
    Command::new("kill")
        .arg(pid.to_string())
        .output()
        .is_ok_and(|output| output.status.success())
}

#[cfg(windows)]
fn signal_terminate(pid: u32) -> bool {
    Command::new("taskkill")
        .args(taskkill_arguments(pid, false))
        .output()
        .is_ok_and(|output| output.status.success())
}

/// What the app runs to end a Core it could not stop unprivileged.
///
/// The elevated process belongs to another user, so this is handed back to the
/// prompt rather than run directly.
#[cfg(unix)]
fn stop_script(pid: u32) -> String {
    format!("kill {pid}")
}

#[cfg(windows)]
fn stop_script(pid: u32) -> String {
    // Forced here: this is the second attempt, after the polite one either
    // failed outright or was refused for want of privileges.
    format!("taskkill {}", taskkill_arguments(pid, true).join(" "))
}

#[derive(Debug, Error)]
pub enum ElevatedCoreError {
    #[error("the administrator prompt was dismissed")]
    AuthorizationDeclined,
    #[error("the elevated shell refused to run: {message}")]
    LaunchRejected { message: String },
    #[error("the elevated shell could not be started")]
    LaunchFailed { source: std::io::Error },
    #[error("the elevated Core wrote no process id to {}: {source}", path.display())]
    PidUnreadable {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("the elevated Core wrote {value:?} instead of a process id")]
    PidMalformed { value: String },
    #[error("the elevated Core exited before it was ready")]
    ExitedBeforeReady,
    #[error("the elevated Core did not open {address} within {timeout:?}")]
    HealthTimedOut {
        address: SocketAddr,
        timeout: Duration,
    },
    #[error("the elevated Core log at {} could not be read: {source}", path.display())]
    LogUnreadable {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl ElevatedCoreError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::AuthorizationDeclined => "tun_authorization_declined",
            Self::LaunchRejected { .. } | Self::LaunchFailed { .. } => "tun_elevation_failed",
            Self::PidUnreadable { .. } | Self::PidMalformed { .. } | Self::ExitedBeforeReady => {
                "tun_core_did_not_start"
            }
            Self::HealthTimedOut { .. } => "tun_core_unhealthy",
            Self::LogUnreadable { .. } => "tun_core_log_unreadable",
        }
    }
}
