use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use magies_desktop_lib::traffic::{TrafficSampleError, sample_traffic};

#[test]
fn samples_split_traffic_json_from_the_loopback_api() {
    let api = TestApi::spawn(
        b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n",
        &[b"{\"up\":1024,", b"\"down\":2048}\n"],
        Duration::from_millis(5),
    );

    let sample =
        tauri::async_runtime::block_on(sample_traffic(api.address, Duration::from_secs(1)))
            .unwrap();

    assert_eq!(sample.upload_bytes_per_second, 1_024);
    assert_eq!(sample.download_bytes_per_second, 2_048);
    assert!(
        api.request
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .starts_with("GET /traffic HTTP/1.1\r\n")
    );
}

#[test]
fn rejects_invalid_samples_and_unsuccessful_status() {
    let invalid = TestApi::spawn(
        b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n",
        &[b"{\"up\":-1,\"down\":0}\n"],
        Duration::ZERO,
    );
    assert!(matches!(
        tauri::async_runtime::block_on(sample_traffic(invalid.address, Duration::from_secs(1),)),
        Err(TrafficSampleError::InvalidBody(_))
    ));

    let unavailable = TestApi::spawn(
        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n",
        &[],
        Duration::ZERO,
    );
    assert!(matches!(
        tauri::async_runtime::block_on(
            sample_traffic(unavailable.address, Duration::from_secs(1),)
        ),
        Err(TrafficSampleError::HttpStatus { status: 503 })
    ));
}

#[test]
fn times_out_when_the_api_does_not_emit_a_sample() {
    let slow = TestApi::spawn(
        b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n",
        &[b"{\"up\":0,\"down\":0}\n"],
        Duration::from_millis(100),
    );

    assert!(matches!(
        tauri::async_runtime::block_on(sample_traffic(slow.address, Duration::from_millis(10),)),
        Err(TrafficSampleError::TimedOut)
    ));
}

struct TestApi {
    address: SocketAddr,
    request: Receiver<String>,
}

impl TestApi {
    fn spawn(
        headers: &'static [u8],
        chunks: &'static [&'static [u8]],
        chunk_delay: Duration,
    ) -> Self {
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
            let _ = stream.write_all(headers);
            for chunk in chunks {
                thread::sleep(chunk_delay);
                let _ = stream.write_all(chunk);
            }
        });
        Self { address, request }
    }
}
