//! An opaque fingerprint of the host's current default network path.
//!
//! This is the cross-platform stand-in for `NWPathMonitor`, which is macOS-only
//! and needs FFI this workspace forbids. Each OS reads its default route with a
//! read-only command; the caller only compares consecutive fingerprints, so the
//! exact text never has to be parsed. Wi-Fi to Ethernet, a hotspot switch, or a
//! DHCP lease change all move the default route and therefore the fingerprint.

use std::io;
use std::process::Command;

/// A read-only command that prints the host's default route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PathCommand {
    pub program: &'static str,
    pub arguments: &'static [&'static str],
}

/// The command this build uses to read the default route.
#[must_use]
pub const fn host_path_command() -> PathCommand {
    #[cfg(target_os = "macos")]
    {
        PathCommand {
            program: "route",
            arguments: &["-n", "get", "default"],
        }
    }
    #[cfg(target_os = "windows")]
    {
        PathCommand {
            program: "route",
            arguments: &["print", "-4", "0.0.0.0"],
        }
    }
    #[cfg(target_os = "linux")]
    {
        PathCommand {
            program: "ip",
            arguments: &["route", "show", "default"],
        }
    }
}

/// Runs a [`PathCommand`] and returns its exit code and stdout.
pub trait NetworkPathProbe: Send + Sync {
    /// # Errors
    ///
    /// Returns the OS error when the command could not be run at all.
    fn read(&self, command: &PathCommand) -> io::Result<(Option<i32>, String)>;
}

struct ProcessPathProbe;

impl NetworkPathProbe for ProcessPathProbe {
    fn read(&self, command: &PathCommand) -> io::Result<(Option<i32>, String)> {
        let output = Command::new(command.program)
            .args(command.arguments)
            .output()?;
        Ok((
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
        ))
    }
}

/// Produces the fingerprint the session watcher compares between ticks.
pub struct NetworkPathReader {
    command: PathCommand,
    probe: Box<dyn NetworkPathProbe>,
}

impl NetworkPathReader {
    /// Reads the host's default route with the real command runner.
    #[must_use]
    pub fn for_host() -> Self {
        Self {
            command: host_path_command(),
            probe: Box::new(ProcessPathProbe),
        }
    }

    #[must_use]
    pub fn with_probe(probe: impl NetworkPathProbe + 'static) -> Self {
        Self {
            command: host_path_command(),
            probe: Box::new(probe),
        }
    }

    /// The current path fingerprint, or `None` when it could not be read.
    ///
    /// A failed read is deliberately not an error value: the caller treats it
    /// as "no new information" so a transient failure never looks like a path
    /// change and never restarts a healthy Core.
    #[must_use]
    pub fn fingerprint(&self) -> Option<String> {
        let (status, stdout) = self.probe.read(&self.command).ok()?;
        if status != Some(0) {
            return None;
        }
        Some(normalize(&stdout))
    }
}

/// Collapses whitespace so cosmetic formatting differences between runs of the
/// same command never register as a path change.
fn normalize(stdout: &str) -> String {
    stdout.split_whitespace().collect::<Vec<_>>().join(" ")
}
