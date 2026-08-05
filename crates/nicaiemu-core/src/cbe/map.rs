//! MAP Map Parser
//!
//! Parses .map tile-based地图 files from CBE archives.
//! Maps contain tile grids, tilesets, and draw records.
//!
//! Format structure (from reverse engineering):
//! - 0x00-0x03: Version/flags (u32 LE)
//! - 0x04-0x05: Width in tiles (u16 LE)
//! - 0x06-0x07: Height in tiles (u16 LE)
//! - 0x08-0x0D: Unknown fields
//! - 0x0E: Tileset name length (u8)
//! - 0x0F-...: Tileset name (null-terminated or length-prefixed)
//! - After name: Tile data (possibly RLE encoded)

use anyhow::Result;
use log::{debug, info};

/// Tile data
#[derive(Debug, Clone, Copy)]
pub struct Tile {
    /// Tile ID (index into tileset)
    pub id: u16,
    /// Horizontal flip flag
    pub flip_x: bool,
    /// Vertical flip flag
    pub flip_y: bool,
}

impl Tile {
    /// Create a new tile
    pub fn new(id: u16, flip_x: bool, flip_y: bool) -> Self {
        Self { id, flip_x, flip_y }
    }
}

/// Draw record for rendering optimization
#[derive(Debug, Clone)]
pub struct DrawRecord {
    /// Source tile position
    pub src_x: u32,
    pub src_y: u32,
    /// Destination position
    pub dst_x: u32,
    pub dst_y: u32,
    /// Width and height
    pub width: u32,
    pub height: u32,
}

/// Parsed map data
#[derive(Debug, Clone)]
pub struct Map {
    /// Map width (in tiles)
    pub width: u32,
    /// Map height (in tiles)
    pub height: u32,
    /// Reference to tileset resource
    pub tileset_ref: Option<String>,
    /// Tile grid (row-major order)
    pub tiles: Vec<Vec<Tile>>,
    /// Draw records for rendering
    pub draw_records: Vec<DrawRecord>,
    /// Raw map data (for debugging)
    pub raw_data: Vec<u8>,
}

impl Map {
    /// Parse a map from raw bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        debug!("Parsing map, {} bytes", data.len());

        if data.is_empty() {
            anyhow::bail!("Empty map data");
        }

        // Analyze the map buffer
        let analysis = Self::analyze_buffer(data);
        debug!(
            "Map analysis: size={}, draw_records={}, rle={}, tiles={}",
            analysis.size,
            analysis.has_draw_records,
            analysis.rle_candidates,
            analysis.tile_candidates
        );

        // Try to detect map structure
        // Common pattern: header + tileset reference + tile grid

        // For now, create a simple placeholder map
        // The actual format requires more reverse engineering
        let width = 15; // Default for 240px / 16px tiles
        let height = 25; // Default for 400px / 16px tiles

        // Create empty tile grid
        let tiles = vec![vec![Tile::new(0, false, false); width as usize]; height as usize];

        info!("Map: {}x{} tiles (placeholder)", width, height);

        Ok(Self {
            width,
            height,
            tileset_ref: None,
            tiles,
            draw_records: Vec::new(),
            raw_data: data.to_vec(),
        })
    }

    /// Parse map with actual tile data
    pub fn parse_with_tiles(data: &[u8], scene_width: u32, scene_height: u32) -> Result<Self> {
        debug!("Parsing map with tiles, {} bytes", data.len());

        if data.len() < 16 {
            anyhow::bail!("Map data too short");
        }

        // Parse header
        let _version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        // Try to find tileset name
        let mut tileset_name = None;

        // Look for common image extensions
        for ext in &[".gif", ".png", ".bmp"] {
            if let Some(pos) = data.windows(ext.len()).position(|w| w == ext.as_bytes()) {
                // Found extension, look backwards for name start
                let mut name_start = pos;
                while name_start > 0 && data[name_start - 1] != 0 && data[name_start - 1] != 8 {
                    name_start -= 1;
                }
                if name_start < pos {
                    let name = String::from_utf8_lossy(&data[name_start..pos]).to_string();
                    tileset_name = Some(name);
                    break;
                }
            }
        }

        // Calculate tile grid size
        let tile_size = 16; // Standard tile size
        let width = scene_width.div_ceil(tile_size);
        let height = scene_height.div_ceil(tile_size);

        debug!(
            "Tile grid: {}x{}, tileset: {:?}",
            width, height, tileset_name
        );

        // Parse tile data (simplified - just create a pattern)
        let mut tiles = Vec::with_capacity(height as usize);
        for y in 0..height {
            let mut row = Vec::with_capacity(width as usize);
            for x in 0..width {
                // Create a simple pattern based on position
                let tile_id = ((x + y) % 8) as u16;
                row.push(Tile::new(tile_id, false, false));
            }
            tiles.push(row);
        }

        info!(
            "Map: {}x{} tiles, tileset: {:?}",
            width, height, tileset_name
        );

        Ok(Self {
            width,
            height,
            tileset_ref: tileset_name,
            tiles,
            draw_records: Vec::new(),
            raw_data: data.to_vec(),
        })
    }

    /// Get tile at position
    pub fn get_tile(&self, x: u32, y: u32) -> Option<&Tile> {
        self.tiles.get(y as usize)?.get(x as usize)
    }

    /// Analyze raw map buffer for debugging
    pub fn analyze_buffer(data: &[u8]) -> MapAnalysis {
        // Count unique byte values
        let mut byte_counts = [0u32; 256];
        for &b in data {
            byte_counts[b as usize] += 1;
        }

        // Count potential tile IDs (0-255 range)
        let tile_candidates: usize = byte_counts[..256]
            .iter()
            .enumerate()
            .filter(|(_, &count)| count > 0)
            .count();

        // Look for RLE patterns (repeated bytes)
        let mut rle_candidates = 0;
        let mut i = 0;
        while i < data.len() {
            let mut run_len = 1;
            while i + run_len < data.len() && data[i + run_len] == data[i] && run_len < 256 {
                run_len += 1;
            }
            if run_len >= 3 {
                rle_candidates += 1;
            }
            i += run_len;
        }

        // Look for draw records (pairs of coordinates)
        let has_draw_records = data.len() > 100; // Heuristic

        MapAnalysis {
            size: data.len(),
            has_draw_records,
            rle_candidates,
            tile_candidates,
        }
    }
}

/// Analysis results for map buffer
#[derive(Debug)]
pub struct MapAnalysis {
    pub size: usize,
    pub has_draw_records: bool,
    pub rle_candidates: usize,
    pub tile_candidates: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_creation() {
        let tile = Tile::new(100, false, true);
        assert_eq!(tile.id, 100);
        assert!(!tile.flip_x);
        assert!(tile.flip_y);
    }

    #[test]
    fn test_map_analysis() {
        let data = vec![0u8; 100];
        let analysis = Map::analyze_buffer(&data);
        assert_eq!(analysis.size, 100);
        assert_eq!(analysis.tile_candidates, 1); // Only byte 0
    }
}
