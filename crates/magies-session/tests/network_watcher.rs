use std::time::{Duration, SystemTime};

use magies_session::{NetworkEvent, NetworkWatcher};

const TICK: Duration = Duration::from_secs(2);
const SLEEP_THRESHOLD: Duration = Duration::from_secs(10);

fn watcher() -> NetworkWatcher {
    NetworkWatcher::new(TICK, SLEEP_THRESHOLD)
}

#[test]
fn the_first_tick_only_records_a_baseline() {
    let mut watcher = watcher();

    assert_eq!(watcher.tick(SystemTime::UNIX_EPOCH, Some("wifi-a")), None);
}

#[test]
fn a_steady_network_never_reports_an_event() {
    let start = SystemTime::UNIX_EPOCH;
    let mut watcher = watcher();
    watcher.tick(start, Some("wifi-a"));

    for step in 1..5 {
        assert_eq!(watcher.tick(start + TICK * step, Some("wifi-a")), None);
    }
}

#[test]
fn a_changed_fingerprint_reports_a_path_change() {
    let start = SystemTime::UNIX_EPOCH;
    let mut watcher = watcher();
    watcher.tick(start, Some("wifi-a"));

    assert_eq!(
        watcher.tick(start + TICK, Some("ethernet-b")),
        Some(NetworkEvent::PathChanged)
    );
    // The new fingerprint becomes the baseline, so the change is not repeated.
    assert_eq!(watcher.tick(start + TICK * 2, Some("ethernet-b")), None);
}

#[test]
fn a_wall_clock_gap_larger_than_the_threshold_reports_a_wake() {
    let start = SystemTime::UNIX_EPOCH;
    let mut watcher = watcher();
    watcher.tick(start, Some("wifi-a"));

    assert_eq!(
        watcher.tick(start + SLEEP_THRESHOLD, Some("wifi-a")),
        Some(NetworkEvent::Woke)
    );
}

#[test]
fn a_wake_outranks_a_path_change_in_the_same_tick() {
    let start = SystemTime::UNIX_EPOCH;
    let mut watcher = watcher();
    watcher.tick(start, Some("wifi-a"));

    assert_eq!(
        watcher.tick(start + SLEEP_THRESHOLD, Some("hotspot-c")),
        Some(NetworkEvent::Woke)
    );
    // The fingerprint seen during the wake still becomes the new baseline.
    assert_eq!(
        watcher.tick(start + SLEEP_THRESHOLD + TICK, Some("hotspot-c")),
        None
    );
}

#[test]
fn a_backwards_clock_reports_a_wake_rather_than_panicking() {
    let start = SystemTime::UNIX_EPOCH + Duration::from_secs(3_600);
    let mut watcher = watcher();
    watcher.tick(start, Some("wifi-a"));

    assert_eq!(
        watcher.tick(start - Duration::from_secs(600), Some("wifi-a")),
        Some(NetworkEvent::Woke)
    );
}

#[test]
fn an_unreadable_fingerprint_keeps_the_last_known_one() {
    let start = SystemTime::UNIX_EPOCH;
    let mut watcher = watcher();
    watcher.tick(start, Some("wifi-a"));

    // A failed read must not be mistaken for a path change in either direction.
    assert_eq!(watcher.tick(start + TICK, None), None);
    assert_eq!(watcher.tick(start + TICK * 2, Some("wifi-a")), None);
    assert_eq!(
        watcher.tick(start + TICK * 3, Some("ethernet-b")),
        Some(NetworkEvent::PathChanged)
    );
}

#[test]
fn a_first_successful_read_after_failures_only_sets_the_baseline() {
    let start = SystemTime::UNIX_EPOCH;
    let mut watcher = watcher();

    assert_eq!(watcher.tick(start, None), None);
    assert_eq!(watcher.tick(start + TICK, Some("wifi-a")), None);
    assert_eq!(
        watcher.tick(start + TICK * 2, Some("ethernet-b")),
        Some(NetworkEvent::PathChanged)
    );
}

#[test]
fn the_tick_interval_is_reported_for_the_driver_loop() {
    assert_eq!(watcher().tick_interval(), TICK);
}
