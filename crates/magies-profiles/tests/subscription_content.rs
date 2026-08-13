use base64::{Engine as _, engine::general_purpose};
use magies_domain::ProxyProtocol;
use magies_profiles::{ShareLinkParseError, SubscriptionContentError, SubscriptionContentParser};
use uuid::Uuid;

const SUBSCRIPTION_ID: &str = "018f78b5-2cd0-7000-a9a6-3bccf60951e8";
const USER_ID: &str = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";

fn subscription_id() -> Uuid {
    Uuid::parse_str(SUBSCRIPTION_ID).unwrap()
}

#[test]
fn parses_every_plain_text_line_into_a_subscription_owned_node() {
    let content = format!(
        "\r\nvless://{USER_ID}@edge.example.com:443#Tokyo\r\n\
         trojan://secret@osaka.example.com:8443#Osaka\r\n"
    );

    let parsed = SubscriptionContentParser
        .parse(content.as_bytes(), subscription_id())
        .unwrap();

    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].node().name.as_str(), "Tokyo");
    assert_eq!(parsed[0].node().protocol_type, ProxyProtocol::Vless);
    assert_eq!(parsed[1].node().name.as_str(), "Osaka");
    assert_eq!(parsed[1].node().protocol_type, ProxyProtocol::Trojan);
    for node in &parsed {
        assert_eq!(node.node().subscription_id, Some(subscription_id()));
        assert!(
            node.node()
                .credential_ref
                .as_str()
                .starts_with(&format!("subscription/{SUBSCRIPTION_ID}/node/"))
        );
        assert_eq!(node.node().protocol_type, node.credential().protocol());
    }
    assert_ne!(parsed[0].node().id, parsed[1].node().id);
}

#[test]
fn parses_an_unpadded_base64_subscription() {
    let plain = format!(
        "vless://{USER_ID}@edge.example.com:443#Tokyo\n\
         hysteria2://secret@hy.example.com:443#Hong%20Kong"
    );
    let encoded = general_purpose::URL_SAFE_NO_PAD.encode(plain);

    let parsed = SubscriptionContentParser
        .parse(encoded.as_bytes(), subscription_id())
        .unwrap();

    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].node().protocol_type, ProxyProtocol::Vless);
    assert_eq!(parsed[1].node().protocol_type, ProxyProtocol::Hysteria2);
}

#[test]
fn rejects_the_whole_snapshot_when_any_line_is_invalid() {
    let content = format!(
        "vless://{USER_ID}@edge.example.com:443#Tokyo\n\
         ssr://token@edge.example.com:443"
    );

    let error = SubscriptionContentParser
        .parse(content.as_bytes(), subscription_id())
        .unwrap_err();

    assert!(matches!(
        error,
        SubscriptionContentError::InvalidLine {
            line: 2,
            source: ShareLinkParseError::UnsupportedScheme,
        }
    ));
}

#[test]
fn reports_empty_invalid_base64_and_non_utf8_content_without_echoing_it() {
    assert!(matches!(
        SubscriptionContentParser.parse(b" \r\n", subscription_id()),
        Err(SubscriptionContentError::Empty)
    ));
    assert!(matches!(
        SubscriptionContentParser.parse(b"not base64!", subscription_id()),
        Err(SubscriptionContentError::InvalidBase64 { .. })
    ));
    assert!(matches!(
        SubscriptionContentParser.parse(&[0xff], subscription_id()),
        Err(SubscriptionContentError::InvalidUtf8 { .. })
    ));
}
