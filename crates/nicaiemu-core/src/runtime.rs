//! Runtime State Management
//!
//! Manages the emulator runtime state including scenes, entities, and scripts.

use anyhow::{Context, Result};
use log::{debug, info};

use crate::cbe::CbeArchive;
use crate::cbe::sce::Scene;
use crate::image_decoder::{self, DecodedImage};

/// Entity state in the runtime
#[derive(Debug, Clone)]
pub struct EntityState {
    /// Entity ID
    pub id: u32,
    /// Position (x, y)
    pub position: (f32, f32),
    /// Current sprite frame
    pub current_frame: u32,
    /// Animation state
    pub animation: Option<String>,
    /// Reference to actor resource
    pub actor_ref: Option<String>,
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
}

/// Main runtime state
#[derive(Debug)]
pub struct NicaiRuntime {
    /// Loaded CBE archive
    archive: CbeArchive,
    /// Current scene
    current_scene: Option<Scene>,
    /// Entity states
    entities: Vec<EntityState>,
    /// Script states
    scripts: Vec<ScriptState>,
    /// Frame buffer for rendering
    frame_buffer: FrameBuffer,
    /// Cached decoded images
    image_cache: std::collections::HashMap<String, DecodedImage>,
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
            entities: Vec::new(),
            scripts: Vec::new(),
            frame_buffer,
            image_cache: std::collections::HashMap::new(),
        }
    }

    /// Load a scene by name
    pub fn load_scene(&mut self, name: &str) -> Result<()> {
        info!("Loading scene: {}", name);

        // Find the scene resource
        let resource = self.archive.find_resource(name)
            .with_context(|| format!("Scene '{}' not found in archive", name))?;

        // Read scene data
        let data = self.archive.read_resource_bytes(resource)?;

        // Parse the scene
        let scene = Scene::parse(data)
            .with_context(|| format!("Failed to parse scene '{}'", name))?;

        info!("Scene loaded: {}x{}", scene.width(), scene.height());

        self.current_scene = Some(scene);
        self.entities.clear();
        self.scripts.clear();

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

        // If we have a scene, try to render its background
        if let Some(_scene) = &self.current_scene {
            // For now, render a simple grid to show the scene is loaded
            for y in (0..self.frame_buffer.height).step_by(32) {
                for x in (0..self.frame_buffer.width).step_by(32) {
                    let color = if (x / 32 + y / 32) % 2 == 0 {
                        (40, 40, 50)
                    } else {
                        (50, 50, 60)
                    };
                    for dy in 0..32.min(self.frame_buffer.height - y) {
                        for dx in 0..32.min(self.frame_buffer.width - x) {
                            self.frame_buffer.set_pixel(
                                x + dx, y + dy,
                                color.0, color.1, color.2, 255,
                            );
                        }
                    }
                }
            }
        }

        &self.frame_buffer
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

    /// Get reference to the archive
    pub fn archive(&self) -> &CbeArchive {
        &self.archive
    }

    /// Get current scene
    pub fn current_scene(&self) -> Option<&Scene> {
        self.current_scene.as_ref()
    }

    /// Get frame buffer
    pub fn frame_buffer(&self) -> &FrameBuffer {
        &self.frame_buffer
    }
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
        assert_eq!(fb.data[idx], 255);     // R
        assert_eq!(fb.data[idx + 1], 128); // G
        assert_eq!(fb.data[idx + 2], 64);  // B
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
}
