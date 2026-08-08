//! CBE Resource Types
//!
//! Defines resource types and entries found in CBE archives.

use std::fmt;
use thiserror::Error;

/// Errors during resource parsing
#[derive(Error, Debug)]
pub enum ResourceError {
    #[error("Invalid resource signature")]
    InvalidSignature,
    #[error("Unsupported resource type: {0}")]
    UnsupportedType(String),
    #[error("Resource not found: {0}")]
    NotFound(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Resource types found in CBE archives
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    /// Scene file (.sce) - contains scene graph and object placement
    Scene,
    /// Map file (.map) - tile-based地图数据
    Map,
    /// Actor file (.actor) - sprite/animation data
    Actor,
    /// Script file (.xse) - XSE virtual machine scripts
    Script,
    /// GIF image (.gif)
    Image,
    /// Audio file
    Audio,
    /// Unknown resource type
    Unknown,
}

impl ResourceType {
    /// Determine resource type from file extension
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "sce" => Self::Scene,
            "map" => Self::Map,
            "actor" => Self::Actor,
            "xse" => Self::Script,
            "gif" => Self::Image,
            "mp3" | "wav" | "amr" | "mid" | "midi" | "ogg" => Self::Audio,
            _ => Self::Unknown,
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Scene => "Scene",
            Self::Map => "Map",
            Self::Actor => "Actor",
            Self::Script => "Script",
            Self::Image => "Image",
            Self::Audio => "Audio",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A single resource entry in a CBE archive
#[derive(Debug, Clone)]
pub struct ResourceEntry {
    /// Resource name (e.g., "guangmingshendian.sce")
    pub name: String,
    /// Section index containing this resource
    pub section_index: usize,
    /// Offset within the section (in bytes)
    pub offset: u64,
    /// Size of the resource data (in bytes)
    pub size: u64,
    /// Resource type (derived from extension)
    pub resource_type: ResourceType,
}

impl ResourceEntry {
    /// Create a new resource entry
    pub fn new(name: String, section_index: usize, offset: u64, size: u64) -> Self {
        let resource_type = ResourceType::from_extension(name.rsplit('.').next().unwrap_or(""));
        Self {
            name,
            section_index,
            offset,
            size,
            resource_type,
        }
    }

    /// Get file extension
    pub fn extension(&self) -> Option<&str> {
        self.name.rsplit('.').next()
    }

    /// Check if this is a scene resource
    pub fn is_scene(&self) -> bool {
        self.resource_type == ResourceType::Scene
    }

    /// Check if this is a script resource
    pub fn is_script(&self) -> bool {
        self.resource_type == ResourceType::Script
    }

    /// Check if this is an audio resource
    pub fn is_audio(&self) -> bool {
        self.resource_type == ResourceType::Audio
    }
}

impl fmt::Display for ResourceEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} (section={}, offset=0x{:X}, size={})",
            self.resource_type, self.name, self.section_index, self.offset, self.size
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_audio_extensions() {
        for extension in ["mp3", "wav", "amr", "mid", "midi", "ogg"] {
            assert_eq!(ResourceType::from_extension(extension), ResourceType::Audio);
        }
    }

    #[test]
    fn audio_entry_predicate() {
        let entry = ResourceEntry::new("song.mp3".to_string(), 0, 0, 10);
        assert!(entry.is_audio());
        assert_eq!(entry.resource_type, ResourceType::Audio);
    }
}
