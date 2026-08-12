#![cfg(target_os = "macos")]

use magies_platform::macos_system_proxy::MacOsSystemProxyAdapter;

#[test]
#[ignore = "reads the host's real System Proxy state"]
fn reads_real_network_service_without_mutating_it() {
    let service = std::env::var("MAGIES_MACOS_NETWORK_SERVICE")
        .expect("set MAGIES_MACOS_NETWORK_SERVICE to an existing service name");
    let state = MacOsSystemProxyAdapter::new(&service)
        .unwrap()
        .read()
        .unwrap();

    assert!(!format!("{state:?}").contains("://"));
}

/// The point of auto-detection can only be checked against a real host: no env
/// var, no hard-coded `Wi-Fi`, just whatever service currently carries traffic.
#[test]
#[ignore = "reads the host's real default route and System Proxy state"]
fn detects_the_service_on_the_real_default_route() {
    let adapter = MacOsSystemProxyAdapter::for_default_route();

    let snapshot = adapter.read_snapshot().unwrap();

    let service = snapshot.network_service();
    assert!(!service.is_empty(), "detected an empty network service");
    println!("default route is served by {service:?}");
    assert!(!format!("{:?}", snapshot.state()).contains("://"));
}
