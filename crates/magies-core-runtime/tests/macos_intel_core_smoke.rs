#![cfg(all(target_os = "macos", target_arch = "x86_64"))]

use std::env::var_os;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

use magies_core_runtime::{CoreProcessSpec, CoreRuntime, CoreState};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const RETRY_INTERVAL: Duration = Duration::from_millis(25);

#[test]
#[ignore = "requires MAGIES_XRAY_BIN pointing to an official macOS Intel binary"]
fn xray_starts_a_local_http_listener_and_stops() {
    run_listener_smoke(
        required_binary("MAGIES_XRAY_BIN"),
        [
            "run",
            "-c",
            fixture("xray-local-http.json").to_str().unwrap(),
        ],
        18_980,
    );
}

#[test]
#[ignore = "requires MAGIES_SING_BOX_BIN pointing to an official macOS Intel binary"]
fn sing_box_starts_a_local_mixed_listener_and_stops() {
    run_listener_smoke(
        required_binary("MAGIES_SING_BOX_BIN"),
        [
            "run",
            "-c",
            fixture("sing-box-local-mixed.json").to_str().unwrap(),
        ],
        18_981,
    );
}

fn run_listener_smoke<I, A>(binary: PathBuf, arguments: I, port: u16)
where
    I: IntoIterator<Item = A>,
    A: Into<std::ffi::OsString>,
{
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let preflight = TcpListener::bind(address).expect("the smoke-test port must be available");
    drop(preflight);

    let mut runtime = CoreRuntime::default();
    runtime
        .start(&CoreProcessSpec::new(binary, arguments))
        .unwrap();

    let started_at = Instant::now();
    loop {
        match runtime.poll().unwrap() {
            CoreState::Running => {}
            state => panic!("Core exited before opening {address}: {state:?}"),
        }

        if TcpStream::connect_timeout(&address, RETRY_INTERVAL).is_ok() {
            break;
        }
        assert!(
            started_at.elapsed() < STARTUP_TIMEOUT,
            "Core did not open {address} within {STARTUP_TIMEOUT:?}"
        );
        sleep(RETRY_INTERVAL);
    }

    runtime.stop().unwrap();
    assert_eq!(runtime.poll().unwrap(), CoreState::Stopped);
}

fn required_binary(variable: &str) -> PathBuf {
    var_os(variable).map_or_else(
        || panic!("{variable} must point to a Core binary"),
        PathBuf::from,
    )
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}
