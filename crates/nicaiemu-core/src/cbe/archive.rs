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

use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use log::{debug, info, warn};

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
        if &buf[off..off + 8] != CBE_SECTION_SIGNATURE {
            return false;
        }

        let marker = u32::from_le_bytes([buf[off + 8], buf[off + 9], buf[off + 10], buf[off + 11]]);
        let count = u32::from_le_bytes([buf[off + 12], buf[off + 13], buf[off + 14], buf[off + 15]]);
        let one = u32::from_le_bytes([buf[off + 16], buf[off + 17], buf[off + 18], buf[off + 19]]);
        let first_data_rel = u32::from_le_bytes([buf[off + 20], buf[off + 21], buf[off + 22], buf[off + 23]]);
        let data_len = u32::from_le_bytes([buf[off + 24], buf[off + 25], buf[off + 26], buf[off + 27]]);

        // Validate header values
        if marker != 8 || count < 1 || count > 10000 || one != 1 {
            return false;
        }
        if first_data_rel < 0x18 || data_len < 1 || first_data_rel as usize + data_len as usize + 0x18 > buf.len() - off {
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
            if len < 1 || len > 96 || pos + 1 + len > buf.len() {
                return false;
            }
            let name = &buf[pos + 1..pos + 1 + len];
            // Check if name looks like ASCII resource name
            let valid = name.iter().all(|&c| {
                (c >= 0x30 && c <= 0x39) ||
                (c >= 0x41 && c <= 0x5a) ||
                (c >= 0x61 && c <= 0x7a) ||
                c == 0x2e || c == 0x5f || c == 0x2d ||
                c >= 0x80 // Allow Chinese/other extended chars
            });
            if !valid {
                return false;
            }
            pos += 1 + len;
            checked += 1;
        }

        checked > 0
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
        let resource_count = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]);
        let one = u32::from_le_bytes([data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11]]);
        let data_rel = u32::from_le_bytes([data[pos + 12], data[pos + 13], data[pos + 14], data[pos + 15]]);
        let data_len = u32::from_le_bytes([data[pos + 16], data[pos + 17], data[pos + 18], data[pos + 19]]);

        debug!("Section {}: marker={}, {} resources, dataRel=0x{:X}, dataLen=0x{:X}",
               index, marker, resource_count, data_rel, data_len);

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
                data[table_pos], data[table_pos + 1],
                data[table_pos + 2], data[table_pos + 3]
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
            let name = String::from_utf8_lossy(&data[name_pos + 1..name_pos + 1 + name_len]).to_string();
            names.push(name);
            name_pos += 1 + name_len;
        }

        // Calculate data start offset
        let data_start = start_offset + data_rel as usize + 0x18;

        // Build resource entries
        let mut resources = Vec::with_capacity(resource_count as usize);
        for i in 0..resource_count as usize {
            let start_rel = if i == 0 { 0 } else { ends[i - 1] as usize };
            let end_rel = if i < ends.len() { ends[i] as usize } else { data_len as usize };

            let start = data_start + start_rel;
            let end = data_start + end_rel;
            let size = end.saturating_sub(start);

            let entry = ResourceEntry::new(
                names[i].clone(),
                index,
                start as u64,
                size as u64,
            );
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

        Some(CbeSection {
            header,
            resources,
        })
    }

    /// Get the file path
    pub fn path(&self) -> &Path {
        &self.path
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
        self.all_resources.iter()
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
        assert_eq!(ResourceType::from_extension("unknown"), ResourceType::Unknown);
    }

    #[test]
    fn test_resource_entry_creation() {
        let entry = ResourceEntry::new(
            "test.sce".to_string(),
            0,
            0x1000,
            0x2000,
        );
        assert_eq!(entry.resource_type, ResourceType::Scene);
        assert_eq!(entry.extension(), Some("sce"));
        assert!(entry.is_scene());
        assert!(!entry.is_script());
    }
}
