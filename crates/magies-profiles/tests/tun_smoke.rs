#![cfg(any(target_os = "linux", target_os = "windows"))]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

use magies_platform::OperatingSystem;
use magies_profiles::{SingBoxTunConfigGenerator, TunProfile};

#[test]
#[ignore = "requires an elevated official sing-box binary and a real TUN backend"]
fn official_sing_box_opens_a_tun_device() {
    let binary = PathBuf::from(
        std::env::var_os("MAGIES_SING_BOX_TUN_BIN")
            .expect("MAGIES_SING_BOX_TUN_BIN must point to official sing-box"),
    );
    let platform = if cfg!(target_os = "windows") {
        OperatingSystem::Windows
    } else {
        OperatingSystem::Linux
    };
    let profile = TunProfile::new(platform, false, 1500, false, false).unwrap();
    let config = SingBoxTunConfigGenerator::generate(&profile);
    let config_path = std::env::temp_dir().join(format!(
        "magies-tun-smoke-{}-{}.json",
        std::process::id(),
        std::env::consts::OS
    ));
    fs::write(&config_path, serde_json::to_vec(config.json()).unwrap()).unwrap();

    let mut child = Command::new(binary)
        .arg("run")
        .arg("-c")
        .arg(&config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    sleep(Duration::from_secs(2));
    if let Some(status) = child.try_wait().unwrap() {
        let output = child.wait_with_output().unwrap();
        panic!(
            "sing-box exited before TUN smoke completed ({status}): {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    child.kill().unwrap();
    child.wait().unwrap();
    fs::remove_file(config_path).unwrap();
}
