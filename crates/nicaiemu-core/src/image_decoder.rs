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

    if data.starts_with(b"\x89PNGGAME") {
        debug!("Decoding firmware PNG");
        let normalized = normalize_firmware_png(data)?;
        return decode_standard_image(&normalized);
    }

    if image::guess_format(data).is_ok() {
        debug!("Decoding standard image");
        return decode_standard_image(data);
    }

    // CBE custom format: 8-byte metadata + RGB565 palette + GIF blocks
    debug!("Attempting CBE custom GIF format");
    decode_cbe_gif(data)
}

fn normalize_firmware_png(data: &[u8]) -> Result<Vec<u8>> {
    const FIRMWARE_SIGNATURE: &[u8; 8] = b"\x89PNGGAME";
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if !data.starts_with(FIRMWARE_SIGNATURE) {
        anyhow::bail!("Invalid firmware PNG signature");
    }

    let mut normalized = Vec::with_capacity(data.len() + 1024);
    normalized.extend_from_slice(PNG_SIGNATURE);
    let mut position = 8usize;
    while position + 8 <= data.len() {
        let declared_length = u32::from_be_bytes(
            data[position..position + 4]
                .try_into()
                .context("Firmware PNG chunk length is truncated")?,
        ) as usize;
        let chunk_type = &data[position + 4..position + 8];
        let is_palette = chunk_type == b"PLTE";
        let source_length = if is_palette {
            (declared_length / 3) * 2
        } else {
            declared_length
        };
        let chunk_end = position
            .checked_add(8 + source_length + 4)
            .context("Firmware PNG chunk length overflow")?;
        if chunk_end > data.len() {
            anyhow::bail!("Firmware PNG chunk is truncated");
        }

        normalized.extend_from_slice(&(declared_length as u32).to_be_bytes());
        normalized.extend_from_slice(chunk_type);
        if is_palette {
            if !declared_length.is_multiple_of(3) {
                anyhow::bail!("Firmware PNG palette length is invalid");
            }
            let palette = &data[position + 8..position + 8 + source_length];
            for color in palette.chunks_exact(2) {
                let (red, green, blue) = rgb565_to_rgb888(u16::from_be_bytes([color[0], color[1]]));
                normalized.extend_from_slice(&[red, green, blue]);
            }
            let crc_start = normalized.len() - declared_length - 4;
            let crc = png_crc32(&normalized[crc_start..]);
            normalized.extend_from_slice(&crc.to_be_bytes());
        } else {
            normalized.extend_from_slice(&data[position + 8..chunk_end]);
        }
        position = chunk_end;
        if chunk_type == b"IEND" {
            return Ok(normalized);
        }
    }
    anyhow::bail!("Firmware PNG has no IEND chunk")
}

fn png_crc32(data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    crc ^ u32::MAX
}

/// Decode an image format supported by the image crate.
fn decode_standard_image(data: &[u8]) -> Result<DecodedImage> {
    let img = image::load_from_memory(data).context("Failed to decode standard image")?;
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

    #[test]
    fn normalizes_firmware_png_palette() {
        let mut encoded = b"\x89PNGGAME".to_vec();
        encoded.extend_from_slice(&6u32.to_be_bytes());
        encoded.extend_from_slice(b"PLTE");
        encoded.extend_from_slice(&0xf800u16.to_be_bytes());
        encoded.extend_from_slice(&0x07e0u16.to_be_bytes());
        encoded.extend_from_slice(&[0; 4]);
        encoded.extend_from_slice(&0u32.to_be_bytes());
        encoded.extend_from_slice(b"IEND");
        encoded.extend_from_slice(&[0xae, 0x42, 0x60, 0x82]);

        let normalized = normalize_firmware_png(&encoded).unwrap();

        assert!(normalized.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(&normalized[16..22], &[255, 0, 0, 0, 255, 0]);
        assert!(normalized.ends_with(&[0xae, 0x42, 0x60, 0x82]));
    }
}
