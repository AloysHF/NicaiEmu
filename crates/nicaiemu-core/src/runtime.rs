//! Runtime State Management
//!
//! Manages the emulator runtime state including scenes, entities, and scripts.

use anyhow::{Context, Result};
use log::{debug, info};

use crate::cbe::CbeArchive;
use crate::cbe::sce::Scene;

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

    /// Update runtime state
    pub fn update(&mut self, dt: f32) {
        // TODO: Implement entity updates, animation, script execution
        debug!("Runtime update: dt={:.3}s", dt);
    }

    /// Render the current frame
    pub fn render(&mut self) -> &FrameBuffer {
        // Clear frame buffer
        self.frame_buffer.clear();

        // TODO: Implement actual rendering
        // For now, just fill with a solid color
        for y in 0..self.frame_buffer.height {
            for x in 0..self.frame_buffer.width {
                // Create a simple gradient
                let r = (x * 255 / self.frame_buffer.width) as u8;
                let g = (y * 255 / self.frame_buffer.height) as u8;
                let b = 128;
                self.frame_buffer.set_pixel(x, y, r, g, b, 255);
            }
        }

        &self.frame_buffer
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
}
