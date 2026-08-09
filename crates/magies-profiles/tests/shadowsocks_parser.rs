use base64::{Engine as _, engine::general_purpose};
use magies_domain::{CredentialRef, ProxyProtocol, TransportConfig};
use magies_profiles::{ShadowsocksParseError, ShadowsocksParser, VlessParseError};
use uuid::Uuid;

const PASSWORD: &str = "secret:password";
const NODE_ID: &str = "018f78b5-08ee-7caa-94f3-1d5d781aba23";

#[test]
fn recognizes_only_the_exact_shadowsocks_scheme() {
    let parser = ShadowsocksParser;

    assert!(parser.can_parse("  ss://YWVzLTI1Ni1nY206cGFzc3dvcmQ@example.com:8388  "));
    assert!(!parser.can_parse("trojan://password@example.com:443"));
    assert!(!parser.can_parse("SS://YWVzLTI1Ni1nY206cGFzc3dvcmQ@example.com:8388"));
}

#[test]
fn parses_sip002_base64_user_info_and_materializes_after_secret_storage() {
    let user_info = general_purpose::URL_SAFE_NO_PAD.encode(format!("aes-256-gcm:{PASSWORD}"));
    let parsed = ShadowsocksParser
        .parse(&format!(
            "ss://{user_info}@edge.example.com:8388/#US%20West"
        ))
        .unwrap();

    assert_eq!(parsed.name().as_str(), "US West");
    assert_eq!(parsed.server().as_str(), "edge.example.com");
    assert_eq!(parsed.port().get(), 8388);
    assert_eq!(parsed.transport(), &TransportConfig::Tcp);
    assert_eq!(parsed.tls(), None);
    assert_eq!(parsed.credential().method(), "aes-256-gcm");
    assert_eq!(parsed.credential().password(), PASSWORD);

    let credential_ref = CredentialRef::new("keychain://nodes/shadowsocks-password").unwrap();
    let node = parsed
        .into_proxy_node(Uuid::parse_str(NODE_ID).unwrap(), credential_ref)
        .unwrap();

    assert_eq!(node.protocol_type, ProxyProtocol::Shadowsocks);
    assert_eq!(node.name.as_str(), "US West");
    assert_eq!(node.server.as_str(), "edge.example.com");
    assert_eq!(node.transport, Some(TransportConfig::Tcp));
    assert_eq!(node.tls, None);
    assert!(node.udp_enabled);
    assert_eq!(
        node.credential_ref.as_str(),
        "keychain://nodes/shadowsocks-password"
    );
}

#[test]
fn parses_plain_user_info_required_by_aead_2022_and_ipv6() {
    let parsed = ShadowsocksParser
        .parse(
            "ss://2022-blake3-aes-256-gcm:\
             YctPZ6U7xPPcU%2Bgp3u%2B0tx%2FtRizJN9K8y%2BuKlW2qjlI%3D\
             @[2001:db8::1]:8443#AEAD%202022",
        )
        .unwrap();

    assert_eq!(parsed.name().as_str(), "AEAD 2022");
    assert_eq!(parsed.server().as_str(), "2001:db8::1");
    assert_eq!(parsed.port().get(), 8443);
    assert_eq!(parsed.credential().method(), "2022-blake3-aes-256-gcm");
    assert_eq!(
        parsed.credential().password(),
        "YctPZ6U7xPPcU+gp3u+0tx/tRizJN9K8y+uKlW2qjlI="
    );
}

#[test]
fn parses_percent_encoded_plain_password_for_classic_aead() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-128-gcm:p%40ss%3Aword@example.com:443")
        .unwrap();

    assert_eq!(parsed.name().as_str(), "example.com:443");
    assert_eq!(parsed.credential().method(), "aes-128-gcm");
    assert_eq!(parsed.credential().password(), "p@ss:word");
}

#[test]
fn parses_v2rayn_legacy_whole_payload_base64() {
    let payload =
        general_purpose::STANDARD.encode("chacha20-ietf-poly1305:pa:ss@[2001:db8::5]:9443");
    let parsed = ShadowsocksParser
        .parse(&format!("ss://{payload}#Legacy%20IPv6"))
        .unwrap();

    assert_eq!(parsed.name().as_str(), "Legacy IPv6");
    assert_eq!(parsed.server().as_str(), "2001:db8::5");
    assert_eq!(parsed.port().get(), 9443);
    assert_eq!(parsed.credential().method(), "chacha20-ietf-poly1305");
    assert_eq!(parsed.credential().password(), "pa:ss");
}

#[test]
fn accepts_supported_xray_and_sing_box_methods() {
    for method in [
        "none",
        "plain",
        "aes-192-gcm",
        "xchacha20-ietf-poly1305",
        "aes-256-cfb",
        "rc4-md5",
        "2022-blake3-chacha20-poly1305",
    ] {
        let user_info = general_purpose::URL_SAFE_NO_PAD.encode(format!("{method}:password"));
        let parsed = ShadowsocksParser
            .parse(&format!("ss://{user_info}@example.com:8388"))
            .unwrap();

        assert_eq!(parsed.credential().method(), method);
    }
}

#[test]
fn redacts_method_and_password_from_debug_output() {
    let parsed = ShadowsocksParser
        .parse("ss://aes-256-gcm:top-secret@example.com:8388")
        .unwrap();

    let credential_debug = format!("{:?}", parsed.credential());
    let parsed_debug = format!("{parsed:?}");
    for debug_output in [credential_debug, parsed_debug] {
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("aes-256-gcm"));
        assert!(!debug_output.contains("top-secret"));
    }
}

#[test]
fn rejects_plugins_duplicate_and_unknown_query_parameters() {
    let user_info = general_purpose::URL_SAFE_NO_PAD.encode("aes-256-gcm:password");
    let plugin = ShadowsocksParser
        .parse(&format!(
            "ss://{user_info}@example.com:8388/?plugin=v2ray-plugin%3Btls"
        ))
        .unwrap_err();
    let duplicate = ShadowsocksParser
        .parse(&format!(
            "ss://{user_info}@example.com:8388?plugin=a&plugin=b"
        ))
        .unwrap_err();
    let unknown = ShadowsocksParser
        .parse(&format!("ss://{user_info}@example.com:8388?udp-over-tcp=1"))
        .unwrap_err();

    assert!(matches!(plugin, ShadowsocksParseError::UnsupportedPlugin));
    assert!(matches!(
        &duplicate,
        ShadowsocksParseError::InvalidUri {
            source: VlessParseError::DuplicateParameter { name }
        } if name == "plugin"
    ));
    assert!(matches!(
        &unknown,
        ShadowsocksParseError::InvalidUri {
            source: VlessParseError::UnsupportedParameter { name }
        } if name == "udp-over-tcp"
    ));
}

#[test]
fn rejects_invalid_credentials_without_exposing_them_in_errors() {
    let invalid_utf8 = ShadowsocksParser
        .parse("ss://____@example.com:8388")
        .unwrap_err();
    let missing_separator = encoded_user_info_error("aes-256-gcm");
    let missing_method = encoded_user_info_error(":password");
    let missing_password = encoded_user_info_error("aes-256-gcm:");
    let unsupported_method = encoded_user_info_error("rot13:private-password");

    assert!(matches!(invalid_utf8, ShadowsocksParseError::InvalidUtf8));
    assert!(matches!(
        missing_separator,
        ShadowsocksParseError::MissingCredentialSeparator
    ));
    assert!(matches!(
        missing_method,
        ShadowsocksParseError::MissingMethod
    ));
    assert!(matches!(
        missing_password,
        ShadowsocksParseError::MissingPassword
    ));
    assert!(matches!(
        &unsupported_method,
        ShadowsocksParseError::UnsupportedMethod { value } if value == "rot13"
    ));
    assert!(!format!("{unsupported_method:?}").contains("private-password"));
}

#[test]
fn rejects_invalid_payload_endpoint_path_and_percent_encoding() {
    let user_info = general_purpose::URL_SAFE_NO_PAD.encode("aes-256-gcm:password");
    let legacy_missing_endpoint = general_purpose::STANDARD.encode("aes-256-gcm:password");
    let cases = [
        "trojan://password@example.com:443".to_owned(),
        "ss://".to_owned(),
        "ss://not_base64@example.com:8388".to_owned(),
        format!("ss://{legacy_missing_endpoint}"),
        format!("ss://{user_info}@example.com"),
        format!("ss://{user_info}@example.com:8388/not-allowed"),
        format!("ss://{user_info}@example.com:8388#bad%ZZtag"),
    ];
    let errors = cases.map(|uri| ShadowsocksParser.parse(&uri).unwrap_err());

    assert!(matches!(
        errors[0],
        ShadowsocksParseError::UnsupportedScheme
    ));
    assert!(matches!(errors[1], ShadowsocksParseError::EmptyPayload));
    assert!(matches!(
        errors[2],
        ShadowsocksParseError::InvalidBase64 { .. }
    ));
    assert!(matches!(
        errors[3],
        ShadowsocksParseError::MissingEndpointSeparator
    ));
    assert!(matches!(
        &errors[4],
        ShadowsocksParseError::InvalidUri {
            source: VlessParseError::MissingPort
        }
    ));
    assert!(matches!(errors[5], ShadowsocksParseError::UnexpectedPath));
    assert!(matches!(
        &errors[6],
        ShadowsocksParseError::InvalidUri {
            source: VlessParseError::InvalidPercentEncoding
        }
    ));
}

fn encoded_user_info_error(credentials: &str) -> ShadowsocksParseError {
    let user_info = general_purpose::URL_SAFE_NO_PAD.encode(credentials);
    ShadowsocksParser
        .parse(&format!("ss://{user_info}@example.com:8388"))
        .unwrap_err()
}
