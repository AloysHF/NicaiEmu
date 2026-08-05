//! SCE Scene Parser
//!
//! Parses .sce scene files from CBE archives.
//! Scenes contain scene graph, object placement, and resource references.
//!
//! Format structure (from reverse engineering):
//! - 0x0000-0x0009: Unknown header (10 bytes)
//! - 0x000A: SCE2 magic (4 bytes: 0x53, 0x43, 0x45, 0x32)
//! - 0x000E: width (u16 LE)
//! - 0x0010: height (u16 LE)
//! - 0x0012: map_count (u16 LE)
//! - 0x0014: map_table (map_count * 8 bytes)
//! - After map_table: entity placements and script references

use anyhow::Result;
use log::{debug, info, warn};

/// Map reference in a scene
#[derive(Debug, Clone)]
pub struct MapRef {
    /// Map resource name (length-prefixed)
    pub name: String,
    /// Unknown field 0
    pub field0: u8,
    /// Unknown field 1
    pub field1: u8,
    /// Unknown field 2
    pub field2: u8,
    /// Unknown field 3
    pub field3: u8,
}

/// Entity placement in a scene
#[derive(Debug, Clone)]
pub struct Entity {
    /// Position in the scene (x, y)
    pub position: (f32, f32),
    /// Reference to actor resource
    pub actor_ref: Option<String>,
    /// Entity ID or index
    pub id: u32,
    /// Entity type
    pub entity_type: u32,
}

/// Script reference in a scene
#[derive(Debug, Clone)]
pub struct ScriptRef {
    /// Reference to script resource
    pub script_name: String,
    /// Script type or context
    pub script_type: String,
}

/// Parsed scene data
#[derive(Debug, Clone)]
pub struct Scene {
    /// Scene width
    pub width: u32,
    /// Scene height
    pub height: u32,
    /// References to map resources
    pub maps: Vec<MapRef>,
    /// Entities placed in the scene
    pub entities: Vec<Entity>,
    /// Scripts linked to the scene
    pub scripts: Vec<ScriptRef>,
    /// Raw scene data (for debugging)
    pub raw_data: Vec<u8>,
}

impl Scene {
    /// Parse a scene from raw bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        debug!("Parsing scene, {} bytes", data.len());

        if data.len() < 20 {
            anyhow::bail!("Scene data too short: {} bytes", data.len());
        }

        // Check for SCE2 signature at offset 0x0A
        if data.len() >= 14 && &data[0x0A..0x0E] == b"SCE2" {
            debug!("Found SCE2 signature at offset 0x0A");
        } else {
            warn!("No SCE2 signature found, trying to parse anyway");
        }

        // Read width and height (u16 LE at offsets 0x0E and 0x10)
        let width = if data.len() >= 0x10 {
            u16::from_le_bytes([data[0x0E], data[0x0F]]) as u32
        } else {
            240 // Default WQVGA width
        };

        let height = if data.len() >= 0x12 {
            u16::from_le_bytes([data[0x10], data[0x11]]) as u32
        } else {
            400 // Default WQVGA height
        };

        // Read map count (u16 LE at offset 0x12)
        let map_count = if data.len() >= 0x14 {
            u16::from_le_bytes([data[0x12], data[0x13]]) as usize
        } else {
            0
        };

        debug!("Scene: {}x{}, {} maps", width, height, map_count);

        // Parse map table (starts at offset 0x14)
        // Format: 1 byte flag + fixed 4-byte name + 3 bytes + null terminator
        let mut maps = Vec::new();
        let mut pos = 0x14;

        for i in 0..map_count {
            if pos + 9 > data.len() {
                warn!("Map table truncated at offset 0x{:X}", pos);
                break;
            }

            // Read flag byte
            let flag = data[pos];
            pos += 1;

            // Read 4-byte name (not null-terminated)
            let name_bytes = &data[pos..pos + 4];
            let name = String::from_utf8_lossy(name_bytes)
                .trim_end_matches('\0')
                .to_string();
            pos += 4;

            // Read extension byte (should be '.')
            let ext_byte = data[pos];
            pos += 1;

            // Read 3 unknown bytes
            let _field0 = data[pos];
            let field1 = data[pos + 1];
            let field2 = data[pos + 2];
            pos += 3;

            // Skip null terminator
            if pos < data.len() && data[pos] == 0 {
                pos += 1;
            }

            // Construct full map name with .map extension
            let full_name = if ext_byte == b'.' {
                format!("{}.map", name)
            } else {
                name.clone()
            };

            debug!(
                "Map {}: {} (flag: {}, ext: {:02x})",
                i, full_name, flag, ext_byte
            );

            maps.push(MapRef {
                name: full_name,
                field0: flag,
                field1,
                field2,
                field3: 0,
            });
        }

        // Parse entity placements
        // The exact format needs more reverse engineering
        // For now, try to find any resource references in the remaining data
        let mut entities = Vec::new();
        let mut scripts = Vec::new();

        // Simple heuristic: look for length-prefixed strings that look like resource names
        while pos < data.len() {
            let len = data[pos] as usize;
            if len == 0 || len > 64 || pos + 1 + len > data.len() {
                pos += 1;
                continue;
            }

            let name_bytes = &data[pos + 1..pos + 1 + len];
            let name = String::from_utf8_lossy(name_bytes).to_string();

            // Check if it looks like a resource name
            if name.ends_with(".actor") || name.ends_with(".xse") || name.ends_with(".map") {
                debug!("Found resource reference: {}", name);

                if name.ends_with(".actor") {
                    entities.push(Entity {
                        position: (0.0, 0.0),
                        actor_ref: Some(name),
                        id: entities.len() as u32,
                        entity_type: 0,
                    });
                } else if name.ends_with(".xse") {
                    scripts.push(ScriptRef {
                        script_name: name,
                        script_type: "main".to_string(),
                    });
                }
            }

            pos += 1 + len;
        }

        info!(
            "Parsed scene: {}x{}, {} maps, {} entities, {} scripts",
            width,
            height,
            maps.len(),
            entities.len(),
            scripts.len()
        );

        Ok(Self {
            width,
            height,
            maps,
            entities,
            scripts,
            raw_data: data.to_vec(),
        })
    }

    /// Get scene width
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get scene height
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get entity count
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Get map references
    pub fn maps(&self) -> &[MapRef] {
        &self.maps
    }

    /// Get script references
    pub fn scripts(&self) -> &[ScriptRef] {
        &self.scripts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_creation() {
        let scene = Scene {
            width: 240,
            height: 400,
            maps: Vec::new(),
            entities: Vec::new(),
            scripts: Vec::new(),
            raw_data: Vec::new(),
        };

        assert_eq!(scene.width(), 240);
        assert_eq!(scene.height(), 400);
    }

    #[test]
    fn test_scene_parse_minimal() {
        // Minimal scene data with SCE2 signature
        let mut data = vec![0u8; 32];
        // SCE2 signature at offset 0x0A
        data[0x0A] = 0x53; // 'S'
        data[0x0B] = 0x43; // 'C'
        data[0x0C] = 0x45; // 'E'
        data[0x0D] = 0x32; // '2'
                           // Width = 240 (0x00F0)
        data[0x0E] = 0xF0;
        data[0x0F] = 0x00;
        // Height = 400 (0x0190)
        data[0x10] = 0x90;
        data[0x11] = 0x01;
        // Map count = 0
        data[0x12] = 0x00;
        data[0x13] = 0x00;

        let scene = Scene::parse(&data).unwrap();
        assert_eq!(scene.width(), 240);
        assert_eq!(scene.height(), 400);
        assert_eq!(scene.maps().len(), 0);
    }
}
