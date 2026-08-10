#![cfg(all(target_os = "macos", target_arch = "x86_64"))]

use std::env::var_os;
use std::fs::{remove_file, write};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use magies_core_runtime::{
    CoreBinaryRequirement, CoreRuntime, Sha256Hash, SingBoxAdapter, XrayAdapter, locate_core_binary,
};
use magies_domain::CoreType;
use magies_platform::CpuArchitecture;
use magies_profiles::{LocalSocksConfigGenerator, LocalSocksProfile};

const TIMEOUT: Duration = Duration::from_secs(5);
const XRAY_SHA256: &str = "afd0eaebb77994a18f29b00c5f50a4f7fbb77da06e24352d43035f3cad3c3786";
const SING_BOX_SHA256: &str = "6e9749a4b40821bf07d301f099e75d871ea435861c9f5f0ac5687dc18e81b759";

#[test]
#[ignore = "requires MAGIES_XRAY_BIN pointing to official Xray 26.3.27 darwin/amd64"]
fn generated_xray_config_opens_a_socks5_listener() {
    let port = available_port();
    let config_path = generated_config(CoreType::Xray, port);
    let binary = validated_binary("MAGIES_XRAY_BIN", XRAY_SHA256);
    let adapter = XrayAdapter::new(binary);
    assert_eq!(adapter.version().unwrap().as_str(), "26.3.27");
    let config = adapter.validate_config(&config_path).unwrap();
    let mut runtime = CoreRuntime::default();
    let _output = runtime.start(&adapter.process_spec(&config)).unwrap();

    assert_socks5_handshake(&mut runtime, port);
    runtime.stop().unwrap();
    remove_file(config_path).unwrap();
}

#[test]
#[ignore = "requires MAGIES_SING_BOX_BIN pointing to official sing-box 1.13.18 darwin/amd64"]
fn generated_sing_box_config_opens_a_socks5_listener() {
    let port = available_port();
    let config_path = generated_config(CoreType::SingBox, port);
    let binary = validated_binary("MAGIES_SING_BOX_BIN", SING_BOX_SHA256);
    let adapter = SingBoxAdapter::new(binary);
    assert_eq!(adapter.version().unwrap().as_str(), "1.13.18");
    let config = adapter.validate_config(&config_path).unwrap();
    let mut runtime = CoreRuntime::default();
    let _output = runtime.start(&adapter.process_spec(&config)).unwrap();

    assert_socks5_handshake(&mut runtime, port);
    runtime.stop().unwrap();
    remove_file(config_path).unwrap();
}

fn assert_socks5_handshake(runtime: &mut CoreRuntime, port: u16) {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    runtime.wait_for_tcp_health(address, TIMEOUT).unwrap();
    let mut stream = TcpStream::connect_timeout(&address, TIMEOUT).unwrap();
    stream.set_read_timeout(Some(TIMEOUT)).unwrap();
    stream.write_all(&[5, 1, 0]).unwrap();
    let mut response = [0_u8; 2];
    stream.read_exact(&mut response).unwrap();
    assert_eq!(response, [5, 0]);
}

fn generated_config(core_type: CoreType, port: u16) -> PathBuf {
    let profile = LocalSocksProfile::new(u32::from(port)).unwrap();
    let config = LocalSocksConfigGenerator::generate(core_type, &profile);
    let path = std::env::temp_dir().join(format!(
        "mgclash-local-socks-{core_type:?}-{}.json",
        std::process::id()
    ));
    write(&path, serde_json::to_vec_pretty(config.json()).unwrap()).unwrap();
    path
}

fn available_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn validated_binary(
    variable: &str,
    expected_sha256: &str,
) -> magies_core_runtime::ValidatedCoreBinary {
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
    let mut bytes = [0_u8; 32];
    for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (hex_digit(pair[0]) << 4) | hex_digit(pair[1]);
    }
    Sha256Hash::from_bytes(bytes)
}

fn hex_digit(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("invalid SHA-256 digit"),
    }
}
