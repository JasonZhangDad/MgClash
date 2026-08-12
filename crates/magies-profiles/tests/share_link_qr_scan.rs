//! Covers reading a sharing link back out of a QR code image.

use magies_profiles::{ShareLinkQrScanError, ShareLinkQrScanner};

const LINK: &str = "ss://aes-256-gcm:hunter2@edge.example.com:8388#Tokyo";

/// A PNG of `link`, the way a user receives one.
fn qr_png(link: &str) -> Vec<u8> {
    // Rendered here rather than by the app's own SVG encoder: a scanner that
    // only reads its own output proves nothing about images from elsewhere.
    let code = qrcode::QrCode::new(link.as_bytes()).unwrap();
    let image = code
        .render::<image::Luma<u8>>()
        .min_dimensions(320, 320)
        .build();
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    png
}

#[test]
fn a_photographed_code_gives_back_the_link() {
    let png = qr_png(LINK);

    let link = ShareLinkQrScanner::read(&png).unwrap();

    assert_eq!(link, LINK);
}

#[test]
fn a_long_link_survives_the_round_trip() {
    let long = "hysteria2://hunter2@edge.example.com:5555?sni=www.example.com\
                &pinSHA256=6ff212bbab490b686b06209c6074865f9340f4c0f9c4aa7d34d568c2a2cebe73\
                #\u{1f1ed}\u{1f1f0}Hong Kong";
    let png = qr_png(long);

    assert_eq!(ShareLinkQrScanner::read(&png).unwrap(), long);
}

#[test]
fn an_image_with_no_code_is_a_typed_error() {
    let blank = image::RgbImage::new(64, 64);
    let mut png = Vec::new();
    blank
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();

    assert_eq!(
        ShareLinkQrScanner::read(&png),
        Err(ShareLinkQrScanError::NoCodeFound)
    );
}

#[test]
fn something_that_is_not_an_image_is_a_typed_error() {
    // The file picker accepts whatever the user chose, so this is the ordinary
    // case of picking the wrong file rather than a corrupt one.
    assert!(matches!(
        ShareLinkQrScanner::read(b"ss://aes-256-gcm:hunter2@edge.example.com:8388"),
        Err(ShareLinkQrScanError::UnreadableImage { .. })
    ));
}
