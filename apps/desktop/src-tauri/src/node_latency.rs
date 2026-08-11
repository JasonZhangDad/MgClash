//! Bounded TCP connect latency tests for desktop proxy nodes.

use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

/// Why a node endpoint could not complete a TCP connect test.
#[derive(Debug, Error)]
pub enum TcpLatencyError {
    #[error("the node address could not be resolved")]
    Resolve(#[source] io::Error),
    #[error("the node address resolved to no endpoints")]
    NoAddress,
    #[error("the node endpoint rejected the TCP connection")]
    Connect(#[source] io::Error),
    #[error("the TCP connection test timed out")]
    Timeout,
    #[error("the TCP connection test worker stopped unexpectedly")]
    WorkerStopped,
}

/// Measures one TCP connection in milliseconds, bounded by `timeout` even if
/// host name resolution stalls.
///
/// # Errors
///
/// Returns a typed resolution, connection, timeout, or worker error.
pub fn probe_tcp(server: &str, port: u16, timeout: Duration) -> Result<u32, TcpLatencyError> {
    if timeout.is_zero() {
        return Err(TcpLatencyError::Timeout);
    }

    let server = server.to_owned();
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let result = probe_tcp_inner(&server, port, timeout);
        let _ = sender.send(result);
    });

    match receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => Err(TcpLatencyError::Timeout),
        Err(RecvTimeoutError::Disconnected) => Err(TcpLatencyError::WorkerStopped),
    }
}

fn probe_tcp_inner(server: &str, port: u16, timeout: Duration) -> Result<u32, TcpLatencyError> {
    let started = Instant::now();
    let addresses = (server, port)
        .to_socket_addrs()
        .map_err(TcpLatencyError::Resolve)?;
    let mut last_error = None;
    let mut had_address = false;

    for address in addresses {
        had_address = true;
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(TcpLatencyError::Timeout);
        }
        match TcpStream::connect_timeout(&address, remaining) {
            Ok(_stream) => {
                return Ok(u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX));
            }
            Err(error) => last_error = Some(error),
        }
    }

    if !had_address {
        return Err(TcpLatencyError::NoAddress);
    }
    let error = last_error.expect("a tested address always has a connection result");
    if error.kind() == io::ErrorKind::TimedOut || started.elapsed() >= timeout {
        Err(TcpLatencyError::Timeout)
    } else {
        Err(TcpLatencyError::Connect(error))
    }
}
