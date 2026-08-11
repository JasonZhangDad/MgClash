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
fn reports_an_unreachable_tcp_endpoint_without_latency() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let error = probe_tcp("127.0.0.1", port, Duration::from_secs(1)).unwrap_err();

    assert!(matches!(
        error,
        TcpLatencyError::Connect(_) | TcpLatencyError::Timeout
    ));
}

#[test]
fn a_zero_timeout_fails_before_connecting() {
    assert!(matches!(
        probe_tcp("127.0.0.1", 443, Duration::ZERO),
        Err(TcpLatencyError::Timeout)
    ));
}
