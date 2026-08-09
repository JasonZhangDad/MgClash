use std::ffi::OsString;
use std::fs::canonicalize;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::{CoreProcessSpec, ValidatedCoreBinary};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XrayOperation {
    ReadVersion,
    ValidateConfig,
}

impl std::fmt::Display for XrayOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadVersion => formatter.write_str("read Xray version"),
            Self::ValidateConfig => formatter.write_str("validate Xray config"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum XrayAdapterError {
    #[error("Xray config not found: {path:?}")]
    ConfigNotFound { path: PathBuf },
    #[error("failed to resolve Xray config {path:?}: {source}")]
    ConfigResolveFailed { path: PathBuf, source: io::Error },
    #[error("Xray config is not a file: {path:?}")]
    ConfigNotAFile { path: PathBuf },
    #[error("failed to {operation}: {source}")]
    CommandStartFailed {
        operation: XrayOperation,
        source: io::Error,
    },
    #[error("failed to {operation} (exit code {status:?}): {stderr}")]
    CommandFailed {
        operation: XrayOperation,
        status: Option<i32>,
        stderr: String,
    },
    #[error("invalid Xray version output: {stdout:?}")]
    InvalidVersionOutput { stdout: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XrayVersion(String);

impl XrayVersion {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedXrayConfig(PathBuf);

impl ValidatedXrayConfig {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XrayAdapter {
    binary: ValidatedCoreBinary,
}

impl XrayAdapter {
    #[must_use]
    pub const fn new(binary: ValidatedCoreBinary) -> Self {
        Self { binary }
    }

    /// Reads and parses the version reported by the validated Xray binary.
    ///
    /// # Errors
    ///
    /// Returns a typed error when Xray cannot be started, exits unsuccessfully,
    /// or does not report an Xray version.
    pub fn version(&self) -> Result<XrayVersion, XrayAdapterError> {
        let output = self.execute(XrayOperation::ReadVersion, ["version"])?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let version = stdout
            .lines()
            .find_map(|line| line.strip_prefix("Xray "))
            .and_then(|line| line.split_whitespace().next())
            .filter(|version| !version.is_empty())
            .ok_or_else(|| XrayAdapterError::InvalidVersionOutput {
                stdout: stdout.clone(),
            })?;
        Ok(XrayVersion(version.to_owned()))
    }

    /// Resolves a config file and asks Xray to validate it without starting.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the path is invalid, Xray cannot be started,
    /// or Xray rejects the config.
    pub fn validate_config(
        &self,
        configured_path: impl AsRef<Path>,
    ) -> Result<ValidatedXrayConfig, XrayAdapterError> {
        let configured_path = configured_path.as_ref();
        let path = canonicalize(configured_path).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                XrayAdapterError::ConfigNotFound {
                    path: configured_path.to_path_buf(),
                }
            } else {
                XrayAdapterError::ConfigResolveFailed {
                    path: configured_path.to_path_buf(),
                    source,
                }
            }
        })?;
        if !path.is_file() {
            return Err(XrayAdapterError::ConfigNotAFile { path });
        }

        self.execute(
            XrayOperation::ValidateConfig,
            [
                OsString::from("run"),
                OsString::from("-test"),
                OsString::from("-c"),
                path.as_os_str().to_owned(),
            ],
        )?;
        Ok(ValidatedXrayConfig(path))
    }

    #[must_use]
    pub fn process_spec(&self, config: &ValidatedXrayConfig) -> CoreProcessSpec {
        CoreProcessSpec::new(
            &self.binary,
            [
                OsString::from("run"),
                OsString::from("-c"),
                config.path().as_os_str().to_owned(),
            ],
        )
    }

    fn execute<I, A>(
        &self,
        operation: XrayOperation,
        arguments: I,
    ) -> Result<Output, XrayAdapterError>
    where
        I: IntoIterator<Item = A>,
        A: AsRef<std::ffi::OsStr>,
    {
        let output = Command::new(self.binary.path())
            .args(arguments)
            .output()
            .map_err(|source| XrayAdapterError::CommandStartFailed { operation, source })?;
        if !output.status.success() {
            return Err(XrayAdapterError::CommandFailed {
                operation,
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(output)
    }
}
