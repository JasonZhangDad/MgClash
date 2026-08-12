//! Renders a sharing link as a QR code the user can point a phone at.
//!
//! SVG rather than a bitmap: the webview's CSP blocks every external host, so
//! the markup has to be self-contained, and vector output stays sharp at
//! whatever size the dialog gives it without the app choosing a resolution.

use qrcode::QrCode;
use qrcode::render::svg;
use qrcode::types::QrError;
use thiserror::Error;

/// The smallest edge the rendered square may have, in SVG user units.
///
/// A minimum rather than an exact size: modules are whole units, so the renderer
/// rounds up rather than scaling one to a fraction and blurring its edges. The
/// dialog scales the markup with CSS from there, so nothing has to be re-rendered
/// when the window moves to another display.
const MINIMUM_SIZE: u32 = 256;

/// Renders one sharing link.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ShareLinkQrCode;

impl ShareLinkQrCode {
    /// Encodes `link` as a self-contained SVG document.
    ///
    /// # Errors
    ///
    /// Returns [`ShareLinkQrError::EmptyLink`] for a blank link, and
    /// [`ShareLinkQrError::TooLong`] when the link exceeds what a QR code can
    /// hold.
    pub fn svg(link: &str) -> Result<String, ShareLinkQrError> {
        let link = link.trim();
        if link.is_empty() {
            return Err(ShareLinkQrError::EmptyLink);
        }
        let code = QrCode::new(link.as_bytes()).map_err(|source| match source {
            QrError::DataTooLong => ShareLinkQrError::TooLong { length: link.len() },
            source => ShareLinkQrError::Encode {
                reason: source.to_string(),
            },
        })?;

        Ok(code
            .render()
            .min_dimensions(MINIMUM_SIZE, MINIMUM_SIZE)
            // Named colours rather than a theme's: a code has to keep its
            // contrast whatever the dialog behind it is painted with.
            .dark_color(svg::Color("#000000"))
            .light_color(svg::Color("#ffffff"))
            .build())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ShareLinkQrError {
    #[error("a QR code needs a link to encode")]
    EmptyLink,
    #[error("the link is {length} bytes, more than a QR code can hold")]
    TooLong { length: usize },
    #[error("the link could not be encoded: {reason}")]
    Encode { reason: String },
}

/// Reads a sharing link out of a QR code image.
///
/// Screen capture is deliberately absent: it needs a Screen Recording grant that
/// macOS binds to an unsigned binary's path and hash, so the permission would be
/// lost every time the app is moved or rebuilt. An image the user picked needs no
/// permission at all.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ShareLinkQrScanner;

impl ShareLinkQrScanner {
    /// Decodes the first QR code found in a PNG or JPEG image.
    ///
    /// # Errors
    ///
    /// Returns [`ShareLinkQrScanError::UnreadableImage`] when the bytes are not
    /// an image this build can decode, and [`ShareLinkQrScanError::NoCodeFound`]
    /// when they are but hold no readable code.
    pub fn read(image: &[u8]) -> Result<String, ShareLinkQrScanError> {
        let decoded = image::load_from_memory(image)
            .map_err(|source| ShareLinkQrScanError::UnreadableImage {
                reason: source.to_string(),
            })?
            .to_luma8();

        let mut prepared = rqrr::PreparedImage::prepare(decoded);
        let grids = prepared.detect_grids();
        // The first readable grid wins: a screenshot can hold more than one code,
        // and asking the user which is worse than importing the one that decoded.
        for grid in grids {
            if let Ok((_, content)) = grid.decode() {
                return Ok(content);
            }
        }
        Err(ShareLinkQrScanError::NoCodeFound)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ShareLinkQrScanError {
    #[error("the file is not an image this build can read: {reason}")]
    UnreadableImage { reason: String },
    #[error("no QR code was found in the image")]
    NoCodeFound,
}
