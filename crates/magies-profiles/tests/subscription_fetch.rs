use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use magies_profiles::{
    SubscriptionFetchError, SubscriptionFetchOptions, SubscriptionFetchResult, SubscriptionFetcher,
    SubscriptionValidators,
};

const LAST_MODIFIED: &str = "Sun, 09 Aug 2026 12:00:00 GMT";

#[test]
fn fetches_updated_content_with_validators_and_a_named_user_agent() {
    let body = b"vless://node-secret@example.com:443";
    let server = TestServer::spawn(vec![TestResponse::immediate(response(
        "200 OK",
        &[("ETag", "\"revision-1\""), ("Last-Modified", LAST_MODIFIED)],
        body,
    ))]);
    let url = server.url("/subscription?token=url-secret");

    let result = fetch(&default_fetcher(), &url, None).unwrap();

    assert!(!result.is_not_modified());
    assert_eq!(result.content(), Some(body.as_slice()));
    assert_eq!(result.validators().etag(), Some("\"revision-1\""));
    assert_eq!(result.validators().last_modified(), Some(LAST_MODIFIED));
    let debug_output = format!("{result:?}");
    assert!(debug_output.contains("[REDACTED]"));
    assert!(!debug_output.contains("node-secret"));
    assert!(!debug_output.contains("url-secret"));

    let requests = server.finish();
    let request = requests[0].to_ascii_lowercase();
    assert!(request.starts_with("get /subscription?token=url-secret http/1.1\r\n"));
    assert!(request.contains("user-agent: mgclash/0.1.0\r\n"));
}

#[test]
fn sends_conditional_headers_and_preserves_missing_validators_on_not_modified() {
    let server = TestServer::spawn(vec![TestResponse::immediate(response(
        "304 Not Modified",
        &[("ETag", "\"revision-1\"")],
        b"",
    ))]);
    let previous = SubscriptionValidators::new(
        Some("\"old-revision\"".to_owned()),
        Some(LAST_MODIFIED.to_owned()),
    );

    let result = fetch(
        &default_fetcher(),
        &server.url("/subscription"),
        Some(&previous),
    )
    .unwrap();

    assert!(result.is_not_modified());
    assert_eq!(result.content(), None);
    assert_eq!(result.validators().etag(), Some("\"revision-1\""));
    assert_eq!(result.validators().last_modified(), Some(LAST_MODIFIED));
    let request = server.finish()[0].to_ascii_lowercase();
    assert!(request.contains("if-none-match: \"old-revision\"\r\n"));
    assert!(request.contains(&format!(
        "if-modified-since: {}\r\n",
        LAST_MODIFIED.to_ascii_lowercase()
    )));
}

#[test]
fn follows_bounded_redirects() {
    let server = TestServer::spawn(vec![
        TestResponse::immediate(response("302 Found", &[("Location", "/final")], b"")),
        TestResponse::immediate(response("200 OK", &[], b"ss://updated")),
    ]);

    let result = fetch(&default_fetcher(), &server.url("/start"), None).unwrap();

    assert_eq!(result.content(), Some(b"ss://updated".as_slice()));
    let requests = server.finish();
    assert!(requests[0].starts_with("GET /start HTTP/1.1\r\n"));
    assert!(requests[1].starts_with("GET /final HTTP/1.1\r\n"));
}

#[test]
fn rejects_non_success_status_without_reading_or_logging_the_body() {
    let server = TestServer::spawn(vec![TestResponse::immediate(response(
        "503 Service Unavailable",
        &[],
        b"server-body-secret",
    ))]);
    let url = server.url("/subscription?token=url-secret");

    let error = fetch(&default_fetcher(), &url, None).unwrap_err();

    assert!(matches!(
        error,
        SubscriptionFetchError::HttpStatus { status: 503 }
    ));
    let debug_output = format!("{error:?}");
    assert!(!debug_output.contains("server-body-secret"));
    assert!(!debug_output.contains("url-secret"));
    server.finish();
}

#[test]
fn enforces_the_total_request_timeout() {
    let server = TestServer::spawn(vec![TestResponse::delayed(
        Duration::from_millis(150),
        response("200 OK", &[], b"late"),
    )]);
    let options = SubscriptionFetchOptions {
        timeout: Duration::from_millis(30),
        ..SubscriptionFetchOptions::default()
    };
    let fetcher = SubscriptionFetcher::new(options).unwrap();
    let url = server.url("/subscription?token=timeout-secret");

    let error = fetch(&fetcher, &url, None).unwrap_err();

    assert!(matches!(error, SubscriptionFetchError::TimedOut));
    assert!(!format!("{error:?}").contains("timeout-secret"));
    server.finish();
}

#[test]
fn rejects_oversized_content_length_before_reading_the_body() {
    let server = TestServer::spawn(vec![TestResponse::immediate(response(
        "200 OK",
        &[("Content-Length", "20")],
        b"01234567890123456789",
    ))]);
    let options = SubscriptionFetchOptions {
        max_body_bytes: 5,
        ..SubscriptionFetchOptions::default()
    };
    let fetcher = SubscriptionFetcher::new(options).unwrap();

    let error = fetch(&fetcher, &server.url("/large"), None).unwrap_err();

    assert!(matches!(
        error,
        SubscriptionFetchError::ResponseTooLarge { max_bytes: 5 }
    ));
    server.finish();
}

#[test]
fn rejects_oversized_chunked_bodies_when_length_is_unknown() {
    let raw_response = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\
                         Connection: close\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n"
        .to_vec();
    let server = TestServer::spawn(vec![TestResponse::immediate(raw_response)]);
    let options = SubscriptionFetchOptions {
        max_body_bytes: 10,
        ..SubscriptionFetchOptions::default()
    };
    let fetcher = SubscriptionFetcher::new(options).unwrap();

    let error = fetch(&fetcher, &server.url("/chunked"), None).unwrap_err();

    assert!(matches!(
        error,
        SubscriptionFetchError::ResponseTooLarge { max_bytes: 10 }
    ));
    server.finish();
}

#[test]
fn validates_options_urls_schemes_and_conditional_headers() {
    let zero_timeout = SubscriptionFetcher::new(SubscriptionFetchOptions {
        timeout: Duration::ZERO,
        ..SubscriptionFetchOptions::default()
    })
    .unwrap_err();
    let zero_body_limit = SubscriptionFetcher::new(SubscriptionFetchOptions {
        max_body_bytes: 0,
        ..SubscriptionFetchOptions::default()
    })
    .unwrap_err();
    let fetcher = default_fetcher();
    let malformed_url = fetch(&fetcher, "not a URL", None).unwrap_err();
    let unsupported_scheme = fetch(&fetcher, "file:///tmp/subscription", None).unwrap_err();
    let invalid_validators = SubscriptionValidators::new(Some("bad\nvalue".to_owned()), None);
    let invalid_header = fetch(
        &fetcher,
        "https://example.com/subscription",
        Some(&invalid_validators),
    )
    .unwrap_err();

    assert!(matches!(
        zero_timeout,
        SubscriptionFetchError::InvalidTimeout
    ));
    assert!(matches!(
        zero_body_limit,
        SubscriptionFetchError::InvalidBodyLimit
    ));
    assert!(matches!(
        malformed_url,
        SubscriptionFetchError::InvalidUrl { .. }
    ));
    assert!(matches!(
        &unsupported_scheme,
        SubscriptionFetchError::UnsupportedScheme { scheme } if scheme == "file"
    ));
    assert!(matches!(
        invalid_header,
        SubscriptionFetchError::InvalidRequestHeader {
            name: "If-None-Match"
        }
    ));
}

#[test]
fn strips_sensitive_urls_from_network_and_redirect_errors() {
    let network_server = TestServer::spawn(vec![TestResponse::immediate(response(
        "200 OK",
        &[("Content-Length", "5")],
        b"ok",
    ))]);
    let network_url = network_server.url("/subscription?token=network-secret");
    let network_error = fetch(&default_fetcher(), &network_url, None).unwrap_err();

    assert!(
        matches!(network_error, SubscriptionFetchError::RequestFailed { .. }),
        "unexpected network error: {network_error:?}"
    );
    assert!(!format!("{network_error:?}").contains("network-secret"));
    network_server.finish();

    let redirect_server = TestServer::spawn(vec![
        TestResponse::immediate(response("302 Found", &[("Location", "/again")], b"")),
        TestResponse::immediate(response("302 Found", &[("Location", "/third")], b"")),
    ]);
    let options = SubscriptionFetchOptions {
        max_redirects: 1,
        ..SubscriptionFetchOptions::default()
    };
    let fetcher = SubscriptionFetcher::new(options).unwrap();
    let redirect_url = redirect_server.url("/start?token=redirect-secret");
    let redirect_error = fetch(&fetcher, &redirect_url, None).unwrap_err();

    assert!(matches!(
        redirect_error,
        SubscriptionFetchError::RedirectLimitExceeded
    ));
    assert!(!format!("{redirect_error:?}").contains("redirect-secret"));
    redirect_server.finish();
}

#[test]
fn rejects_non_text_response_validators_without_exposing_them() {
    let mut raw_response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nETag: ".to_vec();
    raw_response.push(0xff);
    raw_response.extend_from_slice(b"\r\nConnection: close\r\n\r\nok");
    let server = TestServer::spawn(vec![TestResponse::immediate(raw_response)]);

    let error = fetch(&default_fetcher(), &server.url("/subscription"), None).unwrap_err();

    assert!(matches!(
        error,
        SubscriptionFetchError::InvalidResponseHeader { name: "ETag" }
    ));
    assert!(!format!("{error:?}").contains('ÿ'));
    server.finish();
}

fn default_fetcher() -> SubscriptionFetcher {
    SubscriptionFetcher::new(SubscriptionFetchOptions::default()).unwrap()
}

fn fetch(
    fetcher: &SubscriptionFetcher,
    url: &str,
    validators: Option<&SubscriptionValidators>,
) -> Result<SubscriptionFetchResult, SubscriptionFetchError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fetcher.fetch(url, validators))
}

struct TestServer {
    address: std::net::SocketAddr,
    requests: Receiver<String>,
    handle: JoinHandle<()>,
}

impl TestServer {
    fn spawn(responses: Vec<TestResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, requests) = mpsc::channel();
        let handle = thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_request(&mut stream);
                sender.send(request).unwrap();
                thread::sleep(response.delay);
                let _ = stream.write_all(&response.bytes);
            }
        });
        Self {
            address,
            requests,
            handle,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    fn finish(self) -> Vec<String> {
        self.handle.join().unwrap();
        self.requests.try_iter().collect()
    }
}

struct TestResponse {
    delay: Duration,
    bytes: Vec<u8>,
}

impl TestResponse {
    fn immediate(bytes: Vec<u8>) -> Self {
        Self {
            delay: Duration::ZERO,
            bytes,
        }
    }

    fn delayed(delay: Duration, bytes: Vec<u8>) -> Self {
        Self { delay, bytes }
    }
}

fn response(status: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut response = format!("HTTP/1.1 {status}\r\nConnection: close\r\n").into_bytes();
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-length"))
    {
        response.extend_from_slice(format!("Content-Length: {}\r\n", body.len()).as_bytes());
    }
    for (name, value) in headers {
        response.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
    }
    response.extend_from_slice(b"\r\n");
    response.extend_from_slice(body);
    response
}

fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        let count = stream.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..count]);
    }
    String::from_utf8(request).unwrap()
}
