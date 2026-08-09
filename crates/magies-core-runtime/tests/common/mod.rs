use std::fs::{copy, create_dir, read, remove_dir, remove_file, write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use magies_core_runtime::{
    CoreBinaryRequirement, Sha256Hash, ValidatedCoreBinary, locate_core_binary,
};
use magies_platform::CpuArchitecture;

static TEMPORARY_PATH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn compile_fixture(source_name: &str, output_name: &str) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(source_name);
    let output = unique_temporary_path(&format!("{output_name}{}", std::env::consts::EXE_SUFFIX));
    let status = Command::new("rustc")
        .args(["--edition=2024"])
        .arg(source)
        .arg("-o")
        .arg(&output)
        .status()
        .expect("rustc must be available to compile a fake Core fixture");
    assert!(status.success(), "fake Core fixture must compile");
    output
}

pub fn validated_binary(path: &Path) -> ValidatedCoreBinary {
    let contents = read(path).expect("the fake Core executable must be readable");
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

pub fn unique_temporary_path(name: &str) -> PathBuf {
    let sequence = TEMPORARY_PATH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "magies-core-adapter-{}-{sequence}-{name}",
        std::process::id()
    ))
}

pub struct TemporaryPath {
    path: PathBuf,
    is_directory: bool,
}

impl TemporaryPath {
    pub fn file(name: &str, contents: &[u8]) -> Self {
        let path = unique_temporary_path(name);
        write(&path, contents).unwrap();
        Self {
            path,
            is_directory: false,
        }
    }

    pub fn copy_of(name: &str, source: &Path) -> Self {
        let path = unique_temporary_path(&format!("{name}{}", std::env::consts::EXE_SUFFIX));
        copy(source, &path).unwrap();
        Self {
            path,
            is_directory: false,
        }
    }

    pub fn directory(name: &str) -> Self {
        let path = unique_temporary_path(name);
        create_dir(&path).unwrap();
        Self {
            path,
            is_directory: true,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn remove(mut self) {
        remove_file(&self.path).unwrap();
        self.path = PathBuf::new();
    }
}

impl Drop for TemporaryPath {
    fn drop(&mut self) {
        if self.path.as_os_str().is_empty() {
            return;
        }
        let result = if self.is_directory {
            remove_dir(&self.path)
        } else {
            remove_file(&self.path)
        };
        if let Err(error) = result {
            eprintln!(
                "failed to remove test path {}: {error}",
                self.path.display()
            );
        }
    }
}
