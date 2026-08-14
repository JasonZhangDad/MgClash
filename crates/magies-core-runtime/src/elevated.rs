//! Running a Core with the privileges a TUN device needs.
//!
//! macOS creates a `utun` through a socket the caller must be root to open —
//! verified against official sing-box 1.13.18, which reports
//! `configure tun interface: Connect: operation not permitted` as a plain user
//! and `inbound/tun[tun-in]: started at utun4` under `sudo`. No code signing
//! and no Network Extension entitlement is involved.
//!
//! An elevated Core cannot be a child process the app owns: the authorization
//! prompt runs the Core under its own privileged shell, so the app tracks it by
//! PID file and stops it by signalling that PID.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use thiserror::Error;

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

/// A Core running under privileges the app itself does not have.
pub struct ElevatedCore<L: ElevationLauncher> {
    launcher: L,
    pid_file: PathBuf,
    log_file: PathBuf,
    pid: Option<u32>,
}

impl<L: ElevationLauncher> ElevatedCore<L> {
    pub fn new(launcher: L, pid_file: impl Into<PathBuf>, log_file: impl Into<PathBuf>) -> Self {
        Self {
            launcher,
            pid_file: pid_file.into(),
            log_file: log_file.into(),
            pid: None,
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
    /// # Errors
    ///
    /// Returns a typed error when the prompt is declined, the shell fails, or
    /// no PID appears — the last means the Core died immediately, and its log
    /// says why.
    pub fn start(&mut self, binary: &Path, config: &Path) -> Result<u32, ElevatedCoreError> {
        // A PID left by an earlier run would otherwise be read back as this one.
        let _ = fs::remove_file(&self.pid_file);
        self.launcher.launch(&elevation_script(
            binary,
            config,
            &self.pid_file,
            &self.log_file,
        ))?;
        let pid = self.read_pid()?;
        self.pid = Some(pid);
        Ok(pid)
    }

    /// Whether the Core is still running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.pid.is_some_and(process_is_alive)
    }

    /// Stops the Core, asking for privileges again when the app cannot signal
    /// it directly.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the second authorization fails.
    pub fn stop(&mut self) -> Result<(), ElevatedCoreError> {
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
        self.launcher.launch(&format!("kill {pid}"))?;
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
fn process_is_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .output()
        .is_ok_and(|output| output.status.success())
}

fn signal_terminate(pid: u32) -> bool {
    Command::new("kill")
        .arg(pid.to_string())
        .output()
        .is_ok_and(|output| output.status.success())
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
}

impl ElevatedCoreError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::AuthorizationDeclined => "tun_authorization_declined",
            Self::LaunchRejected { .. } | Self::LaunchFailed { .. } => "tun_elevation_failed",
            Self::PidUnreadable { .. } | Self::PidMalformed { .. } => "tun_core_did_not_start",
        }
    }
}
