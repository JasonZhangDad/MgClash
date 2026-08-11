use magies_desktop_lib::platform_proxy::{
    PlatformProxyControl, PlatformProxyError, PlatformProxySetupError, SystemProxyStartupStatus,
    startup_status_from,
};
use magies_platform::system_proxy::{PacSetting, ProxyEndpoint, ProxySetting, SystemProxyState};
use magies_platform::system_proxy_recovery::StartupRecovery;
use magies_session::SystemProxySessionControl;

#[test]
fn an_unavailable_adapter_is_reported_when_the_user_connects() {
    let mut control = unavailable();

    assert!(matches!(
        control.enable(&managed_state()),
        Err(PlatformProxyError::Setup(_))
    ));
}

#[test]
fn stopping_an_unavailable_adapter_never_blocks_shutdown() {
    let mut control = unavailable();

    control.stop().unwrap();
}

#[test]
fn the_setup_error_keeps_the_platform_cause_for_the_log() {
    let error = PlatformProxySetupError::new("GNOME proxy schema is absent");

    assert!(error.to_string().contains("GNOME proxy schema is absent"));
}

#[test]
fn an_unavailable_adapter_carries_a_stable_code_for_the_ui() {
    let mut control = unavailable();

    assert_eq!(
        control.enable(&managed_state()).unwrap_err().code(),
        "system_proxy_unavailable"
    );
}

#[test]
fn startup_recovery_states_have_stable_ui_values() {
    assert_eq!(
        startup_status_from(StartupRecovery::Clean),
        SystemProxyStartupStatus::Clean
    );
    assert_eq!(
        startup_status_from(StartupRecovery::RestoreRequired),
        SystemProxyStartupStatus::RestoreRequired
    );
    assert_eq!(
        startup_status_from(StartupRecovery::ManagedCoreRunning),
        SystemProxyStartupStatus::Clean
    );
}

#[test]
fn an_unavailable_adapter_does_not_block_app_startup() {
    assert_eq!(
        unavailable().startup_status().unwrap(),
        SystemProxyStartupStatus::Clean
    );
}

fn unavailable() -> PlatformProxyControl {
    PlatformProxyControl::new(Err(PlatformProxySetupError::new(
        "no System Proxy adapter in this test",
    )))
}

fn managed_state() -> SystemProxyState {
    let endpoint = ProxyEndpoint::new("127.0.0.1", 10_809).unwrap();
    let setting = ProxySetting::new(true, Some(endpoint)).unwrap();
    SystemProxyState::new(
        setting.clone(),
        setting.clone(),
        setting,
        PacSetting::disabled(),
    )
}
