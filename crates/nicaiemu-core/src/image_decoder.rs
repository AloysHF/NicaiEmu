//! Image Decoder for CBE Resources
//!
//! Handles decoding of CBE image resources (GIF with custom RGB565 palettes).

use anyhow::{Context, Result};
use log::debug;

/// Decoded image (RGBA)
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Convert RGB565 to RGB888
fn rgb565_to_rgb888(v: u16) -> (u8, u8, u8) {
    let r5 = (v >> 11) & 0x1F;
    let g6 = (v >> 5) & 0x3F;
    let b5 = v & 0x1F;
    (
        ((r5 << 3) | (r5 >> 2)) as u8,
        ((g6 << 2) | (g6 >> 4)) as u8,
        ((b5 << 3) | (b5 >> 2)) as u8,
    )
}

/// Decode a GIF image from CBE resource bytes
pub fn decode_image(data: &[u8]) -> Result<DecodedImage> {
    if data.is_empty() {
        anyhow::bail!("Empty image data");
    }

    // Check if it's a standard GIF (GIF87a or GIF89a)
    let is_standard_gif = data.len() >= 6 && (data[..6] == *b"GIF89a" || data[..6] == *b"GIF87a");

    if is_standard_gif {
        debug!("Decoding standard GIF");
        return decode_standard_gif(data);
    }

    // CBE custom format: 8-byte metadata + RGB565 palette + GIF blocks
    debug!("Attempting CBE custom GIF format");
    decode_cbe_gif(data)
}

/// Decode a standard GIF image
fn decode_standard_gif(data: &[u8]) -> Result<DecodedImage> {
    let img = image::load_from_memory(data).context("Failed to decode standard GIF")?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(DecodedImage {
        width,
        height,
        data: rgba.into_raw(),
    })
}

/// Decode a CBE custom GIF format
fn decode_cbe_gif(data: &[u8]) -> Result<DecodedImage> {
    if data.len() < 7 {
        anyhow::bail!("CBE GIF header is truncated");
    }
    let flags = data[4];
    if flags & 0x80 == 0 {
        anyhow::bail!("CBE GIF has no local palette");
    }
    let color_count = 1usize << ((flags & 7) + 1);

    // Look for GIF graphic control extension (0x21 0xF9 0x04)
    let gce_pos = data
        .windows(3)
        .position(|w| w == [0x21, 0xF9, 0x04])
        .context("Could not find GIF graphic control extension")?;

    let palette_start = 7;
    let palette_end = palette_start + color_count * 2;
    if gce_pos != palette_end || palette_end > data.len() {
        anyhow::bail!("Invalid CBE GIF format: GCE too early");
    }
    if !(2..=256).contains(&color_count) {
        anyhow::bail!("Invalid palette: {} colors", color_count);
    }

    // Find image descriptor (0x2C)
    let image_start = gce_pos + 8;
    if image_start >= data.len() || data[image_start] != 0x2C {
        anyhow::bail!("Could not find image descriptor");
    }

    let width = u16::from_le_bytes([data[image_start + 5], data[image_start + 6]]) as u32;
    let height = u16::from_le_bytes([data[image_start + 7], data[image_start + 8]]) as u32;

    if width == 0 || width > 4096 || height == 0 || height > 4096 {
        anyhow::bail!("Invalid image dimensions: {}x{}", width, height);
    }

    debug!("CBE GIF: {}x{}, {} colors", width, height, color_count);

    // Build standard GIF header
    let color_bits = color_count.ilog2() as u8;
    let mut gif_data = Vec::with_capacity(data.len() + 100);

    // GIF89a header
    gif_data.extend_from_slice(b"GIF89a");
    // Width and height (little-endian)
    gif_data.extend_from_slice(&(width as u16).to_le_bytes());
    gif_data.extend_from_slice(&(height as u16).to_le_bytes());
    // Global color table flag, color resolution, sort, size of GCT
    gif_data.push(0x80 | ((color_bits - 1) << 4) | (color_bits - 1));
    // Background color index
    gif_data.push(0);
    // Pixel aspect ratio
    gif_data.push(0);

    // Global color table (RGB888 from RGB565)
    for i in 0..color_count {
        let rgb565 =
            u16::from_be_bytes([data[palette_start + i * 2], data[palette_start + i * 2 + 1]]);
        let (r, g, b) = rgb565_to_rgb888(rgb565);
        gif_data.push(r);
        gif_data.push(g);
        gif_data.push(b);
    }

    // Copy remaining GIF blocks (from GCE onwards)
    gif_data.extend_from_slice(&data[gce_pos..]);

    // Decode with image crate
    let img = image::load_from_memory(&gif_data).context("Failed to decode rebuilt CBE GIF")?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();

    Ok(DecodedImage {
        width: w,
        height: h,
        data: rgba.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb565_conversion() {
        // Pure red: 0xF800
        let (r, g, b) = rgb565_to_rgb888(0xF800);
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);

        // Pure green: 0x07E0
        let (r, g, b) = rgb565_to_rgb888(0x07E0);
        // RGB565 pure green: 5 bits = 0x1F, expanded to 8 bits = 252
        assert_eq!(r, 0);
        assert!(g >= 252); // Allow small rounding differences
        assert_eq!(b, 0);

        // Pure blue: 0x001F
        let (r, g, b) = rgb565_to_rgb888(0x001F);
        assert_eq!(r, 0);
        assert_eq!(g, 0);
        assert_eq!(b, 255);

        // White: 0xFFFF
        let (r, g, b) = rgb565_to_rgb888(0xFFFF);
        assert!(r >= 252);
        assert!(g >= 252);
        assert!(b >= 252);
    }
}
