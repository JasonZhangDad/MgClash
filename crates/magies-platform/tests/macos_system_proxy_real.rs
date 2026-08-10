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
