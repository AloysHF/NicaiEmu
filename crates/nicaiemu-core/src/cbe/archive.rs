//! CBE Archive Parser
//!
//! Loads and parses CBE (Cool Bar Engine) game archives.
//! CBE files contain one or more resource sections marked by the signature
//! `FE FE FE FE FE FE FE FE`.

use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use log::{debug, info};

use super::resource::{ResourceEntry, ResourceType};

/// CBE section signature (8 bytes)
const CBE_SECTION_SIGNATURE: [u8; 8] = [0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE];

/// Header for a CBE section
#[derive(Debug, Clone)]
pub struct SectionHeader {
    /// Section index
    pub index: usize,
    /// Offset of this section in the file
    pub file_offset: u64,
    /// Size of the section header
    pub header_size: u32,
    /// Number of resources in this section
    pub resource_count: u32,
    /// Offset to the resource offset table
    pub offset_table_offset: u64,
    /// Offset to the name table
    pub name_table_offset: u64,
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
    /// Raw file data (memory-mapped or loaded)
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

    /// Scan the file for CBE sections
    fn scan_sections(data: &[u8]) -> Result<Vec<CbeSection>> {
        let mut sections = Vec::new();
        let mut offset = 0;
        let mut section_index = 0;

        while offset + 8 <= data.len() {
            // Check for section signature
            if &data[offset..offset + 8] == CBE_SECTION_SIGNATURE {
                debug!("Found section signature at offset 0x{:X}", offset);

                // Parse section header
                let section = Self::parse_section(data, offset, section_index)
                    .with_context(|| format!("Failed to parse section at 0x{:X}", offset))?;

                // Move offset past this section for next scan
                // For now, we'll scan the entire file for signatures
                sections.push(section);
                section_index += 1;
            }
            offset += 1;
        }

        Ok(sections)
    }

    /// Parse a single CBE section starting at the given offset
    fn parse_section(data: &[u8], start_offset: usize, index: usize) -> Result<CbeSection> {
        // Skip signature (8 bytes)
        let mut pos = start_offset + 8;

        // Read section header
        // The exact format needs to be determined from reverse engineering
        // For now, we'll use a simplified parser

        // Read resource count (assume 4 bytes, little-endian)
        if pos + 4 > data.len() {
            anyhow::bail!("Section header truncated at offset 0x{:X}", pos);
        }
        let resource_count = u32::from_le_bytes([
            data[pos], data[pos + 1], data[pos + 2], data[pos + 3]
        ]);
        pos += 4;

        debug!("Section {}: {} resources", index, resource_count);

        // Read offset table
        let mut offsets = Vec::with_capacity(resource_count as usize);
        for _ in 0..resource_count {
            if pos + 4 > data.len() {
                anyhow::bail!("Offset table truncated at offset 0x{:X}", pos);
            }
            let offset = u32::from_le_bytes([
                data[pos], data[pos + 1], data[pos + 2], data[pos + 3]
            ]);
            offsets.push(offset as u64);
            pos += 4;
        }

        // Read name table
        // Names are length-prefixed (1 or 2 bytes) followed by the name string
        let mut resources = Vec::with_capacity(resource_count as usize);
        let name_table_start = pos;

        for &offset in offsets.iter() {
            // Read name length (assume 1 byte for now)
            if pos >= data.len() {
                anyhow::bail!("Name table truncated at offset 0x{:X}", pos);
            }
            let name_len = data[pos] as usize;
            pos += 1;

            // Read name bytes
            if pos + name_len > data.len() {
                anyhow::bail!("Name string truncated at offset 0x{:X}", pos);
            }
            let name_bytes = &data[pos..pos + name_len];
            pos += name_len;

            // Convert to string (try UTF-8, fall back to GBK/GB2312)
            let name = String::from_utf8_lossy(name_bytes).to_string();

            // Calculate resource data offset
            // This is simplified; actual offset calculation may differ
            let resource_offset = start_offset + 8 + offset as usize;

            let entry = ResourceEntry::new(name, index, resource_offset as u64, 0);
            resources.push(entry);
        }

        let header = SectionHeader {
            index,
            file_offset: start_offset as u64,
            header_size: (pos - start_offset) as u32,
            resource_count,
            offset_table_offset: (start_offset + 8) as u64,
            name_table_offset: name_table_start as u64,
        };

        Ok(CbeSection {
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
