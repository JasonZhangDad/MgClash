use std::net::TcpListener;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let mode = arguments.next().expect("mode is required");
    match mode.as_str() {
        "listen" => {
            let port = arguments
                .next()
                .expect("port is required")
                .parse::<u16>()
                .expect("port must be valid");
            let lifetime_ms = arguments
                .next()
                .expect("lifetime is required")
                .parse::<u64>()
                .expect("lifetime must be valid");
            let _listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
            sleep(Duration::from_millis(lifetime_ms));
        }
        "run" => loop {
            sleep(Duration::from_secs(60));
        },
        "exit" => std::process::exit(17),
        _ => panic!("unsupported fake TUN mode"),
    }
}
