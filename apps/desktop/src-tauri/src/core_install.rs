//! Downloads, verifies, and installs sing-box / Xray under the app data directory.
//!
//! User-installed Cores sit beside a manifest with their SHA-256 digest. The
//! desktop shell prefers them over a bundled sing-box when no environment
//! override is set, matching PRD 44.1's verify-then-atomic-replace flow.

use std::fs;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use magies_core_runtime::{
    CoreBinaryRequirement, Sha256Hash, SingBoxAdapter, SingBoxAdapterError, XrayAdapter,
    XrayAdapterError, locate_core_binary,
};
use magies_platform::{CpuArchitecture, OperatingSystem, PlatformError, TargetPlatform};
use magies_profiles::ensure_rustls_crypto_provider;
use serde::{Deserialize, Serialize};
use tar::Archive;
use thiserror::Error;
use zip::ZipArchive;

use crate::core_control::{
    CoreSettings, CoreSettingsError, XRAY_BINARY_VARIABLE, XRAY_SHA256_VARIABLE,
    BINARY_PATH_VARIABLE, SHA256_VARIABLE,
};

const SING_BOX_RELEASE_API: &str =
    "https://api.github.com/repos/SagerNet/sing-box/releases/latest";
const XRAY_RELEASE_API: &str = "https://api.github.com/repos/XTLS/Xray-core/releases/latest";
const DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);
const MAX_ARCHIVE_BYTES: usize = 150 * 1024 * 1024;

const MANIFEST_FILE: &str = "manifest.json";
const SING_BOX_BINARY: &str = "sing-box";
const XRAY_BINARY: &str = "xray";
const GEOIP_FILE: &str = "geoip.dat";
const GEOSITE_FILE: &str = "geosite.dat";
const SING_BOX_SMOKE_CONFIG: &str = r#"{
  "log": { "level": "warn" },
  "inbounds": [
    {
      "type": "mixed",
      "tag": "mixed-in",
      "listen": "127.0.0.1",
      "listen_port": 18981
    }
  ]
}"#;
const XRAY_SMOKE_CONFIG: &str = r#"{
  "log": { "loglevel": "warning" },
  "inbounds": [
    {
      "listen": "127.0.0.1",
      "port": 18980,
      "protocol": "http",
      "settings": {}
    }
  ],
  "outbounds": [
    {
      "protocol": "freedom",
      "settings": {}
    }
  ]
}"#;
const SMOKE_CONFIG_STEM: &str = "install-smoke";

/// Which Core the user asked to install.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreKind {
    SingBox,
    Xray,
}

impl CoreKind {
    /// Parses the UI / command payload.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the name is not `sing-box` or `xray`.
    pub fn parse(value: &str) -> Result<Self, CoreInstallError> {
        match value {
            "sing-box" | "singBox" => Ok(Self::SingBox),
            "xray" | "Xray" => Ok(Self::Xray),
            _ => Err(CoreInstallError::UnknownCore {
                name: value.to_owned(),
            }),
        }
    }
}

/// One installed Core recorded in the manifest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledCoreEntry {
    pub version: String,
    pub sha256: String,
    pub binary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_version: Option<String>,
}

/// Persisted install state for both Cores.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreInstallManifest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sing_box: Option<InstalledCoreEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xray: Option<InstalledCoreEntry>,
}

/// What the Core install dialog can render.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreInstallStatus {
    pub directory: String,
    pub sing_box: Option<InstalledCoreEntry>,
    pub xray: Option<InstalledCoreEntry>,
}

pub struct CoreInstallStore {
    directory: PathBuf,
}

impl CoreInstallStore {
    /// Opens the Core install directory, creating it when missing.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the directory cannot be created.
    pub fn open(directory: impl Into<PathBuf>) -> Result<Self, CoreInstallError> {
        let directory = directory.into();
        fs::create_dir_all(&directory).map_err(|source| CoreInstallError::CreateDirectory {
            path: directory.clone(),
            source,
        })?;
        Ok(Self { directory })
    }

    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Returns the on-disk manifest, if any.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the manifest exists but cannot be read.
    pub fn load_manifest(&self) -> Result<Option<CoreInstallManifest>, CoreInstallError> {
        let path = self.directory.join(MANIFEST_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        let body = fs::read_to_string(&path).map_err(|source| CoreInstallError::ReadManifest {
            path: path.clone(),
            source,
        })?;
        serde_json::from_str(&body)
            .map(Some)
            .map_err(|source| CoreInstallError::ParseManifest { source })
    }

    /// Returns install status for the UI.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the manifest cannot be read.
    pub fn status(&self) -> Result<CoreInstallStatus, CoreInstallError> {
        let manifest = self.load_manifest()?.unwrap_or_default();
        Ok(CoreInstallStatus {
            directory: self.directory.display().to_string(),
            sing_box: manifest.sing_box,
            xray: manifest.xray,
        })
    }

    /// Resolves sing-box settings from the manifest when present.
    #[must_use]
    pub fn sing_box_settings(&self) -> Option<Result<CoreSettings, CoreSettingsError>> {
        self.load_manifest()
            .ok()
            .flatten()
            .and_then(|manifest| manifest.sing_box)
            .map(|entry| entry.into_settings())
    }

    /// Resolves Xray settings from the manifest when present.
    #[must_use]
    pub fn xray_settings(&self) -> Option<Result<CoreSettings, CoreSettingsError>> {
        self.load_manifest()
            .ok()
            .flatten()
            .and_then(|manifest| manifest.xray)
            .map(|entry| entry.into_settings())
    }

    /// Downloads and installs the latest GitHub release for one Core.
    ///
    /// # Errors
    ///
    /// Returns a typed error for HTTP, checksum, extraction, or filesystem failures.
    pub async fn download_latest(
        &self,
        kind: CoreKind,
        geo_directory: Option<&Path>,
    ) -> Result<CoreInstallStatus, CoreInstallError> {
        let target = TargetPlatform::parse(std::env::consts::OS, std::env::consts::ARCH)
            .map_err(CoreInstallError::Target)?;
        match kind {
            CoreKind::SingBox => self.install_sing_box(&target).await?,
            CoreKind::Xray => self.install_xray(&target, geo_directory).await?,
        }
        self.status()
    }
}

impl InstalledCoreEntry {
    fn into_settings(self) -> Result<CoreSettings, CoreSettingsError> {
        CoreSettings::from_values(
            Some(PathBuf::from(self.binary)),
            Some(self.sha256),
        )
    }
}

impl CoreInstallStore {
    async fn install_sing_box(&self, target: &TargetPlatform) -> Result<(), CoreInstallError> {
        let release = fetch_release(SING_BOX_RELEASE_API).await?;
        let version = normalize_tag(&release.tag_name)?;
        let asset = sing_box_asset(*target, &version);
        let archive_bytes = download_named_asset(&release, &asset.archive_name).await?;
        if let Some(checksums) = download_optional_asset(&release, &asset.checksums_name).await? {
            verify_archive_checksum(&checksums, &asset.archive_name, &archive_bytes)?;
        }
        let staging = self.directory.join(format!("staging-{version}"));
        if staging.exists() {
            fs::remove_dir_all(&staging).map_err(|source| CoreInstallError::Write {
                path: staging.clone(),
                source,
            })?;
        }
        fs::create_dir_all(&staging).map_err(|source| CoreInstallError::Write {
            path: staging.clone(),
            source,
        })?;
        let extract_result = extract_sing_box_archive(&archive_bytes, &staging, &asset);
        if extract_result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        extract_result?;
        let extracted = staging.join(&asset.extract_dir).join(&asset.binary_name);
        self.verify_and_commit(
            CoreKind::SingBox,
            &version,
            &extracted,
            self.directory.join(binary_file_name(SING_BOX_BINARY)),
        )?;
        let _ = fs::remove_dir_all(&staging);
        Ok(())
    }

    async fn install_xray(
        &self,
        target: &TargetPlatform,
        geo_directory: Option<&Path>,
    ) -> Result<(), CoreInstallError> {
        let release = fetch_release(XRAY_RELEASE_API).await?;
        let version = normalize_tag(&release.tag_name)?;
        let asset = xray_asset(*target);
        let archive_bytes = download_named_asset(&release, &asset.archive_name).await?;
        let staging = self.directory.join(format!("staging-xray-{version}"));
        if staging.exists() {
            fs::remove_dir_all(&staging).map_err(|source| CoreInstallError::Write {
                path: staging.clone(),
                source,
            })?;
        }
        fs::create_dir_all(&staging).map_err(|source| CoreInstallError::Write {
            path: staging.clone(),
            source,
        })?;
        let extract_result = extract_xray_archive(&archive_bytes, &staging, &asset, geo_directory);
        if extract_result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        extract_result?;
        let extracted = staging.join(&asset.binary_name);
        self.verify_and_commit(
            CoreKind::Xray,
            &version,
            &extracted,
            self.directory.join(binary_file_name(XRAY_BINARY)),
        )?;
        let _ = fs::remove_dir_all(&staging);
        Ok(())
    }

    fn verify_and_commit(
        &self,
        kind: CoreKind,
        version: &str,
        extracted: &Path,
        destination: PathBuf,
    ) -> Result<(), CoreInstallError> {
        let contents = fs::read(extracted).map_err(|source| CoreInstallError::ReadBinary {
            path: extracted.to_path_buf(),
            source,
        })?;
        let sha256 = Sha256Hash::digest(&contents);
        let temporary = destination.with_extension("partial");
        fs::write(&temporary, &contents).map_err(|source| CoreInstallError::Write {
            path: temporary.clone(),
            source,
        })?;
        mark_executable(&temporary)?;
        verify_installed_core(kind, &temporary, version, &self.directory)?;
        let mut manifest = self.load_manifest()?.unwrap_or_default();
        let previous = match kind {
            CoreKind::SingBox => manifest.sing_box.take(),
            CoreKind::Xray => manifest.xray.take(),
        };
        let previous_backup = previous
            .as_ref()
            .map(|entry| archive_previous(&self.directory, entry))
            .transpose()?;
        if let Err(error) = fs::rename(&temporary, &destination) {
            let _ = fs::remove_file(&temporary);
            rollback_failed_install(
                &self.directory,
                &destination,
                previous.as_ref(),
                previous_backup.as_deref(),
                &mut manifest,
                kind,
            )?;
            return Err(CoreInstallError::Write {
                path: destination,
                source: error,
            });
        }
        let entry = InstalledCoreEntry {
            version: version.to_owned(),
            sha256: sha256.to_string(),
            binary: destination.display().to_string(),
            previous_version: previous.as_ref().map(|item| item.version.clone()),
        };
        match kind {
            CoreKind::SingBox => manifest.sing_box = Some(entry),
            CoreKind::Xray => manifest.xray = Some(entry),
        }
        if let Err(error) = write_manifest(&self.directory, &manifest) {
            rollback_failed_install(
                &self.directory,
                &destination,
                previous.as_ref(),
                previous_backup.as_deref(),
                &mut manifest,
                kind,
            )?;
            return Err(error);
        }
        Ok(())
    }
}

struct SingBoxAsset {
    archive_name: String,
    checksums_name: String,
    extract_dir: String,
    binary_name: String,
}

struct XrayAsset {
    archive_name: String,
    binary_name: String,
}

fn sing_box_asset(target: TargetPlatform, version: &str) -> SingBoxAsset {
    let suffix = std::env::consts::EXE_SUFFIX;
    match (target.os(), target.architecture()) {
        (OperatingSystem::MacOs, CpuArchitecture::X86_64) => SingBoxAsset {
            archive_name: format!("sing-box-{version}-darwin-amd64.tar.gz"),
            checksums_name: format!("sing-box-{version}-checksums.txt"),
            extract_dir: format!("sing-box-{version}-darwin-amd64"),
            binary_name: format!("sing-box{suffix}"),
        },
        (OperatingSystem::MacOs, CpuArchitecture::Aarch64) => SingBoxAsset {
            archive_name: format!("sing-box-{version}-darwin-arm64.tar.gz"),
            checksums_name: format!("sing-box-{version}-checksums.txt"),
            extract_dir: format!("sing-box-{version}-darwin-arm64"),
            binary_name: format!("sing-box{suffix}"),
        },
        (OperatingSystem::Windows, CpuArchitecture::X86_64) => SingBoxAsset {
            archive_name: format!("sing-box-{version}-windows-amd64.zip"),
            checksums_name: format!("sing-box-{version}-checksums.txt"),
            extract_dir: format!("sing-box-{version}-windows-amd64"),
            binary_name: format!("sing-box{suffix}"),
        },
        (OperatingSystem::Linux, CpuArchitecture::X86_64) => SingBoxAsset {
            archive_name: format!("sing-box-{version}-linux-amd64.tar.gz"),
            checksums_name: format!("sing-box-{version}-checksums.txt"),
            extract_dir: format!("sing-box-{version}-linux-amd64"),
            binary_name: format!("sing-box{suffix}"),
        },
        (OperatingSystem::Windows | OperatingSystem::Linux, CpuArchitecture::Aarch64) => {
            unreachable!("V0.1 matrix excludes aarch64 on Windows/Linux")
        }
    }
}

fn xray_asset(target: TargetPlatform) -> XrayAsset {
    let suffix = std::env::consts::EXE_SUFFIX;
    match (target.os(), target.architecture()) {
        (OperatingSystem::MacOs, CpuArchitecture::X86_64) => XrayAsset {
            archive_name: "Xray-macos-64.zip".to_owned(),
            binary_name: format!("xray{suffix}"),
        },
        (OperatingSystem::MacOs, CpuArchitecture::Aarch64) => XrayAsset {
            archive_name: "Xray-macos-arm64-v8a.zip".to_owned(),
            binary_name: format!("xray{suffix}"),
        },
        (OperatingSystem::Windows, CpuArchitecture::X86_64) => XrayAsset {
            archive_name: "Xray-windows-64.zip".to_owned(),
            binary_name: format!("xray{suffix}"),
        },
        (OperatingSystem::Linux, CpuArchitecture::X86_64) => XrayAsset {
            archive_name: "Xray-linux-64.zip".to_owned(),
            binary_name: format!("xray{suffix}"),
        },
        (OperatingSystem::Windows | OperatingSystem::Linux, CpuArchitecture::Aarch64) => {
            unreachable!("V0.1 matrix excludes aarch64 on Windows/Linux")
        }
    }
}

fn binary_file_name(stem: &str) -> String {
    format!("{stem}{}", std::env::consts::EXE_SUFFIX)
}

#[derive(Clone, Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Clone, Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

async fn fetch_release(url: &str) -> Result<GithubRelease, CoreInstallError> {
    ensure_rustls_crypto_provider();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(DOWNLOAD_TIMEOUT)
        .user_agent(concat!("MgClash/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|source| CoreInstallError::ClientBuild {
            source: source.without_url(),
        })?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|source| CoreInstallError::RequestFailed {
            url: url.to_owned(),
            source: source.without_url(),
        })?
        .error_for_status()
        .map_err(|source| CoreInstallError::RequestFailed {
            url: url.to_owned(),
            source: source.without_url(),
        })?;
    let body = response
        .text()
        .await
        .map_err(|source| CoreInstallError::RequestFailed {
            url: url.to_owned(),
            source: source.without_url(),
        })?;
    serde_json::from_str(&body).map_err(|source| CoreInstallError::ParseRelease { source })
}

async fn download_named_asset(
    release: &GithubRelease,
    name: &str,
) -> Result<Vec<u8>, CoreInstallError> {
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == name)
        .ok_or_else(|| CoreInstallError::AssetNotFound {
            release: release.tag_name.clone(),
            name: name.to_owned(),
        })?;
    download_url(&asset.browser_download_url).await
}

async fn download_optional_asset(
    release: &GithubRelease,
    name: &str,
) -> Result<Option<String>, CoreInstallError> {
    let Some(asset) = release.assets.iter().find(|asset| asset.name == name) else {
        return Ok(None);
    };
    let body = download_url(&asset.browser_download_url).await?;
    Ok(Some(String::from_utf8(body).map_err(|_| CoreInstallError::InvalidChecksumFile {
        name: name.to_owned(),
    })?))
}

async fn download_url(url: &str) -> Result<Vec<u8>, CoreInstallError> {
    ensure_rustls_crypto_provider();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(DOWNLOAD_TIMEOUT)
        .user_agent(concat!("MgClash/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|source| CoreInstallError::ClientBuild {
            source: source.without_url(),
        })?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|source| CoreInstallError::RequestFailed {
            url: url.to_owned(),
            source: source.without_url(),
        })?
        .error_for_status()
        .map_err(|source| CoreInstallError::RequestFailed {
            url: url.to_owned(),
            source: source.without_url(),
        })?;
    let bytes = response
        .bytes()
        .await
        .map_err(|source| CoreInstallError::RequestFailed {
            url: url.to_owned(),
            source: source.without_url(),
        })?;
    if bytes.len() > MAX_ARCHIVE_BYTES {
        return Err(CoreInstallError::TooLarge {
            url: url.to_owned(),
            bytes: bytes.len(),
        });
    }
    if bytes.is_empty() {
        return Err(CoreInstallError::EmptyBody { url: url.to_owned() });
    }
    Ok(bytes.into())
}

fn verify_archive_checksum(
    checksums: &str,
    archive_name: &str,
    archive_bytes: &[u8],
) -> Result<(), CoreInstallError> {
    let expected = parse_checksum_entry(checksums, archive_name).ok_or_else(|| {
        CoreInstallError::ChecksumEntryMissing {
            archive: archive_name.to_owned(),
        }
    })?;
    let actual = Sha256Hash::digest(archive_bytes).to_string();
    if actual != expected {
        return Err(CoreInstallError::ArchiveChecksumMismatch {
            archive: archive_name.to_owned(),
            expected,
            actual,
        });
    }
    Ok(())
}

fn parse_checksum_entry(checksums: &str, file_name: &str) -> Option<String> {
    for line in checksums.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?.trim_start_matches('*');
        if name == file_name {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

fn extract_sing_box_archive(
    archive_bytes: &[u8],
    destination: &Path,
    asset: &SingBoxAsset,
) -> Result<(), CoreInstallError> {
    if asset.archive_name.ends_with(".tar.gz") {
        let decoder = GzDecoder::new(archive_bytes);
        let mut archive = Archive::new(decoder);
        archive.unpack(destination).map_err(|source| CoreInstallError::Extract {
            archive: asset.archive_name.clone(),
            source,
        })
    } else {
        extract_zip(archive_bytes, destination, Some(&asset.extract_dir))
    }
}

fn extract_xray_archive(
    archive_bytes: &[u8],
    destination: &Path,
    asset: &XrayAsset,
    geo_directory: Option<&Path>,
) -> Result<(), CoreInstallError> {
    extract_zip(archive_bytes, destination, None)?;
    let binary = destination.join(&asset.binary_name);
    if !binary.is_file() {
        return Err(CoreInstallError::BinaryMissing {
            path: binary,
        });
    }
    if let Some(geo_directory) = geo_directory {
        copy_if_present(&destination.join(GEOIP_FILE), &geo_directory.join(GEOIP_FILE))?;
        copy_if_present(
            &destination.join(GEOSITE_FILE),
            &geo_directory.join(GEOSITE_FILE),
        )?;
    }
    Ok(())
}

fn extract_zip(
    archive_bytes: &[u8],
    destination: &Path,
    nested_prefix: Option<&str>,
) -> Result<(), CoreInstallError> {
    let reader = Cursor::new(archive_bytes);
    let mut archive = ZipArchive::new(reader).map_err(|source| CoreInstallError::ExtractZip {
        source,
    })?;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|source| CoreInstallError::ExtractZip {
            source,
        })?;
        let Some(name) = file.enclosed_name().map(PathBuf::from) else {
            continue;
        };
        let relative = match nested_prefix {
            Some(prefix) if name.starts_with(Path::new(prefix)) => name
                .strip_prefix(prefix)
                .map(|value| value.to_path_buf())
                .unwrap_or_else(|_| name.clone()),
            Some(_) => continue,
            None => name,
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = destination.join(relative);
        if file.is_dir() {
            fs::create_dir_all(&target).map_err(|source| CoreInstallError::Write {
                path: target,
                source,
            })?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|source| CoreInstallError::Write {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|source| CoreInstallError::ExtractZip {
                    source: zip::result::ZipError::Io(source),
                })?;
            fs::write(&target, contents).map_err(|source| CoreInstallError::Write {
                path: target,
                source,
            })?;
        }
    }
    Ok(())
}

fn copy_if_present(source: &Path, destination: &Path) -> Result<(), CoreInstallError> {
    if !source.is_file() {
        return Ok(());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|source| CoreInstallError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let temporary = destination.with_extension("dat.partial");
    fs::copy(source, &temporary).map_err(|source| CoreInstallError::Write {
        path: temporary.clone(),
        source,
    })?;
    fs::rename(&temporary, destination).map_err(|source| CoreInstallError::Write {
        path: destination.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn verify_installed_core(
    kind: CoreKind,
    binary: &Path,
    expected_version: &str,
    smoke_directory: &Path,
) -> Result<(), CoreInstallError> {
    match kind {
        CoreKind::SingBox => {
            verify_sing_box_version(binary, expected_version)?;
            verify_sing_box_config(binary, smoke_directory)?;
        }
        CoreKind::Xray => {
            verify_xray_version(binary, expected_version)?;
            verify_xray_config(binary, smoke_directory)?;
        }
    }
    Ok(())
}

fn verify_sing_box_config(binary: &Path, directory: &Path) -> Result<(), CoreInstallError> {
    let config_path = write_smoke_config(directory, "sing-box", SING_BOX_SMOKE_CONFIG)?;
    let result = SingBoxAdapter::new(read_validated_binary(binary)?).validate_config(&config_path);
    let _ = fs::remove_file(&config_path);
    result
        .map(|_| ())
        .map_err(CoreInstallError::SingBoxVersion)
}

fn verify_xray_config(binary: &Path, directory: &Path) -> Result<(), CoreInstallError> {
    let config_path = write_smoke_config(directory, "xray", XRAY_SMOKE_CONFIG)?;
    let result = XrayAdapter::new(read_validated_binary(binary)?).validate_config(&config_path);
    let _ = fs::remove_file(&config_path);
    result
        .map(|_| ())
        .map_err(CoreInstallError::XrayVersion)
}

fn write_smoke_config(
    directory: &Path,
    core: &str,
    body: &str,
) -> Result<PathBuf, CoreInstallError> {
    let path = directory.join(format!("{SMOKE_CONFIG_STEM}-{core}.json"));
    fs::write(&path, body).map_err(|source| CoreInstallError::Write {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

fn rollback_failed_install(
    store_directory: &Path,
    destination: &Path,
    previous: Option<&InstalledCoreEntry>,
    previous_backup: Option<&Path>,
    manifest: &mut CoreInstallManifest,
    kind: CoreKind,
) -> Result<(), CoreInstallError> {
    if destination.is_file() {
        fs::remove_file(destination).map_err(|source| CoreInstallError::Write {
            path: destination.to_path_buf(),
            source,
        })?;
    }
    if let (Some(previous), Some(backup)) = (previous, previous_backup) {
        if backup.is_file() {
            fs::rename(backup, destination).map_err(|source| CoreInstallError::Write {
                path: destination.to_path_buf(),
                source,
            })?;
        }
        match kind {
            CoreKind::SingBox => manifest.sing_box = Some(previous.clone()),
            CoreKind::Xray => manifest.xray = Some(previous.clone()),
        }
        write_manifest(store_directory, manifest)?;
    }
    Ok(())
}

fn verify_sing_box_version(binary: &Path, expected: &str) -> Result<(), CoreInstallError> {
    let version = SingBoxAdapter::new(read_validated_binary(binary)?)
        .version()
        .map_err(CoreInstallError::SingBoxVersion)?;
    ensure_version_matches(expected, version.as_str())
}

fn verify_xray_version(binary: &Path, expected: &str) -> Result<(), CoreInstallError> {
    let version = XrayAdapter::new(read_validated_binary(binary)?)
        .version()
        .map_err(CoreInstallError::XrayVersion)?;
    ensure_version_matches(expected, version.as_str())
}

fn ensure_version_matches(expected: &str, actual: &str) -> Result<(), CoreInstallError> {
    let expected = normalize_tag(expected).unwrap_or_else(|_| expected.to_owned());
    let actual = normalize_tag(actual).unwrap_or_else(|_| actual.to_owned());
    if expected != actual {
        return Err(CoreInstallError::VersionMismatch { expected, actual });
    }
    Ok(())
}

fn read_validated_binary(
    path: &Path,
) -> Result<magies_core_runtime::ValidatedCoreBinary, CoreInstallError> {
    let contents = fs::read(path).map_err(|source| CoreInstallError::ReadBinary {
        path: path.to_path_buf(),
        source,
    })?;
    let sha256 = Sha256Hash::digest(&contents);
    let target = TargetPlatform::parse(std::env::consts::OS, std::env::consts::ARCH)
        .map_err(CoreInstallError::Target)?;
    locate_core_binary(
        path,
        CoreBinaryRequirement::new(target.architecture(), sha256),
    )
    .map_err(|source| CoreInstallError::ValidateBinary {
        path: path.to_path_buf(),
        source,
    })
}

fn normalize_tag(tag: &str) -> Result<String, CoreInstallError> {
    let trimmed = tag.trim().trim_start_matches('v');
    if trimmed.is_empty() {
        return Err(CoreInstallError::EmptyVersionTag);
    }
    Ok(trimmed.to_owned())
}

fn archive_previous(
    directory: &Path,
    previous: &InstalledCoreEntry,
) -> Result<PathBuf, CoreInstallError> {
    let previous_path = PathBuf::from(&previous.binary);
    if !previous_path.is_file() {
        return Err(CoreInstallError::BinaryMissing {
            path: previous_path,
        });
    }
    let backup_dir = directory.join("previous");
    fs::create_dir_all(&backup_dir).map_err(|source| CoreInstallError::Write {
        path: backup_dir.clone(),
        source,
    })?;
    let backup = backup_dir.join(format!(
        "{}-{}",
        previous_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("core"),
        previous.version
    ));
    if backup.exists() {
        fs::remove_file(&backup).map_err(|source| CoreInstallError::Write {
            path: backup.clone(),
            source,
        })?;
    }
    fs::rename(&previous_path, &backup).map_err(|source| CoreInstallError::Write {
        path: backup.clone(),
        source,
    })?;
    Ok(backup)
}

fn write_manifest(directory: &Path, manifest: &CoreInstallManifest) -> Result<(), CoreInstallError> {
    let path = directory.join(MANIFEST_FILE);
    let temporary = path.with_extension("json.partial");
    let body = serde_json::to_string_pretty(manifest).map_err(|source| CoreInstallError::WriteManifest {
        source,
    })?;
    fs::write(&temporary, body).map_err(|source| CoreInstallError::Write {
        path: temporary.clone(),
        source,
    })?;
    fs::rename(&temporary, &path).map_err(|source| CoreInstallError::Write {
        path,
        source,
    })?;
    Ok(())
}

fn mark_executable(path: &Path) -> Result<(), CoreInstallError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|source| CoreInstallError::Write {
                path: path.to_path_buf(),
                source,
            })?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(|source| CoreInstallError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// Resolves sing-box settings with install-store awareness.
///
/// Environment overrides win; otherwise a user-installed binary is preferred
/// over the bundled artifact.
pub fn sing_box_settings_with_store(
    store: Option<&CoreInstallStore>,
) -> Result<CoreSettings, CoreSettingsError> {
    if std::env::var_os(BINARY_PATH_VARIABLE).is_some() || std::env::var(SHA256_VARIABLE).is_ok() {
        return CoreSettings::from_env();
    }
    if let Some(store) = store {
        if let Some(settings) = store.sing_box_settings() {
            return settings;
        }
    }
    CoreSettings::from_env()
}

/// Resolves Xray settings with install-store awareness.
pub fn xray_settings_with_store(
    store: Option<&CoreInstallStore>,
) -> Result<CoreSettings, CoreSettingsError> {
    if std::env::var_os(XRAY_BINARY_VARIABLE).is_some()
        || std::env::var(XRAY_SHA256_VARIABLE).is_ok()
    {
        return CoreSettings::from_values(
            std::env::var_os(XRAY_BINARY_VARIABLE).map(PathBuf::from),
            std::env::var(XRAY_SHA256_VARIABLE).ok(),
        );
    }
    if let Some(store) = store {
        if let Some(settings) = store.xray_settings() {
            return settings;
        }
    }
    CoreSettings::from_values(
        std::env::var_os(XRAY_BINARY_VARIABLE).map(PathBuf::from),
        std::env::var(XRAY_SHA256_VARIABLE).ok(),
    )
}

#[derive(Debug, Error)]
pub enum CoreInstallError {
    #[error("unknown Core name {name:?}")]
    UnknownCore { name: String },
    #[error("failed to create Core install directory {}: {source}", path.display())]
    CreateDirectory { path: PathBuf, source: io::Error },
    #[error("failed to read Core manifest {}: {source}", path.display())]
    ReadManifest { path: PathBuf, source: io::Error },
    #[error("failed to parse Core manifest")]
    ParseManifest { source: serde_json::Error },
    #[error("failed to build the Core download client")]
    ClientBuild { source: reqwest::Error },
    #[error("failed to download {url}")]
    RequestFailed { url: String, source: reqwest::Error },
    #[error("failed to parse a GitHub release response")]
    ParseRelease { source: serde_json::Error },
    #[error("release {release} has no asset named {name}")]
    AssetNotFound { release: String, name: String },
    #[error("Core download from {url} exceeded {bytes} bytes")]
    TooLarge { url: String, bytes: usize },
    #[error("Core download from {url} returned an empty body")]
    EmptyBody { url: String },
    #[error("checksum file {name} is not valid UTF-8")]
    InvalidChecksumFile { name: String },
    #[error("checksum file has no entry for {archive}")]
    ChecksumEntryMissing { archive: String },
    #[error("archive {archive} checksum mismatch (expected {expected}, got {actual})")]
    ArchiveChecksumMismatch {
        archive: String,
        expected: String,
        actual: String,
    },
    #[error("failed to extract {archive}: {source}")]
    Extract {
        archive: String,
        source: io::Error,
    },
    #[error("failed to extract zip archive: {source}")]
    ExtractZip { source: zip::result::ZipError },
    #[error("extracted Core binary is missing at {}", path.display())]
    BinaryMissing { path: PathBuf },
    #[error("failed to read Core binary {}: {source}", path.display())]
    ReadBinary { path: PathBuf, source: io::Error },
    #[error("failed to validate Core binary {}: {source}", path.display())]
    ValidateBinary {
        path: PathBuf,
        source: magies_core_runtime::CoreBinaryError,
    },
    #[error("sing-box reported version {actual}, expected {expected}")]
    VersionMismatch { expected: String, actual: String },
    #[error("GitHub release tag is empty")]
    EmptyVersionTag,
    #[error(transparent)]
    SingBoxVersion(#[from] SingBoxAdapterError),
    #[error(transparent)]
    XrayVersion(#[from] XrayAdapterError),
    #[error(transparent)]
    Target(#[from] PlatformError),
    #[error("failed to write Core install file {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("failed to serialize Core manifest: {source}")]
    WriteManifest { source: serde_json::Error },
}

impl CoreInstallError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::CreateDirectory { .. }
            | Self::ReadManifest { .. }
            | Self::ParseManifest { .. }
            | Self::Write { .. }
            | Self::WriteManifest { .. } => "core_install_store_failed",
            Self::UnknownCore { .. } => "core_install_unknown_core",
            Self::ClientBuild { .. }
            | Self::RequestFailed { .. }
            | Self::ParseRelease { .. }
            | Self::AssetNotFound { .. }
            | Self::TooLarge { .. }
            | Self::EmptyBody { .. }
            | Self::InvalidChecksumFile { .. }
            | Self::ChecksumEntryMissing { .. }
            | Self::ArchiveChecksumMismatch { .. }
            | Self::Extract { .. }
            | Self::ExtractZip { .. }
            | Self::BinaryMissing { .. }
            | Self::ReadBinary { .. }
            | Self::ValidateBinary { .. }
            | Self::VersionMismatch { .. }
            | Self::EmptyVersionTag { .. }
            | Self::SingBoxVersion { .. }
            | Self::XrayVersion { .. }
            | Self::Target { .. } => "core_install_download_failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn parses_checksum_lines() {
        let body = "abc123  sing-box-1.14.0-linux-amd64.tar.gz\n";
        assert_eq!(
            parse_checksum_entry(body, "sing-box-1.14.0-linux-amd64.tar.gz"),
            Some("abc123".to_owned())
        );
    }

    #[test]
    fn builds_sing_box_asset_names_for_macos_arm64() {
        let target = TargetPlatform::parse("macos", "aarch64").unwrap();
        let asset = sing_box_asset(target, "1.14.0");
        assert_eq!(asset.archive_name, "sing-box-1.14.0-darwin-arm64.tar.gz");
        assert_eq!(asset.extract_dir, "sing-box-1.14.0-darwin-arm64");
    }

    #[test]
    fn normalizes_release_tags() {
        assert_eq!(normalize_tag("v1.14.0").unwrap(), "1.14.0");
        assert_eq!(normalize_tag("26.3.27").unwrap(), "26.3.27");
    }

    #[test]
    fn rollback_restores_a_replaced_core_and_manifest() {
        let directory = std::env::temp_dir().join(format!(
            "mgclash-core-rollback-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        let destination = directory.join("sing-box");
        fs::write(&destination, b"old-core").unwrap();
        let backup_dir = directory.join("previous");
        fs::create_dir_all(&backup_dir).unwrap();
        let backup = backup_dir.join("sing-box-1.13.18");
        fs::rename(&destination, &backup).unwrap();
        fs::write(&destination, b"broken-core").unwrap();
        let previous = InstalledCoreEntry {
            version: "1.13.18".to_owned(),
            sha256: "abc".to_owned(),
            binary: destination.display().to_string(),
            previous_version: None,
        };
        let mut manifest = CoreInstallManifest {
            sing_box: Some(InstalledCoreEntry {
                version: "1.14.0".to_owned(),
                sha256: "def".to_owned(),
                binary: destination.display().to_string(),
                previous_version: Some("1.13.18".to_owned()),
            }),
            xray: None,
        };
        rollback_failed_install(
            &directory,
            &destination,
            Some(&previous),
            Some(&backup),
            &mut manifest,
            CoreKind::SingBox,
        )
        .unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"old-core");
        assert_eq!(manifest.sing_box.as_ref().unwrap().version, "1.13.18");
        let _ = fs::remove_dir_all(directory);
    }
}
