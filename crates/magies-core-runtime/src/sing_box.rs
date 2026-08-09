use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Output;

use crate::adapter::{CoreConfigPathError, execute_core_command, resolve_config_path};
use crate::{CoreProcessSpec, ValidatedCoreBinary};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SingBoxOperation {
    ReadVersion,
    ValidateConfig,
}

impl std::fmt::Display for SingBoxOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadVersion => formatter.write_str("read sing-box version"),
            Self::ValidateConfig => formatter.write_str("validate sing-box config"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SingBoxAdapterError {
    #[error("sing-box config not found: {path:?}")]
    ConfigNotFound { path: PathBuf },
    #[error("failed to resolve sing-box config {path:?}: {source}")]
    ConfigResolveFailed { path: PathBuf, source: io::Error },
    #[error("sing-box config is not a file: {path:?}")]
    ConfigNotAFile { path: PathBuf },
    #[error("failed to {operation}: {source}")]
    CommandStartFailed {
        operation: SingBoxOperation,
        source: io::Error,
    },
    #[error("failed to {operation} (exit code {status:?}): {stderr}")]
    CommandFailed {
        operation: SingBoxOperation,
        status: Option<i32>,
        stderr: String,
    },
    #[error("invalid sing-box version output: {stdout:?}")]
    InvalidVersionOutput { stdout: String },
}

impl From<CoreConfigPathError> for SingBoxAdapterError {
    fn from(error: CoreConfigPathError) -> Self {
        match error {
            CoreConfigPathError::NotFound { path } => Self::ConfigNotFound { path },
            CoreConfigPathError::ResolveFailed { path, source } => {
                Self::ConfigResolveFailed { path, source }
            }
            CoreConfigPathError::NotAFile { path } => Self::ConfigNotAFile { path },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SingBoxVersion(String);

impl SingBoxVersion {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedSingBoxConfig(PathBuf);

impl ValidatedSingBoxConfig {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SingBoxAdapter {
    binary: ValidatedCoreBinary,
}

impl SingBoxAdapter {
    #[must_use]
    pub const fn new(binary: ValidatedCoreBinary) -> Self {
        Self { binary }
    }

    /// Reads and parses the version reported by the validated sing-box binary.
    ///
    /// # Errors
    ///
    /// Returns a typed error when sing-box cannot be started, exits
    /// unsuccessfully, or does not report a sing-box version.
    pub fn version(&self) -> Result<SingBoxVersion, SingBoxAdapterError> {
        let output = self.execute(SingBoxOperation::ReadVersion, ["version"])?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let version = stdout
            .lines()
            .find_map(|line| line.strip_prefix("sing-box version "))
            .and_then(|line| line.split_whitespace().next())
            .filter(|version| !version.is_empty())
            .ok_or_else(|| SingBoxAdapterError::InvalidVersionOutput {
                stdout: stdout.clone(),
            })?;
        Ok(SingBoxVersion(version.to_owned()))
    }

    /// Resolves a config file and asks sing-box to validate it without starting.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the path is invalid, sing-box cannot be
    /// started, or sing-box rejects the config.
    pub fn validate_config(
        &self,
        configured_path: impl AsRef<Path>,
    ) -> Result<ValidatedSingBoxConfig, SingBoxAdapterError> {
        let path = resolve_config_path(configured_path)?;
        self.execute(
            SingBoxOperation::ValidateConfig,
            [
                OsString::from("check"),
                OsString::from("-c"),
                path.as_os_str().to_owned(),
            ],
        )?;
        Ok(ValidatedSingBoxConfig(path))
    }

    #[must_use]
    pub fn process_spec(&self, config: &ValidatedSingBoxConfig) -> CoreProcessSpec {
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
        operation: SingBoxOperation,
        arguments: I,
    ) -> Result<Output, SingBoxAdapterError>
    where
        I: IntoIterator<Item = A>,
        A: AsRef<std::ffi::OsStr>,
    {
        let output = execute_core_command(&self.binary, arguments)
            .map_err(|source| SingBoxAdapterError::CommandStartFailed { operation, source })?;
        if !output.status.success() {
            return Err(SingBoxAdapterError::CommandFailed {
                operation,
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(output)
    }
}
