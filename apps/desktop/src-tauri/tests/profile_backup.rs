use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use magies_desktop_lib::app_settings::AppSettings;
use magies_desktop_lib::dns_settings::DnsSettings;
use magies_desktop_lib::profile_backup::{ProfileBackupError, ProfileBundle, ProfileNodesData};
use magies_desktop_lib::route_settings::RouteSettings;
use magies_desktop_lib::subscriptions::SubscriptionBackupEntry;
use uuid::Uuid;

#[test]
fn profile_round_trip_through_a_file() {
    let directory = TestDirectory::new("profile-round-trip");
    let subscription_id = Uuid::from_u128(42);
    let bundle = ProfileBundle::new(
        AppSettings::default(),
        RouteSettings::default(),
        DnsSettings::default(),
        ProfileNodesData {
            manual_nodes: vec![],
            groups: vec![],
            node_order: vec![],
            selected_node_id: None,
        },
        vec![SubscriptionBackupEntry {
            id: subscription_id,
            name: "Primary".to_owned(),
            url: "https://example.com/list".to_owned(),
            update_interval_minutes: 60,
            auto_update: true,
            enabled: true,
            user_agent: None,
            include_keywords: String::new(),
            exclude_keywords: String::new(),
            subconverter_url: None,
        }],
    );

    let path = bundle.write_to(directory.path()).unwrap();
    assert!(path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("mgclash-profile-")));

    let restored = ProfileBundle::read_from(&path).unwrap();
    assert_eq!(restored, bundle);
}

#[test]
fn an_unsupported_profile_schema_is_a_typed_error() {
    let directory = TestDirectory::new("profile-schema");
    let mut bundle = ProfileBundle::new(
        AppSettings::default(),
        RouteSettings::default(),
        DnsSettings::default(),
        ProfileNodesData {
            manual_nodes: vec![],
            groups: vec![],
            node_order: vec![],
            selected_node_id: None,
        },
        vec![],
    );
    let path = bundle.write_to(directory.path()).unwrap();
    bundle.schema = 99;
    fs::write(&path, serde_json::to_vec_pretty(&bundle).unwrap()).unwrap();

    assert!(matches!(
        ProfileBundle::read_from(&path),
        Err(ProfileBackupError::UnsupportedSchema { schema: 99 })
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
            "mgclash-profile-{name}-{}-{id}",
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
