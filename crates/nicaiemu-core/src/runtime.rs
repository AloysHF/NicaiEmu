//! Scene-level runtime state management (experimental).
//!
//! This module is not wired to any frontend: the emulator executes guest code
//! through [`crate::machine::NicaiMachine`]. It is kept crate-internal as a
//! possible foundation for future scene-level HLE tooling.

// Kept for future scene-level HLE tooling; nothing calls it yet.
#![allow(dead_code)]

use anyhow::{Context, Result};
use log::{debug, info};

use crate::cbe::map::Map;
use crate::cbe::sce::Scene;
use crate::cbe::CbeArchive;
use crate::image_decoder::{self, DecodedImage};

/// Entity state in the runtime
#[derive(Debug, Clone)]
pub struct EntityState {
    /// Entity ID
    pub id: u32,
    /// Position in pixels (x, y)
    pub position: (f32, f32),
    /// Current sprite frame
    pub current_frame: u32,
    /// Animation state
    pub animation: Option<String>,
    /// Reference to actor resource
    pub actor_ref: Option<String>,
    /// Cached sprite image
    pub sprite: Option<DecodedImage>,
}

/// Script execution state
#[derive(Debug, Clone)]
pub struct ScriptState {
    /// Script name
    pub name: String,
    /// Is script currently active
    pub active: bool,
    /// Program counter (for VM execution)
    pub pc: u32,
}

/// Frame buffer for rendering
#[derive(Debug, Clone)]
pub struct FrameBuffer {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Pixel data (RGBA)
    pub data: Vec<u8>,
}

impl FrameBuffer {
    /// Create a new frame buffer
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; (width * height * 4) as usize],
        }
    }

    /// Clear the frame buffer
    pub fn clear(&mut self) {
        self.data.fill(0);
    }

    /// Set a pixel
    pub fn set_pixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
        if x < self.width && y < self.height {
            let idx = ((y * self.width + x) * 4) as usize;
            if idx + 4 <= self.data.len() {
                self.data[idx] = r;
                self.data[idx + 1] = g;
                self.data[idx + 2] = b;
                self.data[idx + 3] = a;
            }
        }
    }

    /// Get pixel color at position
    pub fn get_pixel(&self, x: u32, y: u32) -> (u8, u8, u8, u8) {
        if x < self.width && y < self.height {
            let idx = ((y * self.width + x) * 4) as usize;
            if idx + 4 <= self.data.len() {
                return (
                    self.data[idx],
                    self.data[idx + 1],
                    self.data[idx + 2],
                    self.data[idx + 3],
                );
            }
        }
        (0, 0, 0, 0)
    }

    /// Blit an image onto the frame buffer at the specified position
    pub fn blit(&mut self, x: i32, y: i32, image: &DecodedImage) {
        for sy in 0..image.height as i32 {
            for sx in 0..image.width as i32 {
                let dx = x + sx;
                let dy = y + sy;
                if dx >= 0 && dy >= 0 && (dx as u32) < self.width && (dy as u32) < self.height {
                    let src_idx = ((sy as u32 * image.width + sx as u32) * 4) as usize;
                    let dst_idx = ((dy as u32 * self.width + dx as u32) * 4) as usize;
                    if src_idx + 4 <= image.data.len() && dst_idx + 4 <= self.data.len() {
                        let a = image.data[src_idx + 3];
                        if a > 0 {
                            self.data[dst_idx] = image.data[src_idx];
                            self.data[dst_idx + 1] = image.data[src_idx + 1];
                            self.data[dst_idx + 2] = image.data[src_idx + 2];
                            self.data[dst_idx + 3] = a;
                        }
                    }
                }
            }
        }
    }

    /// Blit with scaling
    pub fn blit_scaled(
        &mut self,
        x: i32,
        y: i32,
        scale_x: f32,
        scale_y: f32,
        image: &DecodedImage,
    ) {
        let dst_width = (image.width as f32 * scale_x) as i32;
        let dst_height = (image.height as f32 * scale_y) as i32;

        for dy in 0..dst_height {
            for dx in 0..dst_width {
                let src_x = (dx as f32 / scale_x) as u32;
                let src_y = (dy as f32 / scale_y) as u32;

                if src_x < image.width && src_y < image.height {
                    let dst_px = x + dx;
                    let dst_py = y + dy;

                    if dst_px >= 0
                        && dst_py >= 0
                        && (dst_px as u32) < self.width
                        && (dst_py as u32) < self.height
                    {
                        let src_idx = ((src_y * image.width + src_x) * 4) as usize;
                        let dst_idx = ((dst_py as u32 * self.width + dst_px as u32) * 4) as usize;

                        if src_idx + 4 <= image.data.len() && dst_idx + 4 <= self.data.len() {
                            let a = image.data[src_idx + 3];
                            if a > 0 {
                                self.data[dst_idx] = image.data[src_idx];
                                self.data[dst_idx + 1] = image.data[src_idx + 1];
                                self.data[dst_idx + 2] = image.data[src_idx + 2];
                                self.data[dst_idx + 3] = a;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Main runtime state
#[derive(Debug)]
pub struct NicaiRuntime {
    /// Loaded CBE archive
    archive: CbeArchive,
    /// Current scene
    current_scene: Option<Scene>,
    /// Current map
    current_map: Option<Map>,
    /// Tileset image for the current map
    tileset_image: Option<DecodedImage>,
    /// Entity states
    entities: Vec<EntityState>,
    /// Script states
    scripts: Vec<ScriptState>,
    /// Frame buffer for rendering
    frame_buffer: FrameBuffer,
    /// Cached decoded images
    image_cache: std::collections::HashMap<String, DecodedImage>,
    /// Camera position (for scrolling)
    camera: (f32, f32),
}

impl NicaiRuntime {
    /// Create a new runtime with a loaded archive
    pub fn new(archive: CbeArchive) -> Self {
        let summary = archive.summary();
        info!("Creating runtime for: {}", summary.path.display());
        info!("  Resources: {}", summary.resource_count);

        // Default to WQVGA resolution (Nicai phone standard)
        let frame_buffer = FrameBuffer::new(240, 400);

        Self {
            archive,
            current_scene: None,
            current_map: None,
            tileset_image: None,
            entities: Vec::new(),
            scripts: Vec::new(),
            frame_buffer,
            image_cache: std::collections::HashMap::new(),
            camera: (0.0, 0.0),
        }
    }

    /// Load a scene by name
    pub fn load_scene(&mut self, name: &str) -> Result<()> {
        info!("Loading scene: {}", name);

        // Find the scene resource
        let resource = self
            .archive
            .find_resource(name)
            .with_context(|| format!("Scene '{}' not found in archive", name))?;

        // Read scene data
        let data = self.archive.read_resource_bytes(resource)?;

        // Parse the scene
        let scene =
            Scene::parse(data).with_context(|| format!("Failed to parse scene '{}'", name))?;

        info!(
            "Scene loaded: {}x{} with {} maps",
            scene.width(),
            scene.height(),
            scene.maps().len()
        );

        // Try to load the first map if available
        if let Some(map_ref) = scene.maps().first() {
            self.load_map(&map_ref.name)?;
        }

        self.current_scene = Some(scene);
        self.entities.clear();
        self.scripts.clear();
        self.camera = (0.0, 0.0);

        Ok(())
    }

    /// Load a map by name
    pub fn load_map(&mut self, name: &str) -> Result<()> {
        info!("Loading map: {}", name);

        // Find the map resource
        let resource = self
            .archive
            .find_resource(name)
            .with_context(|| format!("Map '{}' not found in archive", name))?;

        // Read map data
        let data = self.archive.read_resource_bytes(resource)?;

        // Get scene dimensions for tile grid calculation
        let (scene_width, scene_height) = if let Some(scene) = &self.current_scene {
            (scene.width(), scene.height())
        } else {
            (240, 400) // Default WQVGA
        };

        // Parse the map with scene dimensions
        let map = Map::parse_with_tiles(data, scene_width, scene_height)
            .with_context(|| format!("Failed to parse map '{}'", name))?;

        info!("Map loaded: {}x{} tiles", map.width, map.height);

        // Try to load tileset image from map reference
        if let Some(tileset_name) = &map.tileset_ref {
            if let Some(image) = self.get_image(tileset_name) {
                info!("Loaded tileset: {}", tileset_name);
                self.tileset_image = Some(image.clone());
            }
        }

        // If no tileset from map, try common names
        if self.tileset_image.is_none() {
            let tileset_names = [
                "tileset.gif",
                "tiles.gif",
                "ground.gif",
                "bg.gif",
                "map4.gif",
            ];
            for tileset_name in &tileset_names {
                if let Some(image) = self.get_image(tileset_name) {
                    info!("Loaded tileset: {}", tileset_name);
                    self.tileset_image = Some(image.clone());
                    break;
                }
            }
        }

        self.current_map = Some(map);
        Ok(())
    }

    /// Load the first available scene
    pub fn load_first_scene(&mut self) -> Result<()> {
        let scenes = self.archive.scenes();
        if let Some(scene) = scenes.first() {
            let name = scene.name.clone();
            self.load_scene(&name)
        } else {
            anyhow::bail!("No scenes found in archive")
        }
    }

    /// Decode a GIF image from CBE resource bytes
    fn decode_gif(&self, data: &[u8]) -> Result<DecodedImage> {
        image_decoder::decode_image(data)
    }

    /// Get or decode an image by resource name
    pub fn get_image(&mut self, name: &str) -> Option<&DecodedImage> {
        if !self.image_cache.contains_key(name) {
            let resource = self.archive.find_resource(name)?;
            let data = self.archive.read_resource_bytes(resource).ok()?;
            let image = self.decode_gif(data).ok()?;
            self.image_cache.insert(name.to_string(), image);
        }
        self.image_cache.get(name)
    }

    /// Update runtime state
    pub fn update(&mut self, dt: f32) {
        // TODO: Implement entity updates, animation, script execution
        debug!("Runtime update: dt={:.3}s", dt);
    }

    /// Render the current frame
    pub fn render(&mut self) -> &FrameBuffer {
        // Clear frame buffer with black background
        self.frame_buffer.clear();

        // Fill with dark background
        for y in 0..self.frame_buffer.height {
            for x in 0..self.frame_buffer.width {
                self.frame_buffer.set_pixel(x, y, 20, 20, 30, 255);
            }
        }

        // Render map tiles if available
        // Clone map data to avoid borrow checker issues
        let map_data = self.current_map.clone();
        if let Some(map) = &map_data {
            self.render_map(map);
        }

        // Render entities
        for entity in &self.entities {
            if let Some(sprite) = &entity.sprite {
                self.frame_buffer
                    .blit(entity.position.0 as i32, entity.position.1 as i32, sprite);
            }
        }

        &self.frame_buffer
    }

    /// Render map tiles
    fn render_map(&mut self, map: &Map) {
        let tile_size: i32 = 16; // Standard tile size for Nicai games
        let camera_x = self.camera.0 as i32;
        let camera_y = self.camera.1 as i32;

        // Calculate visible tile range
        let start_tile_x = (camera_x / tile_size).max(0) as u32;
        let start_tile_y = (camera_y / tile_size).max(0) as u32;
        let end_tile_x = ((camera_x + self.frame_buffer.width as i32) / tile_size + 1)
            .min(map.width as i32) as u32;
        let end_tile_y = ((camera_y + self.frame_buffer.height as i32) / tile_size + 1)
            .min(map.height as i32) as u32;

        // Render each visible tile
        for ty in start_tile_y..end_tile_y {
            for tx in start_tile_x..end_tile_x {
                if let Some(tile) = map.get_tile(tx, ty) {
                    let screen_x = (tx as i32 * tile_size) - camera_x;
                    let screen_y = (ty as i32 * tile_size) - camera_y;

                    // Render tile based on tile ID
                    self.render_tile(screen_x, screen_y, tile_size as u32, tile.id);
                }
            }
        }
    }

    /// Render a single tile
    fn render_tile(&mut self, x: i32, y: i32, size: u32, tile_id: u16) {
        // If we have a tileset image, use it
        if let Some(tileset) = self.tileset_image.clone() {
            // Calculate tile position in tileset
            // Assuming tileset is arranged in a grid
            let tiles_per_row = tileset.width / size;
            let tile_x = (tile_id as u32 % tiles_per_row) * size;
            let tile_y = (tile_id as u32 / tiles_per_row) * size;

            // Blit tile from tileset
            for dy in 0..size {
                for dx in 0..size {
                    let src_x = tile_x + dx;
                    let src_y = tile_y + dy;

                    if src_x < tileset.width && src_y < tileset.height {
                        let px = x + dx as i32;
                        let py = y + dy as i32;

                        if px >= 0
                            && py >= 0
                            && (px as u32) < self.frame_buffer.width
                            && (py as u32) < self.frame_buffer.height
                        {
                            let src_idx = ((src_y * tileset.width + src_x) * 4) as usize;
                            let dst_idx =
                                ((py as u32 * self.frame_buffer.width + px as u32) * 4) as usize;

                            if src_idx + 4 <= tileset.data.len()
                                && dst_idx + 4 <= self.frame_buffer.data.len()
                            {
                                let a = tileset.data[src_idx + 3];
                                if a > 0 {
                                    self.frame_buffer.data[dst_idx] = tileset.data[src_idx];
                                    self.frame_buffer.data[dst_idx + 1] = tileset.data[src_idx + 1];
                                    self.frame_buffer.data[dst_idx + 2] = tileset.data[src_idx + 2];
                                    self.frame_buffer.data[dst_idx + 3] = a;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // No tileset, render a simple colored rectangle
            let color = match tile_id % 8 {
                0 => (100, 120, 100), // Grass
                1 => (139, 119, 101), // Dirt
                2 => (100, 100, 120), // Stone
                3 => (80, 100, 80),   // Dark grass
                4 => (120, 100, 80),  // Sand
                5 => (60, 80, 100),   // Water
                6 => (100, 80, 60),   // Wood
                _ => (80, 80, 80),    // Default
            };

            for dy in 0..size {
                for dx in 0..size {
                    let px = x + dx as i32;
                    let py = y + dy as i32;
                    if px >= 0
                        && py >= 0
                        && (px as u32) < self.frame_buffer.width
                        && (py as u32) < self.frame_buffer.height
                    {
                        self.frame_buffer
                            .set_pixel(px as u32, py as u32, color.0, color.1, color.2, 255);
                    }
                }
            }
        }
    }

    /// Try to render an image from the archive at the specified position
    pub fn try_render_image(&mut self, name: &str, x: i32, y: i32) -> bool {
        if let Some(image) = self.get_image(name) {
            let img = image.clone();
            self.frame_buffer.blit(x, y, &img);
            true
        } else {
            false
        }
    }

    /// Set camera position
    pub fn set_camera(&mut self, x: f32, y: f32) {
        self.camera = (x, y);
    }

    /// Get camera position
    pub fn camera(&self) -> (f32, f32) {
        self.camera
    }

    /// Get reference to the archive
    pub fn archive(&self) -> &CbeArchive {
        &self.archive
    }

    /// Get current scene
    pub fn current_scene(&self) -> Option<&Scene> {
        self.current_scene.as_ref()
    }

    /// Get current map
    pub fn current_map(&self) -> Option<&Map> {
        self.current_map.as_ref()
    }

    /// Get frame buffer
    pub fn frame_buffer(&self) -> &FrameBuffer {
        &self.frame_buffer
    }
}

/// Convert HSV to RGB
#[cfg(test)]
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_buffer_creation() {
        let fb = FrameBuffer::new(240, 400);
        assert_eq!(fb.width, 240);
        assert_eq!(fb.height, 400);
        assert_eq!(fb.data.len(), 240 * 400 * 4);
    }

    #[test]
    fn test_frame_buffer_set_pixel() {
        let mut fb = FrameBuffer::new(10, 10);
        fb.set_pixel(5, 5, 255, 128, 64, 255);

        let idx = (5 * 10 + 5) * 4;
        assert_eq!(fb.data[idx], 255); // R
        assert_eq!(fb.data[idx + 1], 128); // G
        assert_eq!(fb.data[idx + 2], 64); // B
        assert_eq!(fb.data[idx + 3], 255); // A
    }

    #[test]
    fn test_frame_buffer_blit() {
        let mut fb = FrameBuffer::new(10, 10);
        // Create a 3x3 red image (RGBA format: 4 bytes per pixel)
        let mut data = Vec::new();
        for _ in 0..9 {
            data.extend_from_slice(&[255, 0, 0, 255]); // Red pixel
        }
        let image = DecodedImage {
            width: 3,
            height: 3,
            data,
        };

        fb.blit(1, 1, &image);

        // Check pixel at (1, 1) is red
        let (r, g, b, a) = fb.get_pixel(1, 1);
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
        assert_eq!(a, 255);
    }

    #[test]
    fn test_hsv_to_rgb() {
        // Red
        let (r, g, b) = hsv_to_rgb(0.0, 1.0, 1.0);
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);

        // Green
        let (r, g, b) = hsv_to_rgb(120.0, 1.0, 1.0);
        assert_eq!(r, 0);
        assert_eq!(g, 255);
        assert_eq!(b, 0);

        // Blue
        let (r, g, b) = hsv_to_rgb(240.0, 1.0, 1.0);
        assert_eq!(r, 0);
        assert_eq!(g, 0);
        assert_eq!(b, 255);
    }
}
