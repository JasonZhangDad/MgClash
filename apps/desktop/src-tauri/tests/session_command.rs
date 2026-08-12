use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::net::{Ipv4Addr, TcpListener};
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
use magies_desktop_lib::session::{
    NodeMoveDirection, NodeStores, SessionCommandError, SessionDefaults, SessionService,
};
use magies_domain::{CoreType, CredentialRef, ProxyProtocol, Subscription, TimestampMillis};
use magies_platform::system_proxy::SystemProxyState;
use magies_profiles::{
    CorePreference, CredentialCodec, LocalHttpProfile, LocalSocksProfile, ManualCredentialDraft,
    ManualNodeDraft, ManualNodeStoreError, ShareLinkParser, SqliteManualNodeStore,
    SqliteNodeGroupStore, SqliteNodeOrderStore, SqliteSubscriptionStore, SubscriptionContentParser,
    SubscriptionUpdate, SubscriptionValidators,
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
    // The documented defaults are asserted by
    // `v01_defaults_use_the_documented_loopback_ports`; here the fixture picks
    // free ports, so this only checks the status reports what it was given.
    assert_eq!(status.socks_port, service.defaults().socks.port().get());
    assert_eq!(status.http_port, service.defaults().http.port().get());
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

const OSAKA_LINK: &str = "ss://aes-128-gcm:runtime-secret@osaka.example.com:9000#Osaka";

#[test]
fn importing_a_list_stores_every_node_and_selects_the_first() {
    let (mut service, _runtime, _fail_start) = service();

    let report = service
        .import_nodes(format!("{SHADOWSOCKS_LINK}\n{OSAKA_LINK}").as_bytes())
        .unwrap();

    assert_eq!(report.imported, 2);
    assert_eq!(report.duplicates, 0);
    assert!(report.failures.is_empty());
    assert_eq!(service.nodes().unwrap().len(), 2);
    // Nothing was selected before, so the first import fills the gap.
    assert_eq!(report.status.node.as_ref().unwrap().name, "Tokyo Edge");
    assert!(service.connect().unwrap().connected);
}

#[test]
fn importing_a_list_keeps_an_existing_selection() {
    let (mut service, _runtime, _fail_start) = service();
    let chosen = service.import_node(SHADOWSOCKS_LINK).unwrap().node.unwrap();

    let report = service
        .import_nodes(format!("{OSAKA_LINK}\ntrojan://hunter2@fra.example.com:443#Fra").as_bytes())
        .unwrap();

    assert_eq!(report.imported, 2);
    // Pasting a list must not move the user off the node they chose.
    assert_eq!(report.status.node.as_ref().unwrap().id, chosen.id);
    assert_eq!(service.nodes().unwrap().len(), 3);
}

#[test]
fn importing_a_list_reports_bad_lines_and_keeps_the_good_ones() {
    let (mut service, _runtime, _fail_start) = service();

    let report = service
        .import_nodes(format!("{SHADOWSOCKS_LINK}\nnot a link\n{OSAKA_LINK}").as_bytes())
        .unwrap();

    assert_eq!(report.imported, 2);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].line, Some(2));
    assert!(!report.failures[0].message.is_empty());
    assert_eq!(service.nodes().unwrap().len(), 2);
}

#[test]
fn importing_a_list_drops_repeats_inside_the_body() {
    let (mut service, _runtime, _fail_start) = service();

    let report = service
        .import_nodes(format!("{SHADOWSOCKS_LINK}\n{OSAKA_LINK}\n{SHADOWSOCKS_LINK}").as_bytes())
        .unwrap();

    assert_eq!(report.imported, 2);
    assert_eq!(report.duplicates, 1);
    assert_eq!(service.nodes().unwrap().len(), 2);
}

#[test]
fn importing_the_same_list_twice_adds_nothing_the_second_time() {
    let (mut service, _runtime, _fail_start) = service();
    let body = format!("{SHADOWSOCKS_LINK}\n{OSAKA_LINK}");
    service.import_nodes(body.as_bytes()).unwrap();

    let report = service.import_nodes(body.as_bytes()).unwrap();

    assert_eq!(report.imported, 0);
    assert_eq!(report.duplicates, 2);
    // Re-importing a subscription export is a normal thing to do; it must not
    // double the list.
    assert_eq!(service.nodes().unwrap().len(), 2);
}

#[test]
fn a_second_import_keeps_only_the_nodes_that_are_new() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_nodes(SHADOWSOCKS_LINK.as_bytes()).unwrap();

    let report = service
        .import_nodes(format!("{SHADOWSOCKS_LINK}\n{OSAKA_LINK}").as_bytes())
        .unwrap();

    assert_eq!(report.imported, 1);
    assert_eq!(report.duplicates, 1);
    let names: Vec<_> = service
        .nodes()
        .unwrap()
        .into_iter()
        .map(|node| node.name)
        .collect();
    assert_eq!(names, ["Tokyo Edge", "Osaka"]);
}

#[test]
fn a_node_differing_only_by_credential_is_not_a_repeat() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_nodes(SHADOWSOCKS_LINK.as_bytes()).unwrap();

    // Same endpoint, different password: a second account, not a duplicate.
    let report = service
        .import_nodes(b"ss://aes-128-gcm:other-secret@edge.example.com:8388#Tokyo%20Edge")
        .unwrap();

    assert_eq!(report.imported, 1);
    assert_eq!(report.duplicates, 0);
    assert_eq!(service.nodes().unwrap().len(), 2);
}

#[test]
fn a_manually_created_node_also_blocks_a_repeat_import() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();

    // The single-link path stores the same way, so the fingerprint matches.
    let report = service.import_nodes(SHADOWSOCKS_LINK.as_bytes()).unwrap();

    assert_eq!(report.imported, 0);
    assert_eq!(report.duplicates, 1);
    assert_eq!(service.nodes().unwrap().len(), 1);
}

#[test]
fn importing_an_unreadable_list_is_a_typed_error() {
    let (mut service, _runtime, _fail_start) = service();

    assert_eq!(
        service.import_nodes(b"   ").unwrap_err().code(),
        "invalid_node_list"
    );
    assert_eq!(
        service
            .import_nodes(b"!!!not base64!!!")
            .unwrap_err()
            .code(),
        "invalid_node_list"
    );
    assert!(service.nodes().unwrap().is_empty());
}

#[test]
fn refuses_to_import_a_list_while_connected() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();
    service.connect().unwrap();

    assert_eq!(
        service
            .import_nodes(OSAKA_LINK.as_bytes())
            .unwrap_err()
            .code(),
        "session_active"
    );
}

#[test]
fn enabling_tun_replaces_system_proxy_rather_than_adding_to_it() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();

    service.set_tun_enabled(true);

    // DesktopSession refuses both at once, so a session that still asked for
    // System Proxy would fail with ConflictingNetworkModes instead of starting.
    match service.connect() {
        Ok(status) => assert!(status.connected),
        Err(error) => assert_eq!(
            error.code(),
            "tun_unavailable",
            "TUN should either start or be refused for this platform, got {error}"
        ),
    }
}

#[test]
fn tun_makes_the_matrix_rule_out_xray() {
    let (mut service, _runtime, _fail_start) = service();
    service
        .create_node(trojan_draft("Frankfurt", 8443))
        .unwrap();
    service.set_core_preference(CorePreference::Fixed(CoreType::Xray));
    assert_eq!(service.selected_core(), Ok(CoreType::Xray));

    service.set_tun_enabled(true);

    // Xray has no TUN inbound, so the same node now has no usable Core.
    let error = service.selected_core().unwrap_err();
    assert_eq!(
        error.to_string(),
        "the selected Core cannot run this node: xray cannot provide TUN mode"
    );
}

#[test]
fn turning_tun_off_restores_the_previous_core_choice() {
    let (mut service, _runtime, _fail_start) = service();
    service
        .create_node(trojan_draft("Frankfurt", 8443))
        .unwrap();
    service.set_core_preference(CorePreference::Fixed(CoreType::Xray));
    service.set_tun_enabled(true);
    assert!(service.selected_core().is_err());

    service.set_tun_enabled(false);

    assert_eq!(service.selected_core(), Ok(CoreType::Xray));
}

#[test]
fn the_node_summary_carries_its_transport_and_tls() {
    let (mut service, _runtime, _fail_start) = service();
    // A share link with no TLS parameters: plaintext over plain TCP.
    service.import_node(SHADOWSOCKS_LINK).unwrap();

    let node = &service.nodes().unwrap()[0];

    assert_eq!(node.transport, "tcp");
    assert_eq!(node.tls, None);
}

#[test]
fn a_hysteria2_node_reports_its_own_quic_transport() {
    let (mut service, _runtime, _fail_start) = service();
    service
        .import_node("hysteria2://secret@edge.example.com:8443#Tokyo")
        .unwrap();

    let node = &service.nodes().unwrap()[0];

    // The model stores no transport for Hysteria2 because it carries its own.
    assert_eq!(node.transport, "quic");
    assert_eq!(node.tls, Some("tls"));
}

#[test]
fn the_status_reports_sing_box_by_default() {
    let (mut service, _runtime, _fail_start) = service();
    assert_eq!(service.status().core, "sing-box");

    service.import_node(SHADOWSOCKS_LINK).unwrap();

    assert_eq!(service.status().core, "sing-box");
    assert_eq!(service.selected_core(), Ok(CoreType::SingBox));
}

#[test]
fn choosing_xray_changes_the_reported_core() {
    let (mut service, _runtime, _fail_start) = service();
    service
        .create_node(trojan_draft("Frankfurt", 8443))
        .unwrap();

    service.set_core_preference(CorePreference::Fixed(CoreType::Xray));

    assert_eq!(service.selected_core(), Ok(CoreType::Xray));
    assert_eq!(service.status().core, "xray");
}

#[test]
fn choosing_xray_for_a_hysteria2_node_reports_why_it_cannot_run() {
    let (mut service, _runtime, _fail_start) = service();
    service
        .import_node("hysteria2://secret@edge.example.com:8443#Tokyo")
        .unwrap();

    service.set_core_preference(CorePreference::Fixed(CoreType::Xray));

    let error = service.selected_core().unwrap_err();
    assert_eq!(
        error.to_string(),
        "the selected Core cannot run this node: xray does not support Hysteria2"
    );
}

#[test]
fn auto_keeps_sing_box_for_a_hysteria2_node() {
    let (mut service, _runtime, _fail_start) = service();
    service
        .import_node("hysteria2://secret@edge.example.com:8443#Tokyo")
        .unwrap();

    service.set_core_preference(CorePreference::Auto);

    assert_eq!(service.selected_core(), Ok(CoreType::SingBox));
}

#[test]
fn connecting_with_an_impossible_core_choice_reports_it_instead_of_falling_back() {
    let (mut service, _runtime, _fail_start) = service();
    service
        .import_node("hysteria2://secret@edge.example.com:8443#Tokyo")
        .unwrap();
    service.set_core_preference(CorePreference::Fixed(CoreType::Xray));

    let error = service.connect().unwrap_err();

    // Quietly running sing-box would leave the user believing they were on Xray.
    assert_eq!(error.code(), "core_unavailable");
    assert!(!service.status().connected);
}

#[test]
fn connecting_with_a_workable_core_choice_succeeds() {
    let (mut service, _runtime, _fail_start) = service();
    service
        .create_node(trojan_draft("Frankfurt", 8443))
        .unwrap();
    service.set_core_preference(CorePreference::Fixed(CoreType::Xray));

    assert!(service.connect().unwrap().connected);
    assert_eq!(service.status().core, "xray");
}

#[test]
fn creating_a_manual_node_stores_the_credential_and_selects_it() {
    let (mut service, _runtime, _fail_start) = service();

    let status = service
        .create_node(trojan_draft("Frankfurt", 8443))
        .unwrap();

    let node = status.node.as_ref().unwrap();
    assert_eq!(node.name, "Frankfurt");
    assert_eq!(node.protocol, ProxyProtocol::Trojan);
    assert_eq!(node.server, "edge.example.com");
    assert_eq!(node.port, 8443);
    assert!(!status.connected);

    // Connecting reads the credential back out of the secret store and feeds it
    // to the config generator, so a successful start proves it round-tripped.
    assert!(service.connect().unwrap().connected);
}

#[test]
fn a_manually_created_node_joins_the_imported_ones() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();

    service
        .create_node(trojan_draft("Frankfurt", 8443))
        .unwrap();

    let nodes = service.nodes().unwrap();
    assert_eq!(nodes.len(), 2);
    assert!(nodes.iter().any(|node| node.name == "Frankfurt"));
}

#[test]
fn creating_a_manual_node_rejects_invalid_fields() {
    let (mut service, _runtime, _fail_start) = service();

    let mut blank_name = trojan_draft("Frankfurt", 8443);
    blank_name.name = "   ".to_owned();
    assert_eq!(
        service.create_node(blank_name).unwrap_err().code(),
        "invalid_manual_node"
    );

    let mut bad_port = trojan_draft("Frankfurt", 8443);
    bad_port.port = 0;
    assert_eq!(
        service.create_node(bad_port).unwrap_err().code(),
        "invalid_manual_node"
    );

    let mut blank_password = trojan_draft("Frankfurt", 8443);
    blank_password.credential = ManualCredentialDraft::Trojan {
        password: String::new(),
    };
    assert_eq!(
        service.create_node(blank_password).unwrap_err().code(),
        "invalid_manual_node"
    );

    // A rejected draft must not leave a half-written node behind.
    assert!(service.nodes().unwrap().is_empty());
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
fn reorders_manual_and_subscription_nodes_together() {
    let (mut service, managed_id, _runtime) = service_with_subscription_node();
    let manual_id = service
        .import_node(SHADOWSOCKS_LINK)
        .unwrap()
        .node
        .unwrap()
        .id;
    assert_eq!(
        service
            .nodes()
            .unwrap()
            .into_iter()
            .map(|node| node.id)
            .collect::<Vec<_>>(),
        vec![manual_id, managed_id]
    );

    let moved = service
        .move_node(managed_id, NodeMoveDirection::Up)
        .unwrap();

    assert_eq!(
        moved.into_iter().map(|node| node.id).collect::<Vec<_>>(),
        vec![managed_id, manual_id]
    );
    assert_eq!(
        service
            .move_node(managed_id, NodeMoveDirection::Up)
            .unwrap()
            .into_iter()
            .map(|node| node.id)
            .collect::<Vec<_>>(),
        vec![managed_id, manual_id]
    );
}

#[test]
fn groups_manual_and_subscription_nodes_with_shared_names() {
    let (mut service, managed_id, _runtime) = service_with_subscription_node();
    let manual_id = service
        .import_node(SHADOWSOCKS_LINK)
        .unwrap()
        .node
        .unwrap()
        .id;

    let nodes = service.set_node_group(manual_id, Some(" Work ")).unwrap();
    let group_id = nodes
        .iter()
        .find(|node| node.id == manual_id)
        .unwrap()
        .group_id
        .unwrap();
    assert_eq!(service.status().node.unwrap().group_id, Some(group_id));
    assert_eq!(service.node_groups().unwrap()[0].name, "Work");

    let nodes = service.set_node_group(managed_id, Some("Work")).unwrap();
    assert!(nodes.iter().all(|node| node.group_id == Some(group_id)));

    let nodes = service.set_node_group(manual_id, None).unwrap();
    assert_eq!(
        nodes
            .iter()
            .find(|node| node.id == manual_id)
            .unwrap()
            .group_id,
        None
    );
    let missing = Uuid::from_u128(999);
    assert!(matches!(
        service.set_node_group(missing, Some("Missing")),
        Err(SessionCommandError::NodeStore(ManualNodeStoreError::NodeNotFound { id }))
            if id == missing
    ));
    assert_eq!(service.node_groups().unwrap().len(), 1);
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
    assert_eq!(
        service
            .create_node(trojan_draft("Frankfurt", 8443))
            .unwrap_err()
            .code(),
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
    assert_eq!(target.http_port, service.defaults().http.port().get());
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
        defaults_on_free_ports(),
        NodeStores::new(
            SqliteManualNodeStore::open_in_memory().unwrap(),
            SqliteSubscriptionStore::open_in_memory().unwrap(),
            SqliteNodeOrderStore::open_in_memory().unwrap(),
            SqliteNodeGroupStore::open_in_memory().unwrap(),
        ),
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

fn trojan_draft(name: &str, port: u32) -> ManualNodeDraft {
    ManualNodeDraft {
        name: name.to_owned(),
        server: "edge.example.com".to_owned(),
        port,
        udp_enabled: true,
        transport: None,
        tls: None,
        credential: ManualCredentialDraft::Trojan {
            password: "runtime-secret".to_owned(),
        },
    }
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
        defaults_on_free_ports(),
        NodeStores::new(
            SqliteManualNodeStore::open_in_memory().unwrap(),
            SqliteSubscriptionStore::open_in_memory().unwrap(),
            SqliteNodeOrderStore::open_in_memory().unwrap(),
            SqliteNodeGroupStore::open_in_memory().unwrap(),
        ),
        SqliteRoutingModeStore::open_in_memory().unwrap(),
        SqliteRouteSettingsStore::open_in_memory().unwrap(),
        SqliteDnsSettingsStore::open_in_memory().unwrap(),
    )
    .unwrap();
    (service, runtime, fail_start)
}

/// The V0.1 defaults with the loopback ports moved out of the way.
///
/// `connect` binds the SOCKS and HTTP ports to prove they are free, so a suite
/// pinned to 10808/10809 would fail on any machine already running a proxy
/// client. The ports are still real, just chosen by the OS.
fn defaults_on_free_ports() -> SessionDefaults {
    let (socks, http) = free_ports();
    SessionDefaults {
        socks: LocalSocksProfile::new(u32::from(socks)).unwrap(),
        http: LocalHttpProfile::new(u32::from(http)).unwrap(),
        ..SessionDefaults::v01()
    }
}

/// Two distinct loopback ports the OS reports as free.
///
/// Both listeners are held until both ports are known. Allocating them one at a
/// time returns the same port twice on Linux, whose ephemeral allocator hands
/// back a port as soon as it is released — which the preflight then rejects as a
/// duplicate.
fn free_ports() -> (u16, u16) {
    let bind = || {
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("the OS must hand out a loopback port")
    };
    let first = bind();
    let second = bind();
    let ports = (
        first
            .local_addr()
            .expect("a bound listener has an address")
            .port(),
        second
            .local_addr()
            .expect("a bound listener has an address")
            .port(),
    );
    assert_ne!(ports.0, ports.1, "the OS handed out one port twice");
    ports
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
        defaults_on_free_ports(),
        NodeStores::new(
            SqliteManualNodeStore::open(&database).unwrap(),
            subscriptions,
            SqliteNodeOrderStore::open(&database).unwrap(),
            SqliteNodeGroupStore::open(&database).unwrap(),
        ),
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
        // sing-box names the section `route` and Xray names it `routing`, so
        // the direct-only case is detected per Core.
        let direct_only = parsed["route"]["final"] == "direct"
            || parsed["routing"]["rules"].as_array().is_some_and(|rules| {
                rules
                    .iter()
                    .all(|rule| rule["outboundTag"].as_str() == Some("direct"))
            });
        if direct_only {
            assert!(!config.contains("runtime-secret"));
        } else {
            assert!(config.contains("runtime-secret"));
        }
        // The API port has to be reachable either way, but each Core spells it
        // differently: sing-box as one address, Xray as a listener plus a port.
        let api_exposed = parsed["experimental"]["clash_api"]["external_controller"]
            == "127.0.0.1:9090"
            || parsed["inbounds"].as_array().is_some_and(|inbounds| {
                inbounds
                    .iter()
                    .any(|inbound| inbound["tag"] == "api-in" && inbound["port"] == 9_090)
            });
        assert!(api_exposed, "the Clash/stats API port is missing: {config}");
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

#[test]
fn a_pinned_hysteria2_node_imports_but_refuses_to_connect() {
    let (mut service, _runtime, _fail_start) = service();
    service
        .import_node(
            "hysteria2://password@edge.example.com:5555?sni=edge.example.com\
             &pinSHA256=6ff212bbab490b686b06209c6074865f9340f4c0f9c4aa7d34d568c2a2cebe73#Pinned",
        )
        .unwrap();
    let id = service.nodes().unwrap()[0].id;
    service.select_node(id).unwrap();

    // The table has to show the pin: it is why this node behaves differently.
    assert_eq!(service.nodes().unwrap()[0].tls, Some("tls+pin"));
    // sing-box has Hysteria2 but no digest verification; Xray has the digest
    // but no Hysteria2. The refusal names the pin rather than the protocol.
    let error = service.connect().unwrap_err();
    let message = format!("{}", ErrorChain(&error));
    assert!(
        message.contains("pinned certificate"),
        "the refusal must name the pin: {message}"
    );
}

/// Renders an error with its source chain, the way the Tauri layer reports it.
struct ErrorChain<'a>(&'a dyn std::error::Error);

impl std::fmt::Display for ErrorChain<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)?;
        let mut source = self.0.source();
        while let Some(error) = source {
            write!(formatter, ": {error}")?;
            source = error.source();
        }
        Ok(())
    }
}

#[test]
fn connecting_names_the_busy_port_instead_of_a_dead_core() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();
    let id = service.nodes().unwrap()[0].id;
    service.select_node(id).unwrap();
    // Another proxy client holding the SOCKS port is the ordinary case: v2rayN
    // and MgClash both default to 10808.
    let _busy = TcpListener::bind((Ipv4Addr::LOCALHOST, service.defaults().socks.port().get()))
        .expect("the fixture port was free a moment ago");

    let error = service.connect().unwrap_err();

    // Without the preflight the Core exits on its own and the user is told only
    // "Core exited before becoming healthy", which names nothing actionable.
    let message = format!("{error}: {}", source_chain(&error));
    assert!(
        message.contains(&service.defaults().socks.port().to_string()),
        "the refusal must name the busy port: {message}"
    );
}

fn source_chain(error: &dyn std::error::Error) -> String {
    let mut parts = Vec::new();
    let mut source = error.source();
    while let Some(error) = source {
        parts.push(error.to_string());
        source = error.source();
    }
    parts.join(": ")
}

#[test]
fn a_stored_node_is_exported_as_a_link_that_parses_back() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();
    let id = service.nodes().unwrap()[0].id;

    let link = service.export_node_link(id).unwrap();

    // Importing the exported link has to produce the same node, which is the
    // only thing exporting is for.
    let reimported = ShareLinkParser
        .parse(
            &link,
            Uuid::new_v4(),
            CredentialRef::new("keychain://nodes/roundtrip").unwrap(),
        )
        .unwrap();
    let original = &service.nodes().unwrap()[0];
    assert_eq!(reimported.node().protocol_type, original.protocol);
    assert_eq!(reimported.node().server.as_str(), original.server);
    assert_eq!(reimported.node().port.get(), original.port);
    assert_eq!(reimported.node().name.as_str(), original.name);
}

#[test]
fn exporting_an_unknown_node_is_a_typed_not_found() {
    let (service, _runtime, _fail_start) = service();

    assert!(matches!(
        service.export_node_link(Uuid::new_v4()),
        Err(SessionCommandError::NodeStore(
            ManualNodeStoreError::NodeNotFound { .. }
        ))
    ));
}

#[test]
fn cloning_a_node_copies_its_credential_and_leaves_the_selection_alone() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();
    let original = service.nodes().unwrap()[0].id;
    service.select_node(original).unwrap();

    let nodes = service.clone_node(original).unwrap();

    assert_eq!(nodes.len(), 2);
    // Cloning is for "duplicate, then edit"; it must not move the user off the
    // node they are on.
    assert_eq!(service.status().node.unwrap().id, original);
    let clone = nodes.iter().find(|node| node.id != original).unwrap();
    assert_eq!(clone.server, nodes[0].server);
    assert_eq!(clone.port, nodes[0].port);
    // The copy has to be usable on its own, which means its own stored secret.
    let link = service.export_node_link(clone.id).unwrap();
    assert_eq!(link, service.export_node_link(original).unwrap());
}

#[test]
fn cloning_a_subscription_node_is_refused() {
    let (mut service, node_id, _runtime) = service_with_subscription_node();

    // The subscription owns its nodes; a copy would survive a refresh that
    // removes the original, which is not what "clone" implies here.
    assert!(matches!(
        service.clone_node(node_id),
        Err(SessionCommandError::NodeStore(
            ManualNodeStoreError::NodeNotFound { .. }
        ))
    ));
}

#[test]
fn removing_duplicates_keeps_the_first_of_each_repeated_node() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();
    let original = service.nodes().unwrap()[0].id;
    service.clone_node(original).unwrap();
    service.clone_node(original).unwrap();
    service
        .import_node("trojan://hunter2@edge.example.com:443#Trojan")
        .unwrap();

    let removed = service.remove_duplicate_nodes().unwrap();

    assert_eq!(removed, 2);
    let remaining = service.nodes().unwrap();
    assert_eq!(remaining.len(), 2);
    // The first occurrence survives, so the node the user already selected or
    // ordered does not move.
    assert!(remaining.iter().any(|node| node.id == original));
}

#[test]
fn removing_duplicates_leaves_a_list_without_repeats_untouched() {
    let (mut service, _runtime, _fail_start) = service();
    service.import_node(SHADOWSOCKS_LINK).unwrap();
    service
        .import_node("trojan://hunter2@edge.example.com:443#Trojan")
        .unwrap();

    assert_eq!(service.remove_duplicate_nodes().unwrap(), 0);
    assert_eq!(service.nodes().unwrap().len(), 2);
}
