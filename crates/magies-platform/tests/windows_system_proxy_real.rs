#![cfg(target_os = "windows")]

use magies_platform::windows_system_proxy::WindowsSystemProxyAdapter;

#[test]
fn reads_real_wininet_lan_proxy_without_mutating_it() {
    WindowsSystemProxyAdapter::new().read_snapshot().unwrap();
}

#[test]
#[ignore = "writes the current snapshot back to the ephemeral Windows CI user"]
fn round_trips_real_wininet_lan_proxy() {
    let adapter = WindowsSystemProxyAdapter::new();
    let before = adapter.read_snapshot().unwrap();

    adapter.apply_snapshot(&before).unwrap();

    assert_eq!(adapter.read_snapshot().unwrap(), before);
}
