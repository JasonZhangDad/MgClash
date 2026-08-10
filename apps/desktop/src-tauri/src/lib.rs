//! `MgClash` Tauri desktop shell.

use magies_platform::{TargetPlatform, TunAvailability};
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformSummary {
    pub artifact_identifier: &'static str,
    pub tun_availability: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlatformCommandError {
    pub code: &'static str,
    pub message: String,
}

/// Creates the serializable platform summary returned to the desktop UI.
///
/// # Errors
///
/// Returns a typed `unsupported_target` error when the OS/CPU pair is outside
/// the V0.1 support matrix.
pub fn platform_summary_for(
    os: &str,
    architecture: &str,
) -> Result<PlatformSummary, PlatformCommandError> {
    let target = TargetPlatform::parse(os, architecture).map_err(|error| PlatformCommandError {
        code: "unsupported_target",
        message: error.to_string(),
    })?;
    let tun_availability = match target.unsigned_tun_availability() {
        TunAvailability::UnavailableInUnsignedBuild => "unavailableInUnsignedBuild",
        TunAvailability::RequiresElevation => "requiresElevation",
    };

    Ok(PlatformSummary {
        artifact_identifier: target.artifact_identifier(),
        tun_availability,
    })
}

#[tauri::command]
fn platform_summary() -> Result<PlatformSummary, PlatformCommandError> {
    platform_summary_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// Starts the desktop application event loop.
///
/// # Panics
///
/// Panics when Tauri cannot initialize or run the desktop shell.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![platform_summary])
        .run(tauri::generate_context!())
        .expect("failed to run MgClash desktop shell");
}

#[cfg(test)]
mod tests {
    use super::platform_summary;

    #[test]
    fn command_supports_the_build_host() {
        platform_summary().expect("CI must run on a supported V0.1 target");
    }
}
