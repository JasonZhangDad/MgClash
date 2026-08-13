use base64::{Engine as _, engine::general_purpose};
use magies_domain::{CredentialRef, ProxyProtocol};
use magies_profiles::{
    ShareLinkParseError, ShareLinkParser, StoredNodeCredential, VlessParseError,
};
use serde_json::json;
use uuid::Uuid;

const USER_ID: &str = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";
const NODE_ID: &str = "018f78b5-08ee-7caa-94f3-1d5d781aba22";

fn node_id() -> Uuid {
    Uuid::parse_str(NODE_ID).unwrap()
}

fn credential_ref() -> CredentialRef {
    CredentialRef::new("keychain://nodes/share-link").unwrap()
}

fn parse(value: &str) -> Result<magies_profiles::ParsedShareLink, ShareLinkParseError> {
    ShareLinkParser.parse(value, node_id(), credential_ref())
}

#[test]
fn dispatches_a_vless_link_to_the_vless_parser() {
    let parsed = parse(&format!("vless://{USER_ID}@edge.example.com:443#US%20West")).unwrap();

    assert_eq!(parsed.node().protocol_type, ProxyProtocol::Vless);
    assert_eq!(parsed.node().name.as_str(), "US West");
    assert_eq!(parsed.node().server.as_str(), "edge.example.com");
    assert_eq!(parsed.node().port.get(), 443);
    assert_eq!(parsed.node().id, node_id());
    assert!(matches!(
        parsed.credential(),
        StoredNodeCredential::Vless(_)
    ));
}

#[test]
fn dispatches_a_vmess_link_to_the_vmess_parser() {
    let parsed = parse(&format!("vmess://{USER_ID}@edge.example.com:443#Tokyo")).unwrap();

    assert_eq!(parsed.node().protocol_type, ProxyProtocol::Vmess);
    assert_eq!(parsed.node().name.as_str(), "Tokyo");
    assert!(matches!(
        parsed.credential(),
        StoredNodeCredential::Vmess(_)
    ));
}

#[test]
fn dispatches_a_legacy_base64_vmess_link_to_the_vmess_parser() {
    let payload = json!({
        "v": "2",
        "ps": "Legacy",
        "add": "edge.example.com",
        "port": "443",
        "id": USER_ID,
        "aid": "0",
        "net": "tcp"
    });
    let uri = format!(
        "vmess://{}",
        general_purpose::STANDARD.encode(serde_json::to_vec(&payload).unwrap())
    );

    let parsed = parse(&uri).unwrap();

    assert_eq!(parsed.node().protocol_type, ProxyProtocol::Vmess);
    assert_eq!(parsed.node().name.as_str(), "Legacy");
}

#[test]
fn dispatches_a_trojan_link_to_the_trojan_parser() {
    let parsed = parse("trojan://secret%3Apassword@edge.example.com:443#US%20West").unwrap();

    assert_eq!(parsed.node().protocol_type, ProxyProtocol::Trojan);
    assert!(matches!(
        parsed.credential(),
        StoredNodeCredential::Trojan(_)
    ));
}

#[test]
fn dispatches_a_shadowsocks_link_to_the_shadowsocks_parser() {
    let user_info = general_purpose::URL_SAFE_NO_PAD.encode("aes-256-gcm:secret:password");
    let parsed = parse(&format!(
        "ss://{user_info}@edge.example.com:8388/#US%20West"
    ))
    .unwrap();

    assert_eq!(parsed.node().protocol_type, ProxyProtocol::Shadowsocks);
    assert_eq!(parsed.node().port.get(), 8388);
    assert!(matches!(
        parsed.credential(),
        StoredNodeCredential::Shadowsocks(_)
    ));
}

#[test]
fn dispatches_both_hysteria2_schemes_to_the_hysteria2_parser() {
    for uri in [
        "hysteria2://secret%3Atoken@edge.example.com#Default",
        "hy2://secret%3Atoken@edge.example.com:8443#Default",
    ] {
        let parsed = parse(uri).unwrap();

        assert_eq!(parsed.node().protocol_type, ProxyProtocol::Hysteria2);
        assert!(matches!(
            parsed.credential(),
            StoredNodeCredential::Hysteria2(_)
        ));
    }
}

#[test]
fn dispatches_a_tuic_link_to_the_tuic_parser() {
    let parsed = parse(&format!(
        "tuic://{USER_ID}:hunter2@edge.example.com:443#Tokyo"
    ))
    .unwrap();

    assert_eq!(parsed.node().protocol_type, ProxyProtocol::Tuic);
    assert!(matches!(parsed.credential(), StoredNodeCredential::Tuic(_)));
}

#[test]
fn rejects_an_unknown_scheme_before_reaching_a_parser() {
    assert!(matches!(
        parse("wireguard://token@edge.example.com:443"),
        Err(ShareLinkParseError::UnsupportedScheme)
    ));
    assert!(matches!(
        parse("VLESS://token@edge.example.com:443"),
        Err(ShareLinkParseError::UnsupportedScheme)
    ));
    assert!(matches!(
        parse("   "),
        Err(ShareLinkParseError::UnsupportedScheme)
    ));
}

#[test]
fn surfaces_the_selected_parser_error() {
    assert!(matches!(
        parse("vless://edge.example.com:443"),
        Err(ShareLinkParseError::Vless(VlessParseError::MissingUserId))
    ));
    assert!(matches!(
        parse("trojan://@edge.example.com:443"),
        Err(ShareLinkParseError::Trojan(_))
    ));
    assert!(matches!(
        parse("ss://not-base64@edge.example.com:8388"),
        Err(ShareLinkParseError::Shadowsocks(_))
    ));
    assert!(matches!(
        parse("hy2://token@edge.example.com:0"),
        Err(ShareLinkParseError::Hysteria2(_))
    ));
    assert!(matches!(
        parse("vmess://not-base64-json"),
        Err(ShareLinkParseError::Vmess(_))
    ));
}

#[test]
fn keeps_the_parsed_node_and_credential_together_when_split() {
    let parsed = parse(&format!("vless://{USER_ID}@edge.example.com:443#US%20West")).unwrap();
    let (node, credential) = parsed.into_parts();

    assert_eq!(node.protocol_type, credential.protocol());
    assert_eq!(node.credential_ref.as_str(), credential_ref().as_str());
}
