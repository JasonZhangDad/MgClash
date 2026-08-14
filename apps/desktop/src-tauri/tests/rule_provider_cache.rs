use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::process::id;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use magies_desktop_lib::rule_provider_cache::{RuleProviderCache, RuleProviderCacheError};

static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

fn cache_directory(name: &str) -> PathBuf {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("mgclash-rule-cache-{name}-{}-{sequence}", id()))
}

#[test]
fn reports_an_uncached_provider_before_anything_is_downloaded() {
    let directory = cache_directory("empty");
    let cache = RuleProviderCache::open(&directory).unwrap();

    let entry = cache.entry("ads").unwrap();

    assert_eq!(entry.name, "ads");
    assert!(!entry.cached);
    assert_eq!(entry.bytes, 0);
    assert_eq!(entry.updated_at, None);
    assert!(entry.path.ends_with("ads.srs"));
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn refuses_a_name_that_would_escape_the_cache_directory() {
    let directory = cache_directory("unsafe");
    let cache = RuleProviderCache::open(&directory).unwrap();

    for name in ["../etc/passwd", "ads/../..", "", "  "] {
        assert!(matches!(
            cache.entry(name),
            Err(RuleProviderCacheError::InvalidName { .. })
        ));
    }
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn downloads_a_rule_set_and_replaces_the_earlier_copy() {
    let directory = cache_directory("download");
    let cache = RuleProviderCache::open(&directory).unwrap();
    let first = TestServer::spawn(
        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nfirst".to_vec(),
    );

    let entry = tauri::async_runtime::block_on(
        cache.update("ads", &format!("http://{}/ads.srs", first.address)),
    )
    .unwrap();

    assert!(entry.cached);
    assert_eq!(entry.bytes, 5);
    assert!(entry.updated_at.is_some());
    assert_eq!(fs::read(&entry.path).unwrap(), b"first");

    let second = TestServer::spawn(
        b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nsecond".to_vec(),
    );
    let entry = tauri::async_runtime::block_on(
        cache.update("ads", &format!("http://{}/ads.srs", second.address)),
    )
    .unwrap();

    assert_eq!(fs::read(&entry.path).unwrap(), b"second");
    // The temporary the download goes through must not be left behind.
    assert!(
        !PathBuf::from(&entry.path)
            .with_extension("partial")
            .exists()
    );
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn a_failed_download_keeps_the_copy_that_works() {
    let directory = cache_directory("failure");
    let cache = RuleProviderCache::open(&directory).unwrap();
    let good = TestServer::spawn(
        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nrules".to_vec(),
    );
    tauri::async_runtime::block_on(
        cache.update("ads", &format!("http://{}/ads.srs", good.address)),
    )
    .unwrap();

    let missing =
        TestServer::spawn(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec());
    let error = tauri::async_runtime::block_on(
        cache.update("ads", &format!("http://{}/ads.srs", missing.address)),
    )
    .unwrap_err();

    assert_eq!(error.code(), "rule_set_download_failed");
    assert_eq!(fs::read(cache.path_for("ads").unwrap()).unwrap(), b"rules");
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn an_empty_body_is_not_a_rule_set() {
    let directory = cache_directory("empty-body");
    let cache = RuleProviderCache::open(&directory).unwrap();
    let server = TestServer::spawn(
        b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
    );

    let error = tauri::async_runtime::block_on(
        cache.update("ads", &format!("http://{}/ads.srs", server.address)),
    )
    .unwrap_err();

    assert!(matches!(error, RuleProviderCacheError::EmptyBody { .. }));
    assert!(!cache.entry("ads").unwrap().cached);
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn forgetting_a_provider_removes_its_file_and_is_idempotent() {
    let directory = cache_directory("forget");
    let cache = RuleProviderCache::open(&directory).unwrap();
    fs::write(cache.path_for("ads").unwrap(), b"rules").unwrap();

    cache.forget("ads").unwrap();
    assert!(!cache.entry("ads").unwrap().cached);
    // Deleting a provider that was never downloaded is not a failure.
    cache.forget("ads").unwrap();
    fs::remove_dir_all(&directory).unwrap();
}

struct TestServer {
    address: SocketAddr,
}

impl TestServer {
    fn spawn(response: Vec<u8>) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
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
            let _ = stream.write_all(&response);
            let _ = stream.flush();
        });
        Self { address }
    }
}
