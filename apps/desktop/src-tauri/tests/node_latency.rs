use std::net::TcpListener;
use std::time::Duration;

use magies_desktop_lib::node_latency::{TcpLatencyError, probe_tcp};

#[test]
fn measures_a_reachable_tcp_endpoint() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();

    let latency = probe_tcp("127.0.0.1", address.port(), Duration::from_secs(1)).unwrap();

    assert!(latency <= 1_000);
}

#[test]
fn reports_a_refused_tcp_endpoint_as_failed() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    assert!(matches!(
        probe_tcp("127.0.0.1", port, Duration::from_secs(1)),
        Err(TcpLatencyError::Connect(_))
    ));
}

#[test]
fn a_zero_timeout_fails_before_connecting() {
    assert!(matches!(
        probe_tcp("127.0.0.1", 443, Duration::ZERO),
        Err(TcpLatencyError::Timeout)
    ));
}
