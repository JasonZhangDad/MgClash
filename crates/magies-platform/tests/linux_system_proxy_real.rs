#![cfg(target_os = "linux")]

use magies_platform::linux_system_proxy::LinuxSystemProxyAdapter;

#[test]
fn reads_real_gnome_proxy_without_mutating_it() {
    LinuxSystemProxyAdapter::new()
        .unwrap()
        .read_snapshot()
        .unwrap();
}

#[test]
#[ignore = "writes through the in-memory GSettings backend in Linux CI"]
fn round_trips_real_gsettings_snapshot() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    let adapter = LinuxSystemProxyAdapter::new().unwrap();
    let before = adapter.read_snapshot().unwrap();

    adapter.apply_snapshot(&before).unwrap();

    assert_eq!(adapter.read_snapshot().unwrap(), before);
}
