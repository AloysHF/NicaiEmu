//! CBE Archive Parser
//!
//! Loads and parses CBE (Cool Bar Engine) game archives.
//! CBE files are container archives containing game resources.
//!
//! Format structure (from reverse engineering):
//! - Signature: 8 bytes (0xFE x 8)
//! - Section header: marker(4) + count(4) + one(4) + firstDataRel(4) + dataLen(4)
//! - Offset table: (count - 1) * 4 bytes (each is end offset of resource i)
//! - Name table: count * (1 + name_len) bytes
//! - Data region: firstDataRel + 0x18 bytes from section start

use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::fs;
use std::path::{Path, PathBuf};

use super::resource::{ResourceEntry, ResourceType};

/// CBE section signature (8 bytes)
const CBE_SECTION_SIGNATURE: [u8; 8] = [0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE];

/// Section header structure after the 8-byte signature
#[derive(Debug, Clone)]
pub struct SectionHeader {
    /// Section index
    pub index: usize,
    /// Offset of this section in the file
    pub file_offset: u64,
    /// Marker value (always 8?)
    pub marker: u32,
    /// Number of resources in this section
    pub resource_count: u32,
    /// Value 1
    pub one: u32,
    /// Relative offset to the data region (from section start + 0x18)
    pub data_rel: u32,
    /// Total length of the data region
    pub data_len: u32,
    /// Offset to the data start in the file
    pub data_start: u64,
}

/// A CBE section containing multiple resources
#[derive(Debug, Clone)]
pub struct CbeSection {
    /// Section header
    pub header: SectionHeader,
    /// Resources in this section
    pub resources: Vec<ResourceEntry>,
}

/// Main CBE archive structure
#[derive(Debug, Clone)]
pub struct CbeArchive {
    /// Path to the CBE file
    path: PathBuf,
    /// Raw file data (loaded into memory)
    data: Vec<u8>,
    /// Sections found in the archive
    sections: Vec<CbeSection>,
    /// Flat list of all resources
    all_resources: Vec<ResourceEntry>,
}

impl CbeArchive {
    /// Load a CBE archive from a file
    pub fn load(path: &Path) -> Result<Self> {
        info!("Loading CBE archive: {}", path.display());

        // Read the entire file into memory
        let data = fs::read(path)
            .with_context(|| format!("Failed to read CBE file: {}", path.display()))?;

        debug!("File size: {} bytes", data.len());

        // Scan for sections
        let sections = Self::scan_sections(&data)?;
        info!("Found {} sections", sections.len());

        // Build flat resource list
        let mut all_resources = Vec::new();
        for section in &sections {
            all_resources.extend(section.resources.clone());
        }
        info!("Total resources: {}", all_resources.len());

        Ok(Self {
            path: path.to_path_buf(),
            data,
            sections,
            all_resources,
        })
    }

    /// Check if a buffer looks like a CBE resource section
    fn looks_like_resource_section(buf: &[u8], off: usize) -> bool {
        if off + 40 > buf.len() {
            return false;
        }

        // Check signature
        if buf[off..off + 8] != CBE_SECTION_SIGNATURE {
            return false;
        }

        let marker = u32::from_le_bytes([buf[off + 8], buf[off + 9], buf[off + 10], buf[off + 11]]);
        let count =
            u32::from_le_bytes([buf[off + 12], buf[off + 13], buf[off + 14], buf[off + 15]]);
        let one = u32::from_le_bytes([buf[off + 16], buf[off + 17], buf[off + 18], buf[off + 19]]);
        let first_data_rel =
            u32::from_le_bytes([buf[off + 20], buf[off + 21], buf[off + 22], buf[off + 23]]);
        let data_len =
            u32::from_le_bytes([buf[off + 24], buf[off + 25], buf[off + 26], buf[off + 27]]);

        // Validate header values
        if marker != 8 || !(1..=10000).contains(&count) || one != 1 {
            return false;
        }
        if data_len < 1 || first_data_rel as usize + data_len as usize + 0x18 > buf.len() - off {
            return false;
        }

        // Check that we can read at least some valid names
        let names_start = off + 36 + (count as usize - 1) * 4;
        if names_start >= buf.len() {
            return false;
        }

        let mut pos = names_start;
        let mut checked = 0;
        while checked < count.min(16) && pos < buf.len() {
            let len = buf[pos] as usize;
            if !(1..=96).contains(&len) || pos + 1 + len > buf.len() {
                return false;
            }
            let name = &buf[pos + 1..pos + 1 + len];
            // Check if name looks like ASCII resource name
            let valid = name.iter().all(|&c| {
                (0x30..=0x39).contains(&c)
                    || (0x41..=0x5a).contains(&c)
                    || (0x61..=0x7a).contains(&c)
                    || c == 0x2e
                    || c == 0x5f
                    || c == 0x2d
                    || c >= 0x80 // Allow Chinese/other extended chars
            });
            if !valid {
                return false;
            }
            pos += 1 + len;
            checked += 1;
        }

        checked > 0
    }

    fn looks_like_native_resource_section(buf: &[u8], off: usize) -> bool {
        if off + 40 > buf.len() || buf[off..off + 8] != CBE_SECTION_SIGNATURE {
            return false;
        }
        let read = |offset: usize| u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
        let marker = read(off + 8);
        let one = read(off + 12);
        let data_rel = read(off + 16) as usize;
        let data_len = read(off + 20) as usize;
        let count = read(off + 24) as usize;
        if marker != 4 || one != 1 || !(1..=10_000).contains(&count) {
            return false;
        }
        let Some(data_start) = off
            .checked_add(data_rel)
            .and_then(|value| value.checked_add(0x14))
        else {
            return false;
        };
        if data_start
            .checked_add(data_len)
            .is_none_or(|end| end > buf.len())
        {
            return false;
        }
        let Some(mut name_pos) = (off + 32).checked_add((count - 1) * 4) else {
            return false;
        };
        for _ in 0..count.min(16) {
            let Some(&length) = buf.get(name_pos) else {
                return false;
            };
            let length = length as usize;
            if !(1..=96).contains(&length) || name_pos + 1 + length > data_start {
                return false;
            }
            name_pos += 1 + length;
        }
        true
    }

    /// Scan the file for CBE sections
    fn scan_sections(data: &[u8]) -> Result<Vec<CbeSection>> {
        let mut sections = Vec::new();
        let mut section_index = 0;

        for off in 0..data.len().saturating_sub(40) {
            if Self::looks_like_resource_section(data, off) {
                debug!("Found section signature at offset 0x{:X}", off);

                // Parse section header
                if let Some(section) = Self::parse_section(data, off, section_index) {
                    sections.push(section);
                    section_index += 1;
                }
            } else if Self::looks_like_native_resource_section(data, off) {
                debug!("Found native section signature at offset 0x{:X}", off);
                if let Some(section) = Self::parse_native_section(data, off, section_index) {
                    sections.push(section);
                    section_index += 1;
                }
            }
        }

        Ok(sections)
    }

    /// Parse a single CBE section starting at the given offset
    fn parse_section(data: &[u8], start_offset: usize, index: usize) -> Option<CbeSection> {
        // Skip signature (8 bytes)
        let pos = start_offset + 8;

        // Read section header
        let marker = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        let resource_count =
            u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]);
        let one =
            u32::from_le_bytes([data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11]]);
        let data_rel = u32::from_le_bytes([
            data[pos + 12],
            data[pos + 13],
            data[pos + 14],
            data[pos + 15],
        ]);
        let data_len = u32::from_le_bytes([
            data[pos + 16],
            data[pos + 17],
            data[pos + 18],
            data[pos + 19],
        ]);

        debug!(
            "Section {}: marker={}, {} resources, dataRel=0x{:X}, dataLen=0x{:X}",
            index, marker, resource_count, data_rel, data_len
        );

        if resource_count == 0 {
            return None;
        }

        // Read offset table (count - 1 entries, each is the end offset of resource i)
        let mut ends = Vec::with_capacity(resource_count as usize);
        let mut table_pos = start_offset + 36;
        for _ in 0..(resource_count - 1) {
            if table_pos + 4 > data.len() {
                warn!("Offset table truncated at 0x{:X}", table_pos);
                return None;
            }
            let end_offset = u32::from_le_bytes([
                data[table_pos],
                data[table_pos + 1],
                data[table_pos + 2],
                data[table_pos + 3],
            ]);
            ends.push(end_offset);
            table_pos += 4;
        }

        // Read name table
        let names_start = start_offset + 36 + (resource_count as usize - 1) * 4;
        let mut names = Vec::with_capacity(resource_count as usize);
        let mut name_pos = names_start;

        for _ in 0..resource_count {
            if name_pos >= data.len() {
                warn!("Name table truncated at 0x{:X}", name_pos);
                return None;
            }
            let name_len = data[name_pos] as usize;
            if name_len == 0 || name_pos + 1 + name_len > data.len() {
                warn!("Invalid name length at 0x{:X}: {}", name_pos, name_len);
                return None;
            }
            let name =
                String::from_utf8_lossy(&data[name_pos + 1..name_pos + 1 + name_len]).to_string();
            names.push(name);
            name_pos += 1 + name_len;
        }

        // Calculate data start offset
        let data_start = start_offset + data_rel as usize + 0x18;

        // Build resource entries
        let mut resources = Vec::with_capacity(resource_count as usize);
        for i in 0..resource_count as usize {
            let start_rel = if i == 0 { 0 } else { ends[i - 1] as usize };
            let end_rel = if i < ends.len() {
                ends[i] as usize
            } else {
                data_len as usize
            };

            let start = data_start + start_rel;
            let end = data_start + end_rel;
            let size = end.saturating_sub(start);

            let entry = ResourceEntry::new(names[i].clone(), index, start as u64, size as u64);
            resources.push(entry);
        }

        let header = SectionHeader {
            index,
            file_offset: start_offset as u64,
            marker,
            resource_count,
            one,
            data_rel,
            data_len,
            data_start: data_start as u64,
        };

        Some(CbeSection { header, resources })
    }

    fn parse_native_section(data: &[u8], start_offset: usize, index: usize) -> Option<CbeSection> {
        let read = |offset: usize| u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        let marker = read(start_offset + 8);
        let one = read(start_offset + 12);
        let data_rel = read(start_offset + 16);
        let data_len = read(start_offset + 20);
        let resource_count = read(start_offset + 24);
        let count = resource_count as usize;

        let mut ends = Vec::with_capacity(count);
        let mut table_pos = start_offset + 32;
        for _ in 0..count.saturating_sub(1) {
            ends.push(read(table_pos) as usize);
            table_pos += 4;
        }

        let mut names = Vec::with_capacity(count);
        let mut name_pos = table_pos;
        for _ in 0..count {
            let length = *data.get(name_pos)? as usize;
            let name_end = name_pos.checked_add(1 + length)?;
            let name = String::from_utf8_lossy(data.get(name_pos + 1..name_end)?).to_string();
            names.push(name);
            name_pos = name_end;
        }

        let data_start = start_offset
            .checked_add(data_rel as usize)?
            .checked_add(0x14)?;
        let mut resources = Vec::with_capacity(count);
        for (resource_index, name) in names.into_iter().enumerate() {
            let start_rel = if resource_index == 0 {
                0
            } else {
                ends[resource_index - 1]
            };
            let end_rel = ends
                .get(resource_index)
                .copied()
                .unwrap_or(data_len as usize);
            if start_rel > end_rel || data_start.checked_add(end_rel)? > data.len() {
                return None;
            }
            resources.push(ResourceEntry::new(
                name,
                index,
                (data_start + start_rel) as u64,
                (end_rel - start_rel) as u64,
            ));
        }

        Some(CbeSection {
            header: SectionHeader {
                index,
                file_offset: start_offset as u64,
                marker,
                resource_count,
                one,
                data_rel,
                data_len,
                data_start: data_start as u64,
            },
            resources,
        })
    }

    /// Get the file path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the complete CBE file image, including executable and data segments.
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get all sections
    pub fn sections(&self) -> &[CbeSection] {
        &self.sections
    }

    /// Get all resources
    pub fn resources(&self) -> &[ResourceEntry] {
        &self.all_resources
    }

    /// Find a resource by name
    pub fn find_resource(&self, name: &str) -> Option<&ResourceEntry> {
        self.all_resources.iter().find(|r| r.name == name)
    }

    /// Get resources of a specific type
    pub fn resources_by_type(&self, resource_type: ResourceType) -> Vec<&ResourceEntry> {
        self.all_resources
            .iter()
            .filter(|r| r.resource_type == resource_type)
            .collect()
    }

    /// Get all scene resources
    pub fn scenes(&self) -> Vec<&ResourceEntry> {
        self.resources_by_type(ResourceType::Scene)
    }

    /// Get all script resources
    pub fn scripts(&self) -> Vec<&ResourceEntry> {
        self.resources_by_type(ResourceType::Script)
    }

    /// Get all audio resources
    pub fn audio_resources(&self) -> Vec<&ResourceEntry> {
        self.resources_by_type(ResourceType::Audio)
    }

    /// Read raw bytes for a resource
    pub fn read_resource_bytes(&self, entry: &ResourceEntry) -> Result<&[u8]> {
        let start = entry.offset as usize;
        let end = start + entry.size as usize;

        if end > self.data.len() {
            anyhow::bail!(
                "Resource '{}' extends beyond file: offset=0x{:X}, size={}, file_size={}",
                entry.name,
                entry.offset,
                entry.size,
                self.data.len()
            );
        }

        Ok(&self.data[start..end])
    }

    /// Get a summary of the archive
    pub fn summary(&self) -> ArchiveSummary {
        let mut type_counts = std::collections::HashMap::new();
        for resource in &self.all_resources {
            *type_counts.entry(resource.resource_type).or_insert(0) += 1;
        }

        ArchiveSummary {
            path: self.path.clone(),
            file_size: self.data.len(),
            section_count: self.sections.len(),
            resource_count: self.all_resources.len(),
            type_counts,
        }
    }
}

/// Summary of a CBE archive
#[derive(Debug)]
pub struct ArchiveSummary {
    pub path: PathBuf,
    pub file_size: usize,
    pub section_count: usize,
    pub resource_count: usize,
    pub type_counts: std::collections::HashMap<ResourceType, usize>,
}

impl std::fmt::Display for ArchiveSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "CBE Archive: {}", self.path.display())?;
        writeln!(f, "  File size: {} bytes", self.file_size)?;
        writeln!(f, "  Sections: {}", self.section_count)?;
        writeln!(f, "  Resources: {}", self.resource_count)?;
        writeln!(f, "  By type:")?;
        for (rtype, count) in &self.type_counts {
            writeln!(f, "    {}: {}", rtype, count)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_type_from_extension() {
        assert_eq!(ResourceType::from_extension("sce"), ResourceType::Scene);
        assert_eq!(ResourceType::from_extension("map"), ResourceType::Map);
        assert_eq!(ResourceType::from_extension("actor"), ResourceType::Actor);
        assert_eq!(ResourceType::from_extension("xse"), ResourceType::Script);
        assert_eq!(ResourceType::from_extension("gif"), ResourceType::Image);
        assert_eq!(ResourceType::from_extension("mp3"), ResourceType::Audio);
        assert_eq!(ResourceType::from_extension("wav"), ResourceType::Audio);
        assert_eq!(
            ResourceType::from_extension("unknown"),
            ResourceType::Unknown
        );
    }

    #[test]
    fn audio_resources_filter_audio_entries() {
        let archive = CbeArchive {
            path: PathBuf::from("game.CBE"),
            data: Vec::new(),
            sections: Vec::new(),
            all_resources: vec![
                ResourceEntry::new("song.mp3".to_string(), 0, 0, 10),
                ResourceEntry::new("clip.wav".to_string(), 0, 0, 10),
                ResourceEntry::new("scene.sce".to_string(), 0, 0, 10),
            ],
        };
        assert_eq!(archive.audio_resources().len(), 2);
    }

    #[test]
    fn test_resource_entry_creation() {
        let entry = ResourceEntry::new("test.sce".to_string(), 0, 0x1000, 0x2000);
        assert_eq!(entry.resource_type, ResourceType::Scene);
        assert_eq!(entry.extension(), Some("sce"));
        assert!(entry.is_scene());
        assert!(!entry.is_script());
    }

    #[test]
    fn accepts_compact_single_resource_section() {
        let mut data = vec![0u8; 41];
        data[0..8].copy_from_slice(&CBE_SECTION_SIGNATURE);
        data[8..12].copy_from_slice(&8u32.to_le_bytes());
        data[12..16].copy_from_slice(&1u32.to_le_bytes());
        data[16..20].copy_from_slice(&1u32.to_le_bytes());
        data[20..24].copy_from_slice(&14u32.to_le_bytes());
        data[24..28].copy_from_slice(&1u32.to_le_bytes());
        data[36] = 1;
        data[37] = b'x';
        data[38] = 0x5a;

        let sections = CbeArchive::scan_sections(&data).unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].resources.len(), 1);
        assert_eq!(sections[0].resources[0].name, "x");
        assert_eq!(sections[0].resources[0].offset, 38);
        assert_eq!(sections[0].resources[0].size, 1);
    }

    #[test]
    fn accepts_native_resource_section_layout() {
        let mut data = vec![0u8; 43];
        data[0..8].copy_from_slice(&CBE_SECTION_SIGNATURE);
        data[8..12].copy_from_slice(&4u32.to_le_bytes());
        data[12..16].copy_from_slice(&1u32.to_le_bytes());
        data[16..20].copy_from_slice(&20u32.to_le_bytes());
        data[20..24].copy_from_slice(&3u32.to_le_bytes());
        data[24..28].copy_from_slice(&2u32.to_le_bytes());
        data[32..36].copy_from_slice(&1u32.to_le_bytes());
        data[36..40].copy_from_slice(&[1, b'a', 1, b'b']);
        data[40..43].copy_from_slice(&[0x11, 0x22, 0x33]);

        let sections = CbeArchive::scan_sections(&data).unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].resources.len(), 2);
        assert_eq!(sections[0].resources[0].name, "a");
        assert_eq!(sections[0].resources[0].offset, 40);
        assert_eq!(sections[0].resources[0].size, 1);
        assert_eq!(sections[0].resources[1].name, "b");
        assert_eq!(sections[0].resources[1].offset, 41);
        assert_eq!(sections[0].resources[1].size, 2);
    }
}
