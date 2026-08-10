use magies_domain::{CredentialRef, ProxyNode, ProxyProtocol, TransportConfig};
use magies_profiles::{
    DiagnosticRedactor, DnsProfile, DnsServer, DnsStrategy, REDACTED, ShadowsocksParser,
    SingBoxRuntimeConfigGenerator, SingBoxRuntimeProfile,
};
use magies_routing::{RouteOutbound, RouteProfile, RoutingMode};
use serde_json::json;
use uuid::Uuid;

#[test]
fn masks_every_credential_bearing_key_at_any_depth() {
    let redacted = DiagnosticRedactor::new().redact_json(&json!({
        "outbounds": [{
            "type": "vless",
            "uuid": "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc",
            "server": "edge.example.com",
            "tls": {
                "reality": { "public_key": "pk", "short_id": "0123abcd" }
            }
        }, {
            "type": "hysteria2",
            "password": "hy2-secret",
            "obfs": { "type": "salamander", "password": "obfs-secret" }
        }]
    }));

    assert_eq!(redacted["outbounds"][0]["uuid"], REDACTED);
    assert_eq!(
        redacted["outbounds"][0]["tls"]["reality"]["short_id"],
        REDACTED
    );
    assert_eq!(redacted["outbounds"][1]["password"], REDACTED);
    assert_eq!(redacted["outbounds"][1]["obfs"]["password"], REDACTED);
}

#[test]
fn keeps_the_fields_that_make_a_bundle_useful() {
    let redacted = DiagnosticRedactor::new().redact_json(&json!({
        "type": "vless",
        "server": "edge.example.com",
        "server_port": 443,
        "tls": { "enabled": true, "server_name": "www.example.com" },
        "uuid": "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc"
    }));

    assert_eq!(redacted["type"], "vless");
    assert_eq!(redacted["server"], "edge.example.com");
    assert_eq!(redacted["server_port"], 443);
    assert_eq!(redacted["tls"]["enabled"], true);
    assert_eq!(redacted["tls"]["server_name"], "www.example.com");
    assert_eq!(redacted["uuid"], REDACTED);
}

#[test]
fn matches_sensitive_keys_regardless_of_case_or_separator() {
    let redacted = DiagnosticRedactor::new().redact_json(&json!({
        "Password": "a",
        "PRIVATE_KEY": "b",
        "Authorization": "Bearer c",
        "authStr": "d",
        "auth_str": "e"
    }));

    for key in [
        "Password",
        "PRIVATE_KEY",
        "Authorization",
        "authStr",
        "auth_str",
    ] {
        assert_eq!(redacted[key], REDACTED, "{key} must be masked");
    }
}

#[test]
fn masks_a_known_secret_value_even_under_an_unrecognized_key() {
    let redacted = DiagnosticRedactor::new()
        .with_secret("super-secret-token")
        .redact_json(&json!({
            "some_future_field": "super-secret-token",
            "note": "prefix super-secret-token suffix",
            "unrelated": "keep me"
        }));

    assert_eq!(redacted["some_future_field"], REDACTED);
    assert_eq!(redacted["note"], format!("prefix {REDACTED} suffix"));
    assert_eq!(redacted["unrelated"], "keep me");
}

#[test]
fn masks_non_string_values_under_sensitive_keys() {
    let redacted = DiagnosticRedactor::new().redact_json(&json!({
        "password": 1234,
        "uuid": null,
        "auth": ["a", "b"]
    }));

    assert_eq!(redacted["password"], REDACTED);
    assert_eq!(redacted["uuid"], REDACTED);
    assert_eq!(redacted["auth"], REDACTED);
}

#[test]
fn ignores_secrets_too_short_to_match_without_destroying_the_bundle() {
    let redacted = DiagnosticRedactor::new()
        .with_secret("ab")
        .redact_json(&json!({ "server": "ab.example.com" }));

    assert_eq!(redacted["server"], "ab.example.com");
}

#[test]
fn redacts_log_text_as_well_as_json() {
    let redactor = DiagnosticRedactor::new().with_secret("runtime-secret");

    assert_eq!(
        redactor.redact_text("core failed: password=runtime-secret rejected"),
        format!("core failed: password={REDACTED} rejected")
    );
}

#[test]
fn a_real_runtime_config_carries_no_credential_after_redaction() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-128-gcm:runtime-secret@edge.example.com:8388")
        .unwrap();
    let credential = parsed.credential().clone();
    let mut node = ProxyNode::new(
        Uuid::nil(),
        "Diagnostics",
        ProxyProtocol::Shadowsocks,
        "edge.example.com",
        8388,
        Some(CredentialRef::new("secret://nodes/diagnostics").unwrap()),
    )
    .unwrap();
    node.transport = Some(TransportConfig::Tcp);
    let dns = DnsProfile::new(
        vec![DnsServer::system("system").unwrap()],
        Vec::new(),
        "system",
        DnsStrategy::PreferIpv4,
        false,
        false,
    )
    .unwrap();
    let route = RouteProfile::new(RoutingMode::Global, Vec::new(), RouteOutbound::Proxy).unwrap();
    let profile = SingBoxRuntimeProfile::new(
        &node,
        magies_profiles::NodeCredential::Shadowsocks(&credential),
        &dns,
        &route,
    );
    let generated = SingBoxRuntimeConfigGenerator::generate(&profile).unwrap();

    let raw = serde_json::to_string(generated.json()).unwrap();
    assert!(
        raw.contains("runtime-secret"),
        "the fixture must be a real secret-bearing config"
    );

    let redacted = DiagnosticRedactor::new()
        .with_secret("runtime-secret")
        .redact_json(generated.json());

    let text = serde_json::to_string(&redacted).unwrap();
    assert!(
        !text.contains("runtime-secret"),
        "credential leaked: {text}"
    );
    assert!(
        text.contains("edge.example.com"),
        "the bundle must stay useful"
    );
}
