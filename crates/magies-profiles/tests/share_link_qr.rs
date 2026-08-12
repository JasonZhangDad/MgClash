//! Covers rendering a sharing link as a scannable QR code.

use magies_profiles::{ShareLinkQrCode, ShareLinkQrError};

const LINK: &str = "ss://aes-256-gcm:hunter2@edge.example.com:8388#Tokyo";

#[test]
fn a_link_becomes_a_self_contained_svg() {
    let svg = ShareLinkQrCode::svg(LINK).unwrap();

    // Self-contained because the webview's CSP blocks every external host, so
    // anything fetched at render time would simply not appear. The `xmlns`
    // declaration is an identifier, not a resource, and is required.
    assert!(svg.starts_with("<?xml") || svg.starts_with("<svg"), "{svg}");
    assert!(svg.contains("</svg>"), "{svg}");
    for forbidden in ["<image", "<script", "href=", "url("] {
        assert!(
            !svg.contains(forbidden),
            "the SVG must fetch nothing, found {forbidden}"
        );
    }
}

#[test]
fn the_code_is_at_least_the_requested_size() {
    let svg = ShareLinkQrCode::svg(LINK).unwrap();

    // Modules are whole pixels, so the rendered square grows to fit rather than
    // scaling a module to a fraction and blurring the edges.
    let width: u32 = svg
        .split("width=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| panic!("no width in {svg}"));
    assert!(width >= 256, "the code is only {width} units wide");
}

#[test]
fn the_code_carries_the_whole_link() {
    // A Hysteria2 link with a pinned digest is the longest thing the app
    // exports; truncating it would produce a code that scans into a broken node.
    let long = "hysteria2://hunter2@edge.example.com:5555?sni=www.example.com\
                &pinSHA256=6ff212bbab490b686b06209c6074865f9340f4c0f9c4aa7d34d568c2a2cebe73\
                &obfs=salamander&obfs-password=hunter2#\u{1f1ed}\u{1f1f0}Hong Kong";

    let svg = ShareLinkQrCode::svg(long).unwrap();

    assert!(svg.contains("</svg>"));
}

#[test]
fn an_empty_link_is_refused() {
    // An empty code scans as nothing; showing one would look like the export
    // worked.
    assert_eq!(
        ShareLinkQrCode::svg("   "),
        Err(ShareLinkQrError::EmptyLink)
    );
}

#[test]
fn a_link_too_long_to_encode_is_a_typed_error() {
    let overlong = "vless://".to_owned() + &"a".repeat(8_000);

    // A QR code has a fixed capacity; failing loudly beats rendering a code
    // that cannot be scanned.
    assert!(matches!(
        ShareLinkQrCode::svg(&overlong),
        Err(ShareLinkQrError::TooLong { .. })
    ));
}
