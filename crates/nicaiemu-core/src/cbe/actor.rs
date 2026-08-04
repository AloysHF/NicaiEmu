//! Actor Parser
//!
//! Parses .actor sprite/animation files from CBE archives.
//! Actors contain sprite sheets, animation data, and metadata.

use anyhow::Result;
use log::debug;

/// Sprite frame
#[derive(Debug, Clone)]
pub struct SpriteFrame {
    /// Frame index
    pub index: u32,
    /// X offset in sprite sheet
    pub x: u32,
    /// Y offset in sprite sheet
    pub y: u32,
    /// Frame width
    pub width: u32,
    /// Frame height
    pub height: u32,
}

/// Animation sequence
#[derive(Debug, Clone)]
pub struct Animation {
    /// Animation name
    pub name: String,
    /// Frame indices in sequence
    pub frames: Vec<u32>,
    /// Frame duration (in ms)
    pub frame_duration: u32,
    /// Loop flag
    pub looped: bool,
}

/// Parsed actor data
#[derive(Debug, Clone)]
pub struct Actor {
    /// Primary sprite sheet reference
    pub sprite_sheet_ref: Option<String>,
    /// Sprite frames
    pub frames: Vec<SpriteFrame>,
    /// Animations
    pub animations: Vec<Animation>,
    /// Actor metadata
    pub metadata: ActorMetadata,
    /// Raw actor data (for debugging)
    pub raw_data: Vec<u8>,
}

/// Actor metadata
#[derive(Debug, Clone, Default)]
pub struct ActorMetadata {
    /// Actor type or class
    pub actor_type: Option<String>,
    /// Collision box dimensions
    pub collision_width: Option<u32>,
    pub collision_height: Option<u32>,
}

impl Actor {
    /// Parse an actor from raw bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        debug!("Parsing actor, {} bytes", data.len());

        // Check for FF-heavy token pattern (characteristic of actor streams)
        let ff_count = data.iter().filter(|&&b| b == 0xFF).count();
        let ff_ratio = ff_count as f64 / data.len() as f64;
        debug!("FF token ratio: {:.2}%", ff_ratio * 100.0);

        // TODO: Implement actual actor parsing based on reverse engineering
        // This is a placeholder structure

        Ok(Self {
            sprite_sheet_ref: None,
            frames: Vec::new(),
            animations: Vec::new(),
            metadata: ActorMetadata::default(),
            raw_data: data.to_vec(),
        })
    }

    /// Get frame count
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Get animation count
    pub fn animation_count(&self) -> usize {
        self.animations.len()
    }

    /// Find animation by name
    pub fn find_animation(&self, name: &str) -> Option<&Animation> {
        self.animations.iter().find(|a| a.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sprite_frame_creation() {
        let frame = SpriteFrame {
            index: 0,
            x: 0,
            y: 0,
            width: 32,
            height: 32,
        };
        assert_eq!(frame.width, 32);
        assert_eq!(frame.height, 32);
    }
}
