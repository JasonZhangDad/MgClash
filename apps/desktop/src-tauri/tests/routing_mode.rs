use std::fs;
use std::process::id;
use std::sync::atomic::{AtomicUsize, Ordering};

use magies_desktop_lib::routing_mode::{
    RoutingModeStoreError, SqliteRoutingModeStore, parse_routing_mode, route_profile_for,
};
use magies_routing::{RoutingMode, SingBoxRouteConfigGenerator};
use rusqlite::Connection;

static DATABASE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

#[test]
fn accepts_only_the_three_desktop_routing_modes() {
    assert_eq!(parse_routing_mode("global").unwrap(), RoutingMode::Global);
    assert_eq!(parse_routing_mode("rule").unwrap(), RoutingMode::Rule);
    assert_eq!(parse_routing_mode("direct").unwrap(), RoutingMode::Direct);
    assert!(parse_routing_mode("Global").is_err());
    assert!(parse_routing_mode("unknown").is_err());
}

#[test]
fn builds_the_expected_route_for_each_mode() {
    for (mode, expected_final) in [
        (RoutingMode::Global, "proxy"),
        (RoutingMode::Rule, "proxy"),
        (RoutingMode::Direct, "direct"),
    ] {
        let profile = route_profile_for(mode);
        let generated = SingBoxRouteConfigGenerator::generate(&profile);

        assert_eq!(profile.mode(), mode);
        assert_eq!(generated.json()["final"], expected_final);
    }
}

#[test]
fn defaults_to_global_and_persists_the_selected_mode() {
    let path = database_path("persists");
    let store = SqliteRoutingModeStore::open(&path).unwrap();
    assert_eq!(store.load().unwrap(), RoutingMode::Global);

    store.save(RoutingMode::Direct).unwrap();
    drop(store);

    let reopened = SqliteRoutingModeStore::open(&path).unwrap();
    assert_eq!(reopened.load().unwrap(), RoutingMode::Direct);

    drop(reopened);
    fs::remove_file(path).unwrap();
}

#[test]
fn rejects_an_unsupported_value_already_in_the_database() {
    let path = database_path("invalid");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE routing_mode (id INTEGER PRIMARY KEY, mode TEXT NOT NULL);
             INSERT INTO routing_mode (id, mode) VALUES (1, 'unknown');",
        )
        .unwrap();
    drop(connection);

    let store = SqliteRoutingModeStore::open(&path).unwrap();
    assert!(matches!(
        store.load(),
        Err(RoutingModeStoreError::InvalidStoredValue { value }) if value == "unknown"
    ));

    drop(store);
    fs::remove_file(path).unwrap();
}

fn database_path(name: &str) -> std::path::PathBuf {
    let sequence = DATABASE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mgclash-routing-mode-{name}-{}-{sequence}.sqlite",
        id()
    ))
}
