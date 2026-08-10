//! A stand-in Core for the soak harness.
//!
//! Unlike `fake_sing_box`, this one actually binds the loopback ports the
//! generated config asks for, so the session's real TCP health check and the
//! recovery probe exercise the same code path they would against sing-box —
//! without needing a pinned binary or a reachable proxy server.
//!
//! Compiled with bare `rustc` at test time, so it cannot use serde: the
//! `listen_port` values are pulled out of the config with string scanning.

use std::env::args_os;
use std::fs::read_to_string;
use std::net::TcpListener;
use std::path::Path;
use std::process::exit;
use std::thread;

fn main() {
    let arguments: Vec<_> = args_os().skip(1).collect();

    if arguments == ["version"] {
        println!("sing-box version 1.13.18");
        return;
    }

    if arguments.len() == 3 && arguments[1] == "-c" {
        let path = Path::new(&arguments[2]);
        let ports = listen_ports(path);
        if arguments[0] == "check" {
            return;
        }
        if arguments[0] == "run" {
            serve(&ports);
            return;
        }
    }

    eprintln!("unsupported soak Core arguments: {arguments:?}");
    exit(64);
}

/// Binds every port the config declares and accepts forever, so the session's
/// health check sees a listener exactly as it would with a real Core.
fn serve(ports: &[u16]) {
    if ports.is_empty() {
        eprintln!("soak Core found no listen_port in the config");
        exit(65);
    }

    let listeners: Vec<_> = ports
        .iter()
        .map(|port| {
            TcpListener::bind(("127.0.0.1", *port)).unwrap_or_else(|error| {
                eprintln!("soak Core could not bind 127.0.0.1:{port}: {error}");
                exit(66);
            })
        })
        .collect();

    let handles: Vec<_> = listeners
        .into_iter()
        .map(|listener| {
            thread::spawn(move || {
                for stream in listener.incoming() {
                    // Closing immediately is enough: the probe only checks that
                    // something accepts the connection.
                    drop(stream);
                }
            })
        })
        .collect();

    for handle in handles {
        let _ = handle.join();
    }
}

fn listen_ports(path: &Path) -> Vec<u16> {
    let contents = read_to_string(path).unwrap_or_else(|error| {
        eprintln!("soak Core failed to read config: {error}");
        exit(34);
    });

    let mut ports = Vec::new();
    for section in contents.split("\"listen_port\"").skip(1) {
        let digits: String = section
            .trim_start()
            .trim_start_matches(':')
            .trim_start()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(port) = digits.parse::<u16>() {
            ports.push(port);
        }
    }
    ports
}
