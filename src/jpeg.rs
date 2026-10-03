//! Baseline JPEG encoding (RGB input) via the `jpeg-encoder` crate.

use jpeg_encoder::{ColorType, Encoder};

/// Encode a contiguous RGB888 buffer as baseline JPEG.
pub fn encode_rgb(rgb: &[u8], width: u32, height: u32, quality: u8) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let encoder = Encoder::new(&mut output, quality.clamp(1, 100));
    encoder
        .encode(rgb, width as u16, height as u16, ColorType::Rgb)
        .map_err(|e| e.to_string())?;
    Ok(output)
}
