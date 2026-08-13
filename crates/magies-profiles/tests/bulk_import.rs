//! Covers bulk node import: the two body envelopes, per-line error reporting,
//! in-body deduplication, and the manual-node ownership the parser guarantees.

use base64::{Engine as _, engine::general_purpose};
use magies_domain::ProxyProtocol;
use magies_profiles::{BulkImportError, BulkImportLineError, BulkNodeImportParser};

const TOKYO: &str = "ss://aes-128-gcm:secret@edge.example.com:8388#Tokyo";
const OSAKA: &str = "ss://aes-128-gcm:secret@osaka.example.com:9000#Osaka";
const TROJAN: &str = "trojan://hunter2@frankfurt.example.com:443#Frankfurt";

fn parse(content: &str) -> magies_profiles::BulkImportOutcome {
    BulkNodeImportParser.parse(content.as_bytes()).unwrap()
}

#[test]
fn imports_every_line_of_a_plain_list() {
    let outcome = parse(&format!("{TOKYO}\n{OSAKA}\n{TROJAN}"));

    assert_eq!(outcome.nodes.len(), 3);
    assert!(outcome.failures.is_empty());
    assert_eq!(outcome.duplicates, 0);

    let names: Vec<_> = outcome
        .nodes
        .iter()
        .map(|node| node.node().name.as_str().to_owned())
        .collect();
    assert_eq!(names, ["Tokyo", "Osaka", "Frankfurt"]);
    assert_eq!(outcome.nodes[2].node().protocol_type, ProxyProtocol::Trojan);
}

#[test]
fn imported_nodes_are_manual_and_independently_addressed() {
    let outcome = parse(&format!("{TOKYO}\n{OSAKA}"));

    for parsed in &outcome.nodes {
        let node = parsed.node();
        // Manual ownership is what keeps these nodes editable and deletable.
        assert!(node.subscription_id.is_none());
        assert!(node.group_id.is_none());
        assert!(node.enabled);
        assert!(
            node.credential_ref.as_str().starts_with("node/"),
            "expected a manual credential reference, got {:?}",
            node.credential_ref.as_str()
        );
    }

    let first = outcome.nodes[0].node();
    let second = outcome.nodes[1].node();
    assert_ne!(first.id, second.id);
    assert_ne!(
        first.credential_ref.as_str(),
        second.credential_ref.as_str()
    );
}

#[test]
fn decodes_a_whole_body_base64_envelope() {
    let encoded = general_purpose::STANDARD.encode(format!("{TOKYO}\n{OSAKA}"));

    let outcome = parse(&encoded);

    assert_eq!(outcome.nodes.len(), 2);
    assert!(outcome.failures.is_empty());
}

#[test]
fn skips_blank_lines_and_a_leading_byte_order_mark() {
    let outcome = parse(&format!("\u{feff}{TOKYO}\n\n   \n{OSAKA}\n"));

    assert_eq!(outcome.nodes.len(), 2);
    assert!(outcome.failures.is_empty());
}

#[test]
fn reports_a_bad_line_and_keeps_the_rest() {
    let outcome = parse(&format!("{TOKYO}\nnot a link\n{OSAKA}"));

    assert_eq!(outcome.nodes.len(), 2);
    assert_eq!(outcome.failures.len(), 1);

    let failure = &outcome.failures[0];
    assert_eq!(failure.line, 2);
    assert!(matches!(
        failure.reason,
        BulkImportLineError::InvalidLink { .. }
    ));
}

#[test]
fn reports_the_line_number_of_the_original_body() {
    let outcome = parse(&format!("{TOKYO}\n\nnot a link\n{OSAKA}"));

    assert_eq!(outcome.failures.len(), 1);
    // Blank lines are skipped but still counted, so the number matches what the
    // user sees in their editor.
    assert_eq!(outcome.failures[0].line, 3);
}

#[test]
fn a_body_where_every_line_fails_is_not_an_error() {
    // Both lines carry a scheme, so the body reads as plain text and each line
    // is rejected on its own. Neither `naive` nor `ftp` is a supported P0/P1
    // scheme.
    let outcome = parse("ssr://example.com\nftp://example.com");

    assert!(outcome.nodes.is_empty());
    assert_eq!(outcome.failures.len(), 2);
    assert_eq!(outcome.failures[0].line, 1);
    assert_eq!(outcome.failures[1].line, 2);
}

#[test]
fn drops_a_node_repeated_inside_the_same_body() {
    let outcome = parse(&format!("{TOKYO}\n{OSAKA}\n{TOKYO}"));

    assert_eq!(outcome.nodes.len(), 2);
    assert_eq!(outcome.duplicates, 1);
    assert!(outcome.failures.is_empty());
}

#[test]
fn keeps_two_accounts_that_differ_only_by_credential() {
    let other_password = "ss://aes-128-gcm:other-secret@edge.example.com:8388#Tokyo";

    let outcome = parse(&format!("{TOKYO}\n{other_password}"));

    // Same server and port, different credential: two real nodes.
    assert_eq!(outcome.nodes.len(), 2);
    assert_eq!(outcome.duplicates, 0);
}

#[test]
fn keeps_the_same_credential_on_two_different_endpoints() {
    let outcome = parse(&format!("{TOKYO}\n{OSAKA}"));

    assert_eq!(outcome.nodes.len(), 2);
    assert_eq!(outcome.duplicates, 0);
}

#[test]
fn a_differing_name_alone_does_not_make_a_new_node() {
    let renamed = "ss://aes-128-gcm:secret@edge.example.com:8388#Tokyo%20Edge";

    let outcome = parse(&format!("{TOKYO}\n{renamed}"));

    // The label is not part of what identifies a node.
    assert_eq!(outcome.nodes.len(), 1);
    assert_eq!(outcome.duplicates, 1);
}

#[test]
fn rejects_an_empty_body() {
    assert!(matches!(
        BulkNodeImportParser.parse(b"   \n\n  ").unwrap_err(),
        BulkImportError::Empty
    ));
}

#[test]
fn rejects_a_body_that_is_neither_links_nor_base64() {
    assert!(matches!(
        BulkNodeImportParser.parse(b"!!!not base64!!!").unwrap_err(),
        BulkImportError::InvalidBase64 { .. }
    ));
}

#[test]
fn rejects_non_utf8_content() {
    assert!(matches!(
        BulkNodeImportParser.parse(&[0xff, 0xfe, 0xfd]).unwrap_err(),
        BulkImportError::InvalidUtf8 { .. }
    ));
}

#[test]
fn exposes_the_failing_line_through_the_error_source_chain() {
    let outcome = parse("ssr://example.com");
    let failure = &outcome.failures[0];

    assert_eq!(failure.to_string(), "line 1 could not be imported");
    assert_eq!(
        std::error::Error::source(failure).unwrap().to_string(),
        "not a supported sharing link"
    );
}
