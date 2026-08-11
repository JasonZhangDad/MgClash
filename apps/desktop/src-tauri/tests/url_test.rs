use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use magies_desktop_lib::url_test::{UrlTestError, probe_url};

#[test]
fn requests_the_test_url_through_the_local_http_proxy() {
    let proxy = TestProxy::spawn(
        Duration::ZERO,
        b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );

    let latency = tauri::async_runtime::block_on(probe_url(
        "http://probe.example/generate_204",
        proxy.address,
        Duration::from_secs(1),
    ))
    .unwrap();

    assert!(latency <= 1_000);
    assert!(
        proxy
            .request
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .starts_with("GET http://probe.example/generate_204 HTTP/1.1\r\n")
    );
}

#[test]
fn classifies_timeout_and_unsuccessful_status() {
    let slow_proxy = TestProxy::spawn(
        Duration::from_millis(100),
        b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n",
    );
    assert!(matches!(
        tauri::async_runtime::block_on(probe_url(
            "http://probe.example/generate_204",
            slow_proxy.address,
            Duration::from_millis(10),
        )),
        Err(UrlTestError::TimedOut)
    ));

    let failing_proxy = TestProxy::spawn(
        Duration::ZERO,
        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n",
    );
    assert!(matches!(
        tauri::async_runtime::block_on(probe_url(
            "http://probe.example/generate_204",
            failing_proxy.address,
            Duration::from_secs(1),
        )),
        Err(UrlTestError::HttpStatus { status: 503 })
    ));
}

#[test]
fn validates_the_url_and_timeout_before_connecting() {
    let address = "127.0.0.1:9".parse().unwrap();

    assert!(matches!(
        tauri::async_runtime::block_on(probe_url(
            "file:///tmp/probe",
            address,
            Duration::from_secs(1),
        )),
        Err(UrlTestError::UnsupportedScheme { .. })
    ));
    assert!(matches!(
        tauri::async_runtime::block_on(probe_url(
            "https://probe.example/generate_204",
            address,
            Duration::ZERO,
        )),
        Err(UrlTestError::InvalidTimeout)
    ));
}

struct TestProxy {
    address: SocketAddr,
    request: Receiver<String>,
}

impl TestProxy {
    fn spawn(delay: Duration, response: &'static [u8]) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, request) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0_u8; 1024];
            while !bytes.ends_with(b"\r\n\r\n") {
                let read = stream.read(&mut chunk).unwrap();
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..read]);
            }
            sender.send(String::from_utf8(bytes).unwrap()).unwrap();
            thread::sleep(delay);
            let _ = stream.write_all(response);
        });
        Self { address, request }
    }
}
