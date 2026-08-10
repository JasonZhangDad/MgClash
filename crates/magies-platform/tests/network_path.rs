use std::io;

use magies_platform::network_path::{
    NetworkPathProbe, NetworkPathReader, PathCommand, host_path_command,
};

#[test]
fn the_host_command_reads_the_default_route_without_changing_it() {
    let command = host_path_command();

    assert!(!command.program.is_empty());
    // A fingerprint read must never mutate host state.
    for argument in command.arguments {
        assert!(!argument.contains("set"), "unexpected mutation: {argument}");
        assert!(!argument.contains("add"), "unexpected mutation: {argument}");
    }
}

#[test]
#[ignore = "runs the host's real route command and needs a default route"]
fn the_host_reports_a_stable_fingerprint_for_its_real_default_route() {
    let reader = NetworkPathReader::for_host();

    let fingerprint = reader
        .fingerprint()
        .expect("the host must have a readable default route");

    assert!(!fingerprint.is_empty());
    assert_eq!(reader.fingerprint().as_deref(), Some(fingerprint.as_str()));
}

#[test]
fn a_stable_route_produces_a_stable_fingerprint() {
    let reader = NetworkPathReader::with_probe(FakeProbe::ok("default via 192.168.1.1 dev en0"));

    let first = reader.fingerprint();
    let second = reader.fingerprint();

    assert!(first.is_some());
    assert_eq!(first, second);
}

#[test]
fn a_different_route_produces_a_different_fingerprint() {
    let wifi = NetworkPathReader::with_probe(FakeProbe::ok("default via 192.168.1.1 dev en0"));
    let ethernet = NetworkPathReader::with_probe(FakeProbe::ok("default via 10.0.0.1 dev en5"));

    assert_ne!(wifi.fingerprint(), ethernet.fingerprint());
}

#[test]
fn irrelevant_whitespace_does_not_look_like_a_path_change() {
    let tight = NetworkPathReader::with_probe(FakeProbe::ok("default via 192.168.1.1 dev en0"));
    let padded =
        NetworkPathReader::with_probe(FakeProbe::ok("  default via 192.168.1.1  dev en0  \n"));

    assert_eq!(tight.fingerprint(), padded.fingerprint());
}

#[test]
fn an_unreadable_path_reports_nothing_rather_than_a_change() {
    let failed = NetworkPathReader::with_probe(FakeProbe::failed());
    let unsuccessful = NetworkPathReader::with_probe(FakeProbe::exit_code(1, "no route"));

    assert_eq!(failed.fingerprint(), None);
    assert_eq!(unsuccessful.fingerprint(), None);
}

#[test]
fn a_route_that_disappears_is_its_own_fingerprint() {
    let present = NetworkPathReader::with_probe(FakeProbe::ok("default via 192.168.1.1 dev en0"));
    let absent = NetworkPathReader::with_probe(FakeProbe::ok(""));

    assert!(absent.fingerprint().is_some());
    assert_ne!(present.fingerprint(), absent.fingerprint());
}

struct FakeProbe {
    result: Option<(Option<i32>, String)>,
}

impl FakeProbe {
    fn ok(stdout: &str) -> Self {
        Self {
            result: Some((Some(0), stdout.to_owned())),
        }
    }

    fn exit_code(code: i32, stdout: &str) -> Self {
        Self {
            result: Some((Some(code), stdout.to_owned())),
        }
    }

    fn failed() -> Self {
        Self { result: None }
    }
}

impl NetworkPathProbe for FakeProbe {
    fn read(&self, _command: &PathCommand) -> io::Result<(Option<i32>, String)> {
        self.result
            .clone()
            .ok_or_else(|| io::Error::other("probe unavailable"))
    }
}
