use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use magies_desktop_lib::app_settings::AppSettings;
use magies_desktop_lib::dns_settings::DnsSettings;
use magies_desktop_lib::preferences_backup::{PreferencesBackupError, PreferencesBundle};
use magies_desktop_lib::route_settings::{
    DesktopRouteOutbound, RouteRuleKind, RouteRuleSetting, RouteSettings,
};

#[test]
fn preferences_round_trip_through_a_file() {
    let directory = TestDirectory::new("preferences-round-trip");
    let bundle = PreferencesBundle::new(
        AppSettings {
            connect_on_launch: true,
            socks_port: 20_808,
            ..AppSettings::default()
        },
        RouteSettings {
            final_outbound: DesktopRouteOutbound::Direct,
            rules: vec![RouteRuleSetting {
                kind: RouteRuleKind::DomainSuffix,
                value: "example.com".to_owned(),
                outbound: DesktopRouteOutbound::Proxy,
                enabled: true,
            }],
        },
        DnsSettings {
            server: "8.8.8.8".to_owned(),
            ..DnsSettings::default()
        },
    );

    let path = bundle.write_to(directory.path()).unwrap();
    assert!(path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("mgclash-preferences-")));

    let restored = PreferencesBundle::read_from(&path).unwrap();
    assert_eq!(restored, bundle);
}

#[test]
fn an_unsupported_schema_is_a_typed_error() {
    let directory = TestDirectory::new("preferences-schema");
    let mut bundle = PreferencesBundle::new(
        AppSettings::default(),
        RouteSettings::default(),
        DnsSettings::default(),
    );
    let path = bundle.write_to(directory.path()).unwrap();
    bundle.schema = 99;
    fs::write(&path, serde_json::to_vec_pretty(&bundle).unwrap()).unwrap();

    assert!(matches!(
        PreferencesBundle::read_from(&path),
        Err(PreferencesBackupError::UnsupportedSchema { schema: 99 })
    ));
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(name: &str) -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "mgclash-preferences-{name}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
