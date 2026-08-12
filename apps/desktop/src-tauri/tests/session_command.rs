use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::id;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use magies_desktop_lib::dns_settings::{
    DesktopDnsStrategy, DnsMode, DnsSettings, SqliteDnsSettingsStore,
};
use magies_desktop_lib::route_settings::{
    DesktopRouteOutbound, RouteRuleKind, RouteRuleSetting, RouteSettings, SqliteRouteSettingsStore,
};
use magies_desktop_lib::routing_mode::SqliteRoutingModeStore;
use magies_desktop_lib::session::{SessionCommandError, SessionDefaults, SessionService};
use magies_domain::{CredentialRef, ProxyProtocol, Subscription, TimestampMillis};
use magies_platform::system_proxy::SystemProxyState;
use magies_profiles::{
    CredentialCodec, LocalHttpProfile, LocalSocksProfile, SqliteManualNodeStore,
    SqliteSubscriptionStore, SubscriptionContentParser, SubscriptionUpdate, SubscriptionValidators,
};
use magies_routing::RoutingMode;
use magies_session::{
    CoreSessionControl, DesktopSession, NetworkEvent, RecoveryOutcome, SessionHealthProbe,
    SystemProxySessionControl,
};
use magies_storage::{MemorySecretStore, SecretStore};
use uuid::Uuid;

const SHADOWSOCKS_LINK: &str = "ss://aes-128-gcm:runtime-secret@edge.example.com:8388#Tokyo%20Edge";

#[test]
fn reports_an_idle_status_before_a_node_is_imported() {
    let (service, _runtime, _fail_start) = service();
    let status = service.status();

    assert!(!status.connected);
    assert!(status.node.is_none());
    assert_eq!(status.core, "sing-box");
    assert_eq!(status.dns, DnsSettings::default());
    assert_eq!(status.mode, "global");
    assert_eq!(status.route, RouteSettings::default());
    assert!(status.system_proxy);
    assert_eq!(status.socks_port, 10_808);
    assert_eq!(status.http_port, 10_809);
}

#[test]
fn importing_a_share_link_stores_the_credential_and_selects_the_node() {
    let (mut service, _runtime, _fail_start) = service();

    let status = service.import_node(SHADOWSOCKS_LINK).unwrap();

    let node = status.node.as_ref().unwrap();
    assert_eq!(node.name, "Tokyo Edge");
    assert_eq!(node.protocol, ProxyProtocol::Shadowsocks);
    assert_eq!(node.server, "edge.example.com");
    assert_eq!(node.port, 8388);
    assert!(!status.connected);
}

#[test]
fn importing_a_second_link_keeps_both_nodes_and_replaces_the_selection() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();

    let status = service
        .import_node("ss://aes-128-gcm:runtime-secret@edge.example.com:9000#Osaka")
        .unwrap();

    assert_eq!(status.node.as_ref().unwrap().name, "Osaka");
    assert_eq!(status.node.as_ref().unwrap().port, 9000);
    assert_eq!(service.nodes().unwrap().len(), 2);
}

#[test]
fn records_manual_node_latency_without_changing_the_selection() {
    let (mut service, _runtime, _fail_start) = service();
    let tokyo = service.import_node(SHADOWSOCKS_LINK).unwrap().node.unwrap();
    let osaka = service
        .import_node("ss://aes-128-gcm:runtime-secret@edge.example.com:9000#Osaka")
        .unwrap()
        .node
        .unwrap();

    let tested = service
        .record_node_latency(tokyo.id, Some(42), TimestampMillis::new(100))
        .unwrap();

    assert_eq!(tested.latency_ms, Some(42));
    assert_eq!(tested.last_tested_at, Some(100));
    assert_eq!(service.status().node.unwrap().id, osaka.id);
    assert_eq!(service.node(tokyo.id).unwrap(), tested);
}

#[test]
fn records_failed_subscription_node_latency_on_the_selected_node() {
    let (mut service, node_id, _runtime) = service_with_subscription_node();

    let tested = service
        .record_node_latency(node_id, None, TimestampMillis::new(200))
        .unwrap();

    assert_eq!(tested.latency_ms, None);
    assert_eq!(tested.last_tested_at, Some(200));
    assert_eq!(service.status().node.unwrap(), tested);
}

#[test]
fn edits_a_manual_node_while_preserving_its_protocol_and_credential() {
    let (mut service, _runtime, _fail_start) = service();
    let original = service.import_node(SHADOWSOCKS_LINK).unwrap().node.unwrap();

    let status = service
        .edit_node(original.id, " Tokyo 2 ", " new.example.com ", 443)
        .unwrap();

    let edited = status.node.unwrap();
    assert_eq!(edited.id, original.id);
    assert_eq!(edited.name, "Tokyo 2");
    assert_eq!(edited.server, "new.example.com");
    assert_eq!(edited.port, 443);
    assert_eq!(edited.protocol, ProxyProtocol::Shadowsocks);
    assert_eq!(service.node(original.id).unwrap(), edited);
}

#[test]
fn rejects_invalid_read_only_and_connected_node_edits() {
    let (mut service, _runtime, _fail_start) = service();
    let node = service.import_node(SHADOWSOCKS_LINK).unwrap().node.unwrap();

    assert_eq!(
        service
            .edit_node(node.id, " ", "edge.example.com", 443)
            .unwrap_err()
            .code(),
        "invalid_node"
    );
    assert_eq!(service.status().node.as_ref().unwrap().name, "Tokyo Edge");

    service.connect().unwrap();
    assert_eq!(
        service
            .edit_node(node.id, "Tokyo 2", "edge.example.com", 443)
            .unwrap_err()
            .code(),
        "session_active"
    );

    let (mut managed_service, managed_id, _runtime) = service_with_subscription_node();
    assert_eq!(
        managed_service
            .edit_node(managed_id, "Managed", "edge.example.com", 443)
            .unwrap_err()
            .code(),
        "subscription_node_read_only"
    );
}

#[test]
fn rejects_latency_updates_for_a_missing_node() {
    let (mut service, _runtime, _fail_start) = service();

    assert_eq!(
        service
            .record_node_latency(Uuid::nil(), None, TimestampMillis::new(300))
            .unwrap_err()
            .code(),
        "node_not_found"
    );
    assert_eq!(
        service.node(Uuid::nil()).unwrap_err().code(),
        "node_not_found"
    );
}

#[test]
fn selects_and_deletes_nodes_while_disconnected() {
    let (mut service, _runtime, _fail_start) = service();
    let tokyo = service.import_node(SHADOWSOCKS_LINK).unwrap().node.unwrap();
    service
        .import_node("ss://aes-128-gcm:runtime-secret@edge.example.com:9000#Osaka")
        .unwrap();

    let status = service.select_node(tokyo.id).unwrap();
    assert_eq!(status.node.as_ref().unwrap().name, "Tokyo Edge");

    let status = service.delete_node(tokyo.id).unwrap();
    assert!(status.node.is_none());
    let nodes = service.nodes().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].name, "Osaka");
}

#[test]
fn selects_and_connects_a_read_only_subscription_node() {
    let (mut service, subscription_node_id, _runtime) = service_with_subscription_node();

    assert_eq!(
        service.status().node.as_ref().unwrap().id,
        subscription_node_id
    );
    let nodes = service.nodes().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].id, subscription_node_id);
    assert!(!nodes[0].deletable);

    let selected = service.select_node(subscription_node_id).unwrap();
    assert_eq!(selected.node.as_ref().unwrap().id, subscription_node_id);
    assert!(service.connect().unwrap().connected);
    service.disconnect().unwrap();
    assert_eq!(
        service
            .delete_node(subscription_node_id)
            .unwrap_err()
            .code(),
        "subscription_node_read_only"
    );
}

#[test]
fn manual_import_replaces_a_persisted_subscription_selection() {
    let (mut service, subscription_node_id, _runtime) = service_with_subscription_node();
    service.select_node(subscription_node_id).unwrap();

    let imported = service.import_node(SHADOWSOCKS_LINK).unwrap();
    let synced = service.sync_selected_node().unwrap();

    assert_ne!(imported.node.as_ref().unwrap().id, subscription_node_id);
    assert_eq!(synced.node, imported.node);
    assert!(synced.node.unwrap().deletable);
}

#[test]
fn synchronization_drops_a_subscription_node_that_was_disabled() {
    let (mut service, subscription_node_id, runtime) = service_with_subscription_node();
    service.select_node(subscription_node_id).unwrap();
    let external = SqliteSubscriptionStore::open(runtime.path().join("nodes.sqlite")).unwrap();
    let selected_node_id = service.status().node.unwrap().id;
    let actual_subscription_id = external
        .active_nodes()
        .unwrap()
        .into_iter()
        .find(|node| node.id == selected_node_id)
        .unwrap()
        .subscription_id
        .unwrap();
    let mut subscription = external
        .subscription(actual_subscription_id)
        .unwrap()
        .unwrap();
    subscription.enabled = false;
    external
        .update_subscription_settings(&subscription)
        .unwrap();

    let status = service.sync_selected_node().unwrap();

    assert!(status.node.is_none());
    assert_eq!(service.connect().unwrap_err().code(), "no_selected_node");
}

#[test]
fn refuses_to_change_nodes_while_connected() {
    let (mut service, _runtime, _fail_start) = service();
    let node = service.import_node(SHADOWSOCKS_LINK).unwrap().node.unwrap();
    service.connect().unwrap();

    assert_eq!(
        service.select_node(node.id).unwrap_err().code(),
        "session_active"
    );
    assert_eq!(
        service.delete_node(node.id).unwrap_err().code(),
        "session_active"
    );
    assert_eq!(
        service.import_node(SHADOWSOCKS_LINK).unwrap_err().code(),
        "session_active"
    );
}

#[test]
fn changes_the_route_while_disconnected_and_uses_it_for_the_next_connection() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();

    let status = service.set_routing_mode(RoutingMode::Direct).unwrap();

    assert_eq!(status.mode, "direct");
    service.connect().unwrap();
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(service.runtime_config_path().unwrap()).unwrap()).unwrap();
    assert_eq!(config["route"]["final"], "direct");
}

#[test]
fn refuses_to_change_the_route_while_connected() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();
    service.connect().unwrap();

    let error = service.set_routing_mode(RoutingMode::Rule).unwrap_err();

    assert_eq!(error.code(), "session_active");
    assert_eq!(service.status().mode, "global");
}

#[test]
fn changes_route_rules_and_uses_them_for_the_next_connection() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();
    service.set_routing_mode(RoutingMode::Rule).unwrap();
    let settings = RouteSettings {
        rules: vec![RouteRuleSetting {
            kind: RouteRuleKind::DomainSuffix,
            value: "cn".to_owned(),
            outbound: DesktopRouteOutbound::Direct,
            enabled: true,
        }],
        final_outbound: DesktopRouteOutbound::Proxy,
    };

    let status = service.set_route_settings(settings.clone()).unwrap();

    assert_eq!(status.route, settings);
    service.connect().unwrap();
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(service.runtime_config_path().unwrap()).unwrap()).unwrap();
    assert_eq!(config["route"]["rules"][1]["domain_suffix"][0], ".cn");
    assert_eq!(config["route"]["rules"][1]["outbound"], "direct");
    assert_eq!(config["route"]["final"], "proxy");
}

#[test]
fn rejects_invalid_route_settings_without_changing_the_current_settings() {
    let (mut service, _runtime, _fail_start) = service();

    let error = service
        .set_route_settings(RouteSettings {
            rules: vec![RouteRuleSetting {
                kind: RouteRuleKind::Port,
                value: "zero".to_owned(),
                outbound: DesktopRouteOutbound::Proxy,
                enabled: true,
            }],
            ..RouteSettings::default()
        })
        .unwrap_err();

    assert_eq!(error.code(), "invalid_route_settings");
    assert_eq!(service.status().route, RouteSettings::default());
}

#[test]
fn refuses_to_change_route_settings_while_connected() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();
    service.connect().unwrap();

    assert_eq!(
        service
            .set_route_settings(RouteSettings::default())
            .unwrap_err()
            .code(),
        "session_active"
    );
}

#[test]
fn changes_dns_while_disconnected_and_uses_it_for_the_next_connection() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();
    let settings = DnsSettings {
        mode: DnsMode::Doh,
        server: "cloudflare-dns.com".to_owned(),
        port: 443,
        strategy: DesktopDnsStrategy::Ipv4Only,
        fake_ip_enabled: true,
        system_domains: vec!["lan".to_owned()],
        ..DnsSettings::default()
    };

    let status = service.set_dns_settings(settings.clone()).unwrap();

    assert_eq!(status.dns, settings);
    service.connect().unwrap();
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(service.runtime_config_path().unwrap()).unwrap()).unwrap();
    assert_eq!(config["dns"]["servers"][1]["type"], "https");
    assert_eq!(config["dns"]["servers"][1]["server"], "cloudflare-dns.com");
    assert_eq!(config["dns"]["servers"][2]["type"], "fakeip");
    assert_eq!(config["dns"]["rules"][0]["domain_suffix"][0], "lan");
    assert_eq!(config["dns"]["strategy"], "ipv4_only");
}

#[test]
fn refuses_to_change_dns_while_connected() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();
    service.connect().unwrap();

    let error = service
        .set_dns_settings(DnsSettings {
            mode: DnsMode::PlainTcp,
            ..DnsSettings::default()
        })
        .unwrap_err();

    assert_eq!(error.code(), "session_active");
    assert_eq!(service.status().dns, DnsSettings::default());
}

#[test]
fn rejects_invalid_dns_without_changing_the_current_settings() {
    let (mut service, _runtime, _fail_start) = service();

    let error = service
        .set_dns_settings(DnsSettings {
            mode: DnsMode::PlainUdp,
            port: 0,
            ..DnsSettings::default()
        })
        .unwrap_err();

    assert_eq!(error.code(), "invalid_dns_settings");
    assert_eq!(service.status().dns, DnsSettings::default());
}

#[test]
fn rejects_an_unsupported_share_link_without_selecting_a_node() {
    let (mut service, _runtime, _fail_start) = service();

    assert!(matches!(
        service.import_node("tuic://token@edge.example.com:443"),
        Err(SessionCommandError::ShareLink(_))
    ));
    assert!(service.status().node.is_none());
}

#[test]
fn connecting_starts_the_core_and_the_system_proxy() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut service, runtime, _fail_start) = service_with_events(&events);
    service.import_node(SHADOWSOCKS_LINK).unwrap();

    let status = service.connect().unwrap();

    assert!(status.connected);
    assert_eq!(status.node.as_ref().unwrap().name, "Tokyo Edge");
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["core_start", "proxy_enable"]
    );

    let status = service.disconnect().unwrap();

    assert!(!status.connected);
    assert!(status.node.is_some());
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["core_start", "proxy_enable", "proxy_stop", "core_stop"]
    );
    assert!(fs::read_dir(runtime.path()).unwrap().next().is_none());
}

#[test]
fn exposes_a_url_test_target_only_while_connected() {
    let (mut service, _runtime, _fail_start) = service();
    let node = service.import_node(SHADOWSOCKS_LINK).unwrap().node.unwrap();

    assert_eq!(
        service.url_test_target().unwrap_err().code(),
        "session_inactive"
    );

    service.connect().unwrap();
    let target = service.url_test_target().unwrap();

    assert_eq!(target.node_id, node.id);
    assert_eq!(target.http_port, 10_809);
}

#[test]
fn exposes_the_loopback_traffic_api_only_while_connected() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();

    assert_eq!(
        service.traffic_api_address().unwrap_err().code(),
        "session_inactive"
    );

    service.connect().unwrap();

    assert_eq!(
        service.traffic_api_address().unwrap(),
        "127.0.0.1:9090".parse().unwrap()
    );
}

#[test]
fn connecting_without_an_imported_node_fails_before_touching_the_core() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut service, _runtime, _fail_start) = service_with_events(&events);

    assert!(matches!(
        service.connect(),
        Err(SessionCommandError::NoSelectedNode)
    ));
    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn surfaces_a_failing_core_start_as_a_session_error() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let runtime = RuntimeDirectory::new("core-start-failure");
    let core = FakeCore::new(events.clone(), Arc::new(AtomicBool::new(true)));
    let mut service = SessionService::new(
        DesktopSession::new(
            MemorySecretStore::default(),
            core,
            FakeProxy::new(events.clone()),
            runtime.path(),
        ),
        SessionDefaults::v01(),
        SqliteManualNodeStore::open_in_memory().unwrap(),
        SqliteSubscriptionStore::open_in_memory().unwrap(),
        SqliteRoutingModeStore::open_in_memory().unwrap(),
        SqliteRouteSettingsStore::open_in_memory().unwrap(),
        SqliteDnsSettingsStore::open_in_memory().unwrap(),
    )
    .unwrap();
    service.import_node(SHADOWSOCKS_LINK).unwrap();

    assert!(matches!(
        service.connect(),
        Err(SessionCommandError::Session(_))
    ));
    assert!(!service.status().connected);
    assert_eq!(events.lock().unwrap().as_slice(), ["core_start"]);
}

#[test]
fn disconnecting_an_idle_session_reports_a_session_error() {
    let (mut service, _runtime, _fail_start) = service();

    assert!(matches!(
        service.disconnect(),
        Err(SessionCommandError::Session(_))
    ));
}

type TestService = SessionService<MemorySecretStore, FakeCore, FakeProxy>;

fn service() -> (TestService, RuntimeDirectory, Arc<AtomicBool>) {
    service_with_events(&Arc::new(Mutex::new(Vec::new())))
}

fn service_with_events(
    events: &Arc<Mutex<Vec<&'static str>>>,
) -> (TestService, RuntimeDirectory, Arc<AtomicBool>) {
    let runtime = RuntimeDirectory::new("session-service");
    let fail_start = Arc::new(AtomicBool::new(false));
    let service = SessionService::new(
        DesktopSession::new(
            MemorySecretStore::default(),
            FakeCore::new(events.clone(), fail_start.clone()),
            FakeProxy::new(events.clone()),
            runtime.path(),
        ),
        SessionDefaults::v01(),
        SqliteManualNodeStore::open_in_memory().unwrap(),
        SqliteSubscriptionStore::open_in_memory().unwrap(),
        SqliteRoutingModeStore::open_in_memory().unwrap(),
        SqliteRouteSettingsStore::open_in_memory().unwrap(),
        SqliteDnsSettingsStore::open_in_memory().unwrap(),
    )
    .unwrap();
    (service, runtime, fail_start)
}

fn service_with_subscription_node() -> (TestService, Uuid, RuntimeDirectory) {
    let runtime = RuntimeDirectory::new("subscription-node");
    let events = Arc::new(Mutex::new(Vec::new()));
    let secret_store = MemorySecretStore::default();
    let subscription_id = Uuid::parse_str("018f78b5-2cd0-7000-a9a6-3bccf60951e8").unwrap();
    let subscription = Subscription::new(
        subscription_id,
        "Primary",
        CredentialRef::new("subscription/primary/url").unwrap(),
        60,
    )
    .unwrap();
    let (node, credential) = SubscriptionContentParser
        .parse(SHADOWSOCKS_LINK.as_bytes(), subscription_id)
        .unwrap()
        .pop()
        .unwrap()
        .into_parts();
    secret_store
        .put(
            &node.credential_ref,
            &CredentialCodec::encode(&credential).unwrap(),
        )
        .unwrap();
    let node_id = node.id;
    let database = runtime.path().join("nodes.sqlite");
    let mut subscriptions = SqliteSubscriptionStore::open(&database).unwrap();
    subscriptions.insert_subscription(&subscription).unwrap();
    subscriptions
        .apply_update(
            &SubscriptionUpdate::new(
                subscription_id,
                vec![node],
                SubscriptionValidators::default(),
                TimestampMillis::new(100),
            )
            .unwrap(),
        )
        .unwrap();
    subscriptions.select_node(node_id).unwrap();
    let service = SessionService::new(
        DesktopSession::new(
            secret_store,
            FakeCore::new(events.clone(), Arc::new(AtomicBool::new(false))),
            FakeProxy::new(events),
            runtime.path(),
        ),
        SessionDefaults::v01(),
        SqliteManualNodeStore::open(&database).unwrap(),
        subscriptions,
        SqliteRoutingModeStore::open(&database).unwrap(),
        SqliteRouteSettingsStore::open(&database).unwrap(),
        SqliteDnsSettingsStore::open(&database).unwrap(),
    )
    .unwrap();
    (service, node_id, runtime)
}

#[test]
fn a_network_event_reconnects_a_dead_core_without_the_user_acting() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut service, _runtime, _fail_start) = service_with_events(&events);
    service.import_node(SHADOWSOCKS_LINK).unwrap();
    service.connect().unwrap();
    events.lock().unwrap().clear();

    let now = Instant::now();
    assert_eq!(service.recovery_due_at(), None);
    service.observe_network(NetworkEvent::Woke, now);
    let due_at = service.recovery_due_at().expect("an event is pending");

    let outcome = service.recover(due_at, &AlwaysUnhealthy).unwrap();

    assert_eq!(outcome, RecoveryOutcome::Reconnected { attempts: 1 });
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["proxy_stop", "core_stop", "core_start", "proxy_enable"]
    );
    assert!(service.status().connected);
}

#[test]
fn a_network_event_leaves_a_healthy_core_alone() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut service, _runtime, _fail_start) = service_with_events(&events);
    service.import_node(SHADOWSOCKS_LINK).unwrap();
    service.connect().unwrap();
    events.lock().unwrap().clear();

    let now = Instant::now();
    service.observe_network(NetworkEvent::PathChanged, now);
    let due_at = service.recovery_due_at().unwrap();

    assert_eq!(
        service.recover(due_at, &AlwaysHealthy).unwrap(),
        RecoveryOutcome::Healthy
    );
    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn health_monitor_reconnects_a_dead_core_without_a_network_event() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut service, _runtime, _fail_start) = service_with_events(&events);
    service.import_node(SHADOWSOCKS_LINK).unwrap();
    service.connect().unwrap();
    events.lock().unwrap().clear();

    let outcome = service
        .monitor_recovery(Instant::now(), &AlwaysUnhealthy)
        .unwrap();

    assert_eq!(outcome, RecoveryOutcome::Reconnected { attempts: 1 });
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["proxy_stop", "core_stop", "core_start", "proxy_enable"]
    );
    assert!(service.status().connected);
}

#[test]
fn every_session_failure_carries_a_stable_code_for_the_ui() {
    let (mut service, _runtime, _fail_start) = service();

    assert_eq!(service.connect().unwrap_err().code(), "no_selected_node");
    assert_eq!(
        service
            .import_node("tuic://token@edge.example.com")
            .unwrap_err()
            .code(),
        "invalid_share_link"
    );
    assert_eq!(service.disconnect().unwrap_err().code(), "session_failed");
    assert_eq!(
        service.select_node(Uuid::nil()).unwrap_err().code(),
        "node_not_found"
    );
}

#[test]
fn v01_defaults_use_the_documented_loopback_ports() {
    let defaults = SessionDefaults::v01();

    assert_eq!(defaults.socks, LocalSocksProfile::default());
    assert_eq!(defaults.http, LocalHttpProfile::default());
    assert_eq!(defaults.clash_api_port.get(), 9_090);
    assert!(defaults.system_proxy);
}

static RUNTIME_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct RuntimeDirectory(PathBuf);

impl RuntimeDirectory {
    fn new(name: &str) -> Self {
        let sequence = RUNTIME_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mgclash-desktop-{name}-{}-{sequence}", id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for RuntimeDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!(
                "failed to remove runtime directory {}: {error}",
                self.0.display()
            );
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FakeError(&'static str);

impl Display for FakeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for FakeError {}

struct FakeCore {
    events: Arc<Mutex<Vec<&'static str>>>,
    fail_start: Arc<AtomicBool>,
}

impl FakeCore {
    fn new(events: Arc<Mutex<Vec<&'static str>>>, fail_start: Arc<AtomicBool>) -> Self {
        Self { events, fail_start }
    }
}

struct AlwaysHealthy;

impl SessionHealthProbe for AlwaysHealthy {
    fn is_healthy(&self) -> bool {
        true
    }
}

struct AlwaysUnhealthy;

impl SessionHealthProbe for AlwaysUnhealthy {
    fn is_healthy(&self) -> bool {
        false
    }
}

impl CoreSessionControl for FakeCore {
    type Error = FakeError;
    type Output = ();

    fn start(&mut self, config_path: &Path) -> Result<Self::Output, Self::Error> {
        self.events.lock().unwrap().push("core_start");
        let config = fs::read_to_string(config_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&config).unwrap();
        if parsed["route"]["final"] == "direct" {
            assert!(!config.contains("runtime-secret"));
        } else {
            assert!(config.contains("runtime-secret"));
        }
        assert!(config.contains("127.0.0.1:9090"));
        if self.fail_start.load(Ordering::Relaxed) {
            Err(FakeError("core start failed"))
        } else {
            Ok(())
        }
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push("core_stop");
        Ok(())
    }
}

struct FakeProxy {
    events: Arc<Mutex<Vec<&'static str>>>,
}

impl FakeProxy {
    fn new(events: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self { events }
    }
}

impl SystemProxySessionControl for FakeProxy {
    type Error = FakeError;

    fn enable(&mut self, _state: &SystemProxyState) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push("proxy_enable");
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.events.lock().unwrap().push("proxy_stop");
        Ok(())
    }
}
