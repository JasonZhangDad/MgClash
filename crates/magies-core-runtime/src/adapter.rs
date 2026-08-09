use std::fs::canonicalize;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::ValidatedCoreBinary;

pub(crate) enum CoreConfigPathError {
    NotFound { path: PathBuf },
    ResolveFailed { path: PathBuf, source: io::Error },
    NotAFile { path: PathBuf },
}

pub(crate) fn resolve_config_path(
    configured_path: impl AsRef<Path>,
) -> Result<PathBuf, CoreConfigPathError> {
    let configured_path = configured_path.as_ref();
    let path = canonicalize(configured_path).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            CoreConfigPathError::NotFound {
                path: configured_path.to_path_buf(),
            }
        } else {
            CoreConfigPathError::ResolveFailed {
                path: configured_path.to_path_buf(),
                source,
            }
        }
    })?;
    if !path.is_file() {
        return Err(CoreConfigPathError::NotAFile { path });
    }
    Ok(path)
}

pub(crate) fn execute_core_command<I, A>(
    binary: &ValidatedCoreBinary,
    arguments: I,
) -> io::Result<Output>
where
    I: IntoIterator<Item = A>,
    A: AsRef<std::ffi::OsStr>,
{
    Command::new(binary.path()).args(arguments).output()
}
