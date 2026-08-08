//! Sparse guest memory regions with byte-order-aware access.

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use armv4t_emu::Memory;
use serde::{Deserialize, Serialize};

/// Public snapshot of one mapped guest memory region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRegionInfo {
    /// Guest base address of the region.
    pub base: u32,
    /// Size of the region in bytes.
    pub size: usize,
    /// Whether guest writes to the region are rejected.
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Region {
    pub(crate) base: u32,
    pub(crate) data: Vec<u8>,
    pub(crate) read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MachineMemory {
    pub(crate) regions: Vec<Region>,
    pub(crate) bad_accesses: BTreeSet<u32>,
    big_endian: bool,
}

impl MachineMemory {
    pub(crate) fn new(big_endian: bool) -> Self {
        Self {
            regions: Vec::new(),
            bad_accesses: BTreeSet::new(),
            big_endian,
        }
    }

    pub(crate) fn map(&mut self, base: u32, size: usize, read_only: bool) {
        self.regions.push(Region {
            base,
            data: vec![0; size],
            read_only,
        });
    }

    pub(crate) fn load(&mut self, address: u32, data: &[u8]) -> Result<()> {
        let region = self
            .region_mut(address, data.len())
            .context("load address is not mapped")?;
        let offset = (address - region.base) as usize;
        region.data[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    pub(crate) fn write_bytes(&mut self, address: u32, data: &[u8]) -> bool {
        let Some(region) = self.region_mut(address, data.len()) else {
            self.bad_accesses.insert(address);
            return false;
        };
        if region.read_only {
            self.bad_accesses.insert(address);
            return false;
        }
        let offset = (address - region.base) as usize;
        region.data[offset..offset + data.len()].copy_from_slice(data);
        true
    }

    pub(crate) fn region(&self, address: u32, size: usize) -> Option<&Region> {
        self.regions.iter().find(|region| {
            let offset = address.wrapping_sub(region.base) as usize;
            address >= region.base
                && offset
                    .checked_add(size)
                    .is_some_and(|end| end <= region.data.len())
        })
    }

    pub(crate) fn region_mut(&mut self, address: u32, size: usize) -> Option<&mut Region> {
        self.regions.iter_mut().find(|region| {
            let offset = address.wrapping_sub(region.base) as usize;
            address >= region.base
                && offset
                    .checked_add(size)
                    .is_some_and(|end| end <= region.data.len())
        })
    }

    pub(crate) fn read(&mut self, address: u32, size: usize) -> u32 {
        if let Some(region) = self.region(address, size) {
            let offset = (address - region.base) as usize;
            let mut bytes = [0u8; 4];
            if self.big_endian {
                bytes[4 - size..].copy_from_slice(&region.data[offset..offset + size]);
                u32::from_be_bytes(bytes)
            } else {
                bytes[..size].copy_from_slice(&region.data[offset..offset + size]);
                u32::from_le_bytes(bytes)
            }
        } else {
            self.bad_accesses.insert(address);
            0
        }
    }

    pub(crate) fn write(&mut self, address: u32, value: u32, size: usize) {
        let big_endian = self.big_endian;
        if let Some(region) = self.region_mut(address, size) {
            if region.read_only {
                self.bad_accesses.insert(address);
                return;
            }
            let offset = (address - region.base) as usize;
            let bytes = if big_endian {
                value.to_be_bytes()
            } else {
                value.to_le_bytes()
            };
            let source = if big_endian {
                &bytes[4 - size..]
            } else {
                &bytes[..size]
            };
            region.data[offset..offset + size].copy_from_slice(source);
        } else {
            self.bad_accesses.insert(address);
        }
    }
}

impl Memory for MachineMemory {
    fn r8(&mut self, addr: u32) -> u8 {
        self.read(addr, 1) as u8
    }
    fn r16(&mut self, addr: u32) -> u16 {
        self.read(addr, 2) as u16
    }
    fn r32(&mut self, addr: u32) -> u32 {
        self.read(addr, 4)
    }
    fn w8(&mut self, addr: u32, val: u8) {
        self.write(addr, val as u32, 1)
    }
    fn w16(&mut self, addr: u32, val: u16) {
        self.write(addr, val as u32, 2)
    }
    fn w32(&mut self, addr: u32, val: u32) {
        self.write(addr, val, 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_memory_honors_guest_endianness() {
        for big_endian in [false, true] {
            let mut memory = MachineMemory::new(big_endian);
            memory.map(0x1000, 16, false);
            memory.w8(0x1000, 0x12);
            memory.w16(0x1002, 0x3456);
            memory.w32(0x1004, 0x789a_bcde);
            assert_eq!(memory.r8(0x1000), 0x12);
            assert_eq!(memory.r16(0x1002), 0x3456);
            assert_eq!(memory.r32(0x1004), 0x789a_bcde);
        }
    }
}
