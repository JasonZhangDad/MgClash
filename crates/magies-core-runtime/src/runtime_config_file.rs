use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use thiserror::Error;

static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AtomicRuntimeConfig;

impl AtomicRuntimeConfig {
    /// Writes a private temporary file and atomically publishes it without
    /// overwriting an existing runtime configuration.
    ///
    /// # Errors
    ///
    /// Returns a typed file-system error. Empty configurations are rejected
    /// before the file system is changed.
    pub fn write(
        configured_path: impl AsRef<Path>,
        contents: &[u8],
    ) -> Result<RuntimeConfigFile, RuntimeConfigFileError> {
        if contents.is_empty() {
            return Err(RuntimeConfigFileError::EmptyConfig);
        }

        let path = configured_path.as_ref();
        let directory = parent_directory(path);
        fs::create_dir_all(directory).map_err(|source| {
            RuntimeConfigFileError::CreateDirectory {
                path: directory.to_path_buf(),
                source,
            }
        })?;
        let mut temporary = TemporaryConfigFile::create(directory)?;
        restrict_permissions(&temporary.file, &temporary.path)?;
        temporary
            .file
            .write_all(contents)
            .map_err(|source| RuntimeConfigFileError::Write { source })?;
        temporary
            .file
            .sync_all()
            .map_err(|source| RuntimeConfigFileError::Sync { source })?;

        match fs::hard_link(&temporary.path, path) {
            Ok(()) => Ok(RuntimeConfigFile {
                path: path.to_path_buf(),
            }),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                Err(RuntimeConfigFileError::AlreadyExists {
                    path: path.to_path_buf(),
                })
            }
            Err(source) => Err(RuntimeConfigFileError::Publish {
                path: path.to_path_buf(),
                source,
            }),
        }
    }
}

struct TemporaryConfigFile {
    file: File,
    path: PathBuf,
}

impl TemporaryConfigFile {
    fn create(directory: &Path) -> Result<Self, RuntimeConfigFileError> {
        loop {
            let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = directory.join(format!(
                ".mgclash-runtime-{}-{sequence}.tmp",
                std::process::id()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => return Ok(Self { file, path }),
                Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
                Err(source) => {
                    return Err(RuntimeConfigFileError::CreateTemporary { path, source });
                }
            }
        }
    }
}

impl Drop for TemporaryConfigFile {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!(
                "failed to remove temporary runtime configuration {}: {error}",
                self.path.display()
            );
        }
    }
}

fn parent_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

#[cfg(unix)]
fn restrict_permissions(file: &File, path: &Path) -> Result<(), RuntimeConfigFileError> {
    use std::os::unix::fs::PermissionsExt;

    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|source| RuntimeConfigFileError::SetPermissions {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(not(unix))]
fn restrict_permissions(_file: &File, _path: &Path) -> Result<(), RuntimeConfigFileError> {
    Ok(())
}

pub struct RuntimeConfigFile {
    path: PathBuf,
}

impl RuntimeConfigFile {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Removes the runtime configuration immediately.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the file cannot be removed.
    pub fn remove(mut self) -> Result<(), RuntimeConfigFileError> {
        let path = std::mem::take(&mut self.path);
        fs::remove_file(&path).map_err(|source| RuntimeConfigFileError::Remove { path, source })
    }
}

impl std::fmt::Debug for RuntimeConfigFile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeConfigFile")
            .field("path", &self.path)
            .finish()
    }
}

impl Drop for RuntimeConfigFile {
    fn drop(&mut self) {
        let path = std::mem::take(&mut self.path);
        if path.as_os_str().is_empty() {
            return;
        }
        if let Err(error) = fs::remove_file(&path)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!(
                "failed to remove runtime configuration {} during cleanup: {error}",
                path.display()
            );
        }
    }
}

#[derive(Debug, Error)]
pub enum RuntimeConfigFileError {
    #[error("runtime configuration must not be empty")]
    EmptyConfig,
    #[error("failed to create runtime directory {path:?}: {source}")]
    CreateDirectory { path: PathBuf, source: io::Error },
    #[error("failed to create temporary runtime configuration in {path:?}: {source}")]
    CreateTemporary { path: PathBuf, source: io::Error },
    #[error("failed to restrict temporary runtime configuration {path:?}: {source}")]
    SetPermissions { path: PathBuf, source: io::Error },
    #[error("failed to write runtime configuration: {source}")]
    Write { source: io::Error },
    #[error("failed to sync runtime configuration: {source}")]
    Sync { source: io::Error },
    #[error("runtime configuration already exists: {path:?}")]
    AlreadyExists { path: PathBuf },
    #[error("failed to publish runtime configuration {path:?}: {source}")]
    Publish { path: PathBuf, source: io::Error },
    #[error("failed to remove runtime configuration {path:?}: {source}")]
    Remove { path: PathBuf, source: io::Error },
}
