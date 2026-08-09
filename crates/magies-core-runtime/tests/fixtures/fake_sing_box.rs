use std::env::args_os;
use std::fs::read_to_string;
use std::path::Path;
use std::process::exit;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    let arguments: Vec<_> = args_os().skip(1).collect();
    let executable_name = std::env::current_exe()
        .ok()
        .and_then(|path| path.file_name().map(|name| name.to_owned()))
        .unwrap_or_default();

    if arguments == ["version"] {
        if executable_name.to_string_lossy().contains("invalid-version") {
            println!("not sing-box");
        } else {
            println!("sing-box version 1.13.18");
        }
        return;
    }

    if arguments.len() == 3 && arguments[0] == "check" && arguments[1] == "-c" {
        validate_config(Path::new(&arguments[2]));
        return;
    }

    if arguments.len() == 3 && arguments[0] == "run" && arguments[1] == "-c" {
        validate_config(Path::new(&arguments[2]));
        sleep(Duration::from_secs(60));
        return;
    }

    eprintln!("unsupported fake sing-box arguments: {arguments:?}");
    exit(64);
}

fn validate_config(path: &Path) {
    let contents = read_to_string(path).unwrap_or_else(|error| {
        eprintln!("failed to read config: {error}");
        exit(34);
    });
    if !contents.contains("\"valid\": true") {
        eprintln!("invalid config");
        exit(33);
    }
}
