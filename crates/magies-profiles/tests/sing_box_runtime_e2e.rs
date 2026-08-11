use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::{self, sleep};
use std::time::{Duration, Instant};

use magies_domain::{CredentialRef, ProxyNode, ProxyProtocol, TransportConfig};
use magies_profiles::{
    DnsProfile, DnsServer, DnsStrategy, LocalHttpProfile, LocalSocksProfile, NodeCredential,
    ShadowsocksParser, SingBoxRuntimeConfigGenerator, SingBoxRuntimeProfile,
};
use magies_routing::{RouteOutbound, RouteProfile, RoutingMode, RoutingRule};
use serde_json::{Value, json};
use uuid::Uuid;

/// Budget for one socket read once everything is running.
const TIMEOUT: Duration = Duration::from_secs(5);
/// Budget for a freshly spawned sing-box to bind its listener. Process startup
/// and a socket read are different things: a cold Windows runner scanning a
/// just-downloaded binary regularly needs more than the read budget, which is
/// what made this test fail there while passing everywhere else.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

#[test]
#[ignore = "requires MAGIES_SING_BOX_CONFIG_BIN pointing to official sing-box 1.13.18"]
fn generated_runtime_proxies_an_http_request_through_selected_node() {
    let binary = PathBuf::from(
        std::env::var_os("MAGIES_SING_BOX_CONFIG_BIN")
            .expect("MAGIES_SING_BOX_CONFIG_BIN must point to official sing-box"),
    );
    let ports = available_ports(5);
    let server_port = ports[0];
    let socks_port = ports[1];
    let http_port = ports[2];
    let unreachable_port = ports[3];
    let api_port = ports[4];
    let server_config = ConfigFile::new(
        "server",
        &json!({
            "log": { "level": "warn" },
            "inbounds": [{
                "type": "shadowsocks",
                "tag": "ss-in",
                "listen": "127.0.0.1",
                "listen_port": server_port,
                "method": "aes-256-gcm",
                "password": "e2e-secret"
            }],
            "outbounds": [{ "type": "direct", "tag": "direct" }],
            "route": { "final": "direct" }
        }),
    );
    let _server = CoreProcess::start(&binary, server_config.path());
    wait_for_port(server_port);

    let parsed = ShadowsocksParser
        .parse(&format!(
            "ss://aes-256-gcm:e2e-secret@127.0.0.1:{server_port}"
        ))
        .unwrap();
    let mut node = ProxyNode::new(
        Uuid::parse_str("018f78b5-08ee-7caa-94f3-1d5d781aba22").unwrap(),
        "E2E Shadowsocks",
        ProxyProtocol::Shadowsocks,
        "127.0.0.1",
        u32::from(server_port),
        Some(CredentialRef::new("keychain://nodes/e2e").unwrap()),
    )
    .unwrap();
    node.transport = Some(TransportConfig::Tcp);
    let dns = DnsProfile::new(
        vec![DnsServer::system("system").unwrap()],
        Vec::new(),
        "system",
        DnsStrategy::PreferIpv4,
        false,
        false,
    )
    .unwrap();
    let route = RouteProfile::new(
        RoutingMode::Rule,
        vec![RoutingRule::domain("localhost", RouteOutbound::Proxy, 0, true).unwrap()],
        RouteOutbound::Direct,
    )
    .unwrap();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        NodeCredential::from(parsed.credential()),
        &dns,
        &route,
    )
    .with_local_proxies(
        LocalSocksProfile::new(u32::from(socks_port)).unwrap(),
        LocalHttpProfile::new(u32::from(http_port)).unwrap(),
    )
    .unwrap()
    .with_clash_api_port(NonZeroU16::new(api_port).unwrap())
    .unwrap();
    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();
    let mut client_json = generated.json().clone();
    client_json["outbounds"][1] = json!({
        "type": "socks",
        "tag": "direct",
        "server": "127.0.0.1",
        "server_port": unreachable_port
    });
    let client_config = ConfigFile::new("client", &client_json);
    let _client = CoreProcess::start(&binary, client_config.path());
    wait_for_port(socks_port);
    wait_for_port(http_port);
    wait_for_port(api_port);

    assert_traffic_stream(api_port);
    assert_http_proxy_request(http_port);
}

fn assert_traffic_stream(api_port: u16) {
    let mut api = TcpStream::connect(("127.0.0.1", api_port)).unwrap();
    api.set_read_timeout(Some(TIMEOUT)).unwrap();
    write!(
        api,
        "GET /traffic HTTP/1.1\r\nHost: 127.0.0.1:{api_port}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();

    let mut response = Vec::new();
    let mut chunk = [0_u8; 512];
    loop {
        let length = api.read(&mut chunk).unwrap();
        assert!(length > 0, "traffic API closed without a sample");
        response.extend_from_slice(&chunk[..length]);
        let Some(header_end) = response.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
            continue;
        };
        let body = &response[header_end + 4..];
        let Some(size_end) = body.windows(2).position(|bytes| bytes == b"\r\n") else {
            continue;
        };
        let size =
            usize::from_str_radix(std::str::from_utf8(&body[..size_end]).unwrap(), 16).unwrap();
        let sample_start = size_end + 2;
        if body.len() < sample_start + size {
            continue;
        }
        let sample: Value =
            serde_json::from_slice(&body[sample_start..sample_start + size]).unwrap();
        assert!(sample["up"].is_u64());
        assert!(sample["down"].is_u64());
        return;
    }
}

fn assert_http_proxy_request(proxy_port: u16) {
    let origin = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let origin_port = origin.local_addr().unwrap().port();
    let origin_thread = thread::spawn(move || {
        let (mut stream, _) = origin.accept().unwrap();
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        let mut request = [0_u8; 512];
        let length = stream.read(&mut request).unwrap();
        assert!(String::from_utf8_lossy(&request[..length]).starts_with("GET /through-proxy "));
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });

    let mut proxy = TcpStream::connect(("127.0.0.1", proxy_port)).unwrap();
    proxy.set_read_timeout(Some(TIMEOUT)).unwrap();
    write!(
        proxy,
        "GET http://localhost:{origin_port}/through-proxy HTTP/1.1\r\n\
         Host: localhost:{origin_port}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = Vec::new();
    proxy.read_to_end(&mut response).unwrap();
    assert!(String::from_utf8_lossy(&response).starts_with("HTTP/1.1 204"));
    origin_thread.join().unwrap();
}

fn available_ports(count: usize) -> Vec<u16> {
    let listeners: Vec<_> = (0..count)
        .map(|_| TcpListener::bind(("127.0.0.1", 0)).unwrap())
        .collect();
    listeners
        .iter()
        .map(|listener| listener.local_addr().unwrap().port())
        .collect()
}

fn wait_for_port(port: u16) {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        sleep(Duration::from_millis(25));
    }
    panic!("sing-box did not open 127.0.0.1:{port} within {STARTUP_TIMEOUT:?}");
}

struct CoreProcess(Child);

impl CoreProcess {
    fn start(binary: &Path, config: &Path) -> Self {
        let child = Command::new(binary)
            .arg("run")
            .arg("-c")
            .arg(config)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        Self(child)
    }
}

impl Drop for CoreProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct ConfigFile(PathBuf);

impl ConfigFile {
    fn new(kind: &str, json: &Value) -> Self {
        let path = std::env::temp_dir().join(format!(
            "magies-runtime-{kind}-{}-{}.json",
            std::process::id(),
            std::env::consts::OS
        ));
        fs::write(&path, serde_json::to_vec(json).unwrap()).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ConfigFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
