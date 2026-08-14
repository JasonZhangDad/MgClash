use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use magies_desktop_lib::connections::{
    ConnectionsError, close_all_connections, close_connection, load_connections,
};

const BODY: &str = r#"{
  "downloadTotal": 4096,
  "uploadTotal": 2048,
  "connections": [
    {
      "id": "0f9c1f7e-0000-4000-8000-000000000001",
      "upload": 120,
      "download": 340,
      "start": "2026-08-13T00:00:00Z",
      "chains": ["proxy", "tokyo"],
      "rule": "rule_set",
      "rulePayload": "geosite-geolocation-!cn",
      "metadata": {
        "network": "tcp",
        "host": "example.com",
        "destinationIP": "93.184.216.34",
        "destinationPort": "443",
        "processPath": "/Applications/Safari.app/Contents/MacOS/Safari"
      }
    },
    {
      "id": "0f9c1f7e-0000-4000-8000-000000000002",
      "upload": 0,
      "download": 0,
      "start": "2026-08-13T00:00:01Z",
      "chains": ["direct"],
      "rule": "final",
      "metadata": {
        "network": "udp",
        "host": "",
        "destinationIP": "1.1.1.1",
        "destinationPort": "53"
      }
    }
  ]
}"#;

#[test]
fn lists_the_connections_the_core_reports() {
    let api = TestApi::spawn(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{BODY}",
            BODY.len()
        )
        .into_bytes(),
    );

    let snapshot =
        tauri::async_runtime::block_on(load_connections(api.address, Duration::from_secs(1)))
            .unwrap();

    assert!(
        api.request
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .starts_with("GET /connections HTTP/1.1\r\n")
    );
    assert_eq!(snapshot.upload_total_bytes, 2_048);
    assert_eq!(snapshot.download_total_bytes, 4_096);

    let first = &snapshot.connections[0];
    assert_eq!(first.host, "example.com");
    assert_eq!(first.destination, "93.184.216.34:443");
    assert_eq!(first.network, "tcp");
    // The path is what the API reports; the window has room for the program.
    assert_eq!(first.process, "Safari");
    assert_eq!(first.rule, "rule_set(geosite-geolocation-!cn)");
    assert_eq!(first.chain, "tokyo → proxy");
    assert_eq!(first.upload_bytes, 120);
    assert_eq!(first.download_bytes, 340);
    assert_eq!(first.start, "2026-08-13T00:00:00Z");

    let second = &snapshot.connections[1];
    // No host, so the address is the only thing left to identify it by.
    assert_eq!(second.host, "1.1.1.1");
    assert_eq!(second.process, "");
    assert_eq!(second.rule, "final");
    assert_eq!(second.chain, "direct");
}

#[test]
fn reports_an_unsuccessful_status_rather_than_an_empty_list() {
    let api = TestApi::spawn(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n".to_vec());

    assert!(matches!(
        tauri::async_runtime::block_on(load_connections(api.address, Duration::from_secs(1))),
        Err(ConnectionsError::HttpStatus { status: 401 })
    ));
}

#[test]
fn closes_one_connection_by_id() {
    let api = TestApi::spawn(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_vec());

    tauri::async_runtime::block_on(close_connection(
        api.address,
        "0f9c1f7e-0000-4000-8000-000000000001",
        Duration::from_secs(1),
    ))
    .unwrap();

    assert!(
        api.request
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .starts_with("DELETE /connections/0f9c1f7e-0000-4000-8000-000000000001 HTTP/1.1\r\n")
    );
}

#[test]
fn refuses_an_id_that_would_reach_another_endpoint() {
    // The id reaches the API as a path segment, so anything that is not one
    // has to be rejected before it is sent.
    assert!(matches!(
        tauri::async_runtime::block_on(close_connection(
            SocketAddr::from(([127, 0, 0, 1], 1)),
            "../configs",
            Duration::from_secs(1),
        )),
        Err(ConnectionsError::InvalidId)
    ));
}

#[test]
fn closes_every_connection_at_once() {
    let api = TestApi::spawn(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_vec());

    tauri::async_runtime::block_on(close_all_connections(api.address, Duration::from_secs(1)))
        .unwrap();

    assert!(
        api.request
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .starts_with("DELETE /connections HTTP/1.1\r\n")
    );
}

struct TestApi {
    address: SocketAddr,
    request: Receiver<String>,
}

impl TestApi {
    fn spawn(response: Vec<u8>) -> Self {
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
            while let Ok(read) = stream.read(&mut chunk) {
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..read]);
                if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let _ = sender.try_send(String::from_utf8_lossy(&bytes).into_owned());
            let _ = stream.write_all(&response);
            let _ = stream.flush();
        });
        Self { address, request }
    }
}
