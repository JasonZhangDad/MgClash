//! A proxy auto-configuration file and the loopback server that hands it out.
//!
//! The host's System Proxy can be pointed at a PAC URL instead of a fixed
//! endpoint, which is v2rayN's Pac 模式. Nothing else in this crate serves
//! anything, so the server is deliberately the smallest thing that answers a
//! `GET`: one thread, one fixed body, no routing and no keep-alive.
//!
//! **A PAC script cannot express the Core's Rule mode.** Rule mode is built from
//! `geoip` / `geosite` matchers, which need databases a browser does not have.
//! What can be written here is Global — everything through the local proxy,
//! except addresses that must stay local — and Direct.

use std::fmt::{Display, Formatter};
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::{io, thread};

/// The path the served script lives at, so the URL is stable.
const PAC_PATH: &str = "/proxy.pac";

/// A generated proxy auto-configuration script.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacScript(String);

impl PacScript {
    /// Everything through the local proxy, except what must stay local.
    ///
    /// The HTTP proxy is offered before SOCKS: a client that cannot speak SOCKS
    /// still works, and the SOCKS entry is a fallback rather than the only
    /// option.
    #[must_use]
    pub fn global(socks_port: u16, http_port: u16) -> Self {
        Self(format!(
            "function FindProxyForURL(url, host) {{\n{LOCAL_DIRECT}\
             \n    return \"PROXY 127.0.0.1:{http_port}; \
             SOCKS5 127.0.0.1:{socks_port}; DIRECT\";\n}}\n"
        ))
    }

    /// Nothing through the proxy.
    #[must_use]
    pub fn direct() -> Self {
        Self("function FindProxyForURL(url, host) {\n    return \"DIRECT\";\n}\n".to_owned())
    }
}

impl Display for PacScript {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The addresses that must never go through the proxy.
///
/// Loopback and the private ranges carry printers, routers and anything else
/// served on the same network; proxying them breaks the host's own LAN.
const LOCAL_DIRECT: &str = r#"    if (
        isPlainHostName(host) ||
        host === "localhost" ||
        shExpMatch(host, "*.local") ||
        isInNet(host, "127.0.0.0", "255.0.0.0") ||
        isInNet(host, "10.0.0.0", "255.0.0.0") ||
        isInNet(host, "172.16.0.0", "255.240.0.0") ||
        isInNet(host, "192.168.0.0", "255.255.0.0") ||
        isInNet(host, "169.254.0.0", "255.255.0.0")
    ) {
        return "DIRECT";
    }"#;

/// Serves one script on loopback for as long as it is alive.
#[derive(Debug)]
pub struct PacServer {
    address: SocketAddr,
    running: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl PacServer {
    /// Binds an OS-assigned loopback port and starts answering requests.
    ///
    /// # Errors
    ///
    /// Returns the bind or address error when the loopback port cannot be
    /// opened.
    pub fn start(script: &PacScript) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        let running = Arc::new(AtomicBool::new(true));
        let body = Arc::new(script.to_string());

        let worker = thread::spawn({
            let running = Arc::clone(&running);
            move || {
                for stream in listener.incoming() {
                    if !running.load(Ordering::Relaxed) {
                        break;
                    }
                    // One unusable connection must not stop the server: the host
                    // retries, and a dead PAC server looks to the user like the
                    // whole session is broken.
                    if let Ok(stream) = stream {
                        answer(&stream, &body);
                    }
                }
            }
        });

        Ok(Self {
            address,
            running,
            worker: Some(worker),
        })
    }

    /// The URL to hand to the host's System Proxy settings.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}{PAC_PATH}", self.address)
    }
}

impl Drop for PacServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        // `incoming` blocks until something connects, so the loop is woken by a
        // connection it will refuse rather than by the flag alone.
        let _ = TcpStream::connect(self.address);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Answers one request with the script, whatever was asked for.
///
/// The server exists to serve one file, so the request line is read only to
/// leave the socket in a state the client expects.
fn answer(stream: &TcpStream, body: &str) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    while reader.read_line(&mut line).unwrap_or(0) > 0 {
        if line == "\r\n" || line == "\n" {
            break;
        }
        line.clear();
    }

    let mut stream = stream;
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\n\
         Content-Type: application/x-ns-proxy-autoconfig\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.flush();
}
