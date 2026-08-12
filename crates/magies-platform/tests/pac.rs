//! Covers the generated proxy auto-configuration script and the loopback server
//! that hands it to the host.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use magies_platform::pac::{PacScript, PacServer};

#[test]
fn the_global_script_sends_everything_to_the_local_proxy() {
    let script = PacScript::global(10_808, 10_809).to_string();

    // The HTTP proxy first: a browser that cannot speak SOCKS still works, and
    // the SOCKS entry is the fallback rather than the only option.
    assert!(
        script.contains("PROXY 127.0.0.1:10809"),
        "no HTTP proxy in {script}"
    );
    assert!(
        script.contains("SOCKS5 127.0.0.1:10808"),
        "no SOCKS proxy in {script}"
    );
    assert!(script.contains("function FindProxyForURL(url, host)"));
}

#[test]
fn the_global_script_keeps_local_addresses_direct() {
    let script = PacScript::global(10_808, 10_809).to_string();

    // Sending loopback and LAN traffic through the proxy would break printers,
    // routers and anything else served on the same network.
    for expected in [
        "isPlainHostName(host)",
        "localhost",
        "127.0.0.0",
        "10.0.0.0",
        "172.16.0.0",
        "192.168.0.0",
        "return \"DIRECT\"",
    ] {
        assert!(
            script.contains(expected),
            "{expected} missing from {script}"
        );
    }
}

#[test]
fn the_server_serves_the_script_with_the_pac_content_type() {
    let server = PacServer::start(&PacScript::global(10_808, 10_809)).unwrap();

    let response = fetch(&server.url());

    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    // Browsers and `networksetup` both expect this exact type; text/plain is
    // silently ignored by some of them.
    assert!(
        response.contains("Content-Type: application/x-ns-proxy-autoconfig"),
        "{response}"
    );
    assert!(response.contains("FindProxyForURL"), "{response}");
}

#[test]
fn the_server_url_is_loopback_only() {
    let server = PacServer::start(&PacScript::global(10_808, 10_809)).unwrap();

    let url = server.url();

    // A PAC file naming the local proxy ports must not be reachable from the
    // network the host is on.
    assert!(url.starts_with("http://127.0.0.1:"), "{url}");
    assert!(url.ends_with("/proxy.pac"), "{url}");
}

#[test]
fn the_server_answers_more_than_one_request() {
    let server = PacServer::start(&PacScript::global(10_808, 10_809)).unwrap();

    // The host re-reads the file whenever it likes, so one-shot is not enough.
    for _ in 0..3 {
        assert!(fetch(&server.url()).contains("FindProxyForURL"));
    }
}

#[test]
fn stopping_the_server_releases_its_port() {
    let server = PacServer::start(&PacScript::global(10_808, 10_809)).unwrap();
    let url = server.url();

    drop(server);

    let address = url
        .trim_start_matches("http://")
        .trim_end_matches("/proxy.pac")
        .to_owned();
    // Give the accept loop a moment to notice the shutdown.
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        TcpStream::connect(&address).is_err(),
        "the PAC server is still listening on {address}"
    );
}

fn fetch(url: &str) -> String {
    let address = url
        .trim_start_matches("http://")
        .trim_end_matches("/proxy.pac");
    let mut stream = TcpStream::connect(address).expect("the PAC server must accept connections");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(stream, "GET /proxy.pac HTTP/1.1\r\nHost: {address}\r\n\r\n").unwrap();

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let blank = line == "\r\n";
        response.push_str(&line);
        if blank {
            break;
        }
    }
    let mut body = String::new();
    let _ = reader.read_line(&mut body);
    response.push_str(&body);
    // The body arrives in one write, so a single read after the headers is
    // enough to see whether the script is there.
    let mut rest = String::new();
    while reader.read_line(&mut rest).unwrap_or(0) > 0 {
        response.push_str(&rest);
        rest.clear();
        if response.contains("FindProxyForURL") {
            break;
        }
    }
    response
}

#[test]
fn a_request_that_sends_no_headers_still_gets_the_script() {
    let server = PacServer::start(&PacScript::global(10_808, 10_809)).unwrap();
    let address = server
        .url()
        .trim_start_matches("http://")
        .trim_end_matches("/proxy.pac")
        .to_owned();

    // `networksetup` and some browsers close the request without a blank line;
    // the server has to answer anyway rather than wait for one.
    let mut stream = TcpStream::connect(&address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(stream, "GET /proxy.pac HTTP/1.0\r\n\r\n").unwrap();
    let mut response = String::new();
    BufReader::new(stream)
        .read_to_string(&mut response)
        .unwrap();

    assert!(response.contains("FindProxyForURL"), "{response}");
    assert!(response.contains("Content-Length:"), "{response}");
}

#[test]
fn every_server_gets_its_own_port() {
    let first = PacServer::start(&PacScript::global(10_808, 10_809)).unwrap();
    let second = PacServer::start(&PacScript::global(10_808, 10_809)).unwrap();

    // Two sessions must not fight over one port, and the OS picks both.
    assert_ne!(first.url(), second.url());
    assert!(fetch(&first.url()).contains("FindProxyForURL"));
    assert!(fetch(&second.url()).contains("FindProxyForURL"));
}
