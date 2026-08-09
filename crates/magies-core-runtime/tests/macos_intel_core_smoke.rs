#![cfg(all(target_os = "macos", target_arch = "x86_64"))]

use std::env::var_os;
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::time::Duration;

use magies_core_runtime::{
    CoreBinaryRequirement, CoreProcessSpec, CoreRuntime, CoreState, Sha256Hash, SingBoxAdapter,
    ValidatedCoreBinary, XrayAdapter, locate_core_binary,
};
use magies_platform::CpuArchitecture;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const XRAY_SHA256: &str = "afd0eaebb77994a18f29b00c5f50a4f7fbb77da06e24352d43035f3cad3c3786";
const SING_BOX_SHA256: &str = "6e9749a4b40821bf07d301f099e75d871ea435861c9f5f0ac5687dc18e81b759";

#[test]
#[ignore = "requires MAGIES_XRAY_BIN pointing to an official macOS Intel binary"]
fn xray_starts_a_local_http_listener_and_stops() {
    let adapter = XrayAdapter::new(validated_binary("MAGIES_XRAY_BIN", XRAY_SHA256));
    assert_eq!(adapter.version().unwrap().as_str(), "26.3.27");
    let config = adapter
        .validate_config(fixture("xray-local-http.json"))
        .unwrap();

    run_listener_smoke(&adapter.process_spec(&config), 18_980);
}

#[test]
#[ignore = "requires MAGIES_SING_BOX_BIN pointing to an official macOS Intel binary"]
fn sing_box_starts_a_local_mixed_listener_and_stops() {
    let adapter = SingBoxAdapter::new(validated_binary("MAGIES_SING_BOX_BIN", SING_BOX_SHA256));
    assert_eq!(adapter.version().unwrap().as_str(), "1.13.18");
    let config = adapter
        .validate_config(fixture("sing-box-local-mixed.json"))
        .unwrap();

    run_listener_smoke(&adapter.process_spec(&config), 18_981);
}

fn run_listener_smoke(spec: &CoreProcessSpec, port: u16) {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let preflight = TcpListener::bind(address).expect("the smoke-test port must be available");
    drop(preflight);

    let mut runtime = CoreRuntime::default();
    runtime.start(spec).unwrap();
    let health = runtime
        .wait_for_tcp_health(address, STARTUP_TIMEOUT)
        .unwrap();
    assert!(health.ready_after <= STARTUP_TIMEOUT);

    runtime.stop().unwrap();
    assert_eq!(runtime.poll().unwrap(), CoreState::Stopped);
}

fn validated_binary(variable: &str, expected_sha256: &str) -> ValidatedCoreBinary {
    let path = var_os(variable).map_or_else(
        || panic!("{variable} must point to a Core binary"),
        PathBuf::from,
    );
    locate_core_binary(
        path,
        CoreBinaryRequirement::new(CpuArchitecture::X86_64, decode_sha256(expected_sha256)),
    )
    .unwrap()
}

fn decode_sha256(hex: &str) -> Sha256Hash {
    assert_eq!(hex.len(), 64, "SHA-256 must contain 64 hex characters");
    let mut bytes = [0_u8; 32];
    for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = u8::from_str_radix(
            std::str::from_utf8(pair).expect("SHA-256 fixture must be UTF-8"),
            16,
        )
        .expect("SHA-256 fixture must contain hex characters");
    }
    Sha256Hash::from_bytes(bytes)
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}
