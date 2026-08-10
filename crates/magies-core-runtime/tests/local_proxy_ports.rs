use std::error::Error;
use std::io;
use std::net::TcpListener;

use magies_core_runtime::{LocalProxyPortChecker, LocalProxyPortError, LocalProxyPortKind};

#[test]
fn accepts_two_available_loopback_ports() {
    let socks = available_port();
    let mut http = available_port();
    while http == socks {
        http = available_port();
    }

    LocalProxyPortChecker::check(socks, http).unwrap();
}

#[test]
fn reports_which_local_proxy_port_is_already_in_use() {
    let occupied = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let socks = occupied.local_addr().unwrap().port();

    let error = LocalProxyPortChecker::check(socks, different_port(socks)).unwrap_err();

    assert_eq!(error.kind(), Some(LocalProxyPortKind::Socks));
    assert_eq!(error.port(), socks);
    assert_eq!(
        error
            .source()
            .and_then(|source| source.downcast_ref::<io::Error>())
            .map(io::Error::kind),
        Some(io::ErrorKind::AddrInUse)
    );
    assert!(error.to_string().contains(&socks.to_string()));
}

#[test]
fn rejects_a_shared_socks_and_http_port_before_binding() {
    assert!(matches!(
        LocalProxyPortChecker::check(10_808, 10_808),
        Err(LocalProxyPortError::DuplicatePort { port: 10_808 })
    ));
}

#[test]
fn rejects_zero_instead_of_treating_it_as_an_ephemeral_port() {
    assert!(matches!(
        LocalProxyPortChecker::check(0, 10_809),
        Err(LocalProxyPortError::InvalidPort {
            kind: LocalProxyPortKind::Socks
        })
    ));
    assert!(matches!(
        LocalProxyPortChecker::check(10_808, 0),
        Err(LocalProxyPortError::InvalidPort {
            kind: LocalProxyPortKind::Http
        })
    ));
}

fn available_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn different_port(port: u16) -> u16 {
    if port == u16::MAX { port - 1 } else { port + 1 }
}
