//! ARM/Thumb execution and firmware service bridge.

use std::collections::{BTreeSet, HashMap, VecDeque};

use anyhow::{bail, Context, Result};
use armv4t_emu::{reg, Cpu, Memory, Mode};
use encoding_rs::GBK;
use log::{debug, warn};

use crate::cbe::CbeArchive;
use crate::image_decoder;

const ROM_BASE: u32 = 0x0100_0000;
const STACK_BASE: u32 = 0x0200_0000;
const STACK_SIZE: usize = 0x10_0000;
const HEAP_BASE: u32 = 0x0500_0000;
const HEAP_SIZE: usize = 0x100_0000;
const MANAGER_BASE: u32 = 0x0a00_0000;
const MANAGER_SIZE: usize = 0x10_0000;
const SERVICE_BASE: u32 = 0x0c00_0000;
const SERVICE_SIZE: u32 = 0x10_0000;
const EXIT_ADDRESS: u32 = 0x0f00_0000;

const TABLE_STRIDE: u32 = 0x400;
const MEMORY_BLOCK_POOL: u32 = HEAP_BASE + 0x40_0000;
const MEMORY_BLOCK_PTR: u32 = HEAP_BASE + 0x80_0000;
const MEMORY_BLOCK_SERVICE: u32 = SERVICE_BASE + 0x6c48;
const DREAM_FACTORY_PACKAGE_SLOT: u32 = MANAGER_BASE + 0x7ff0;
const DREAM_FACTORY_MEMORY_BLOCK_SLOT: u32 = MANAGER_BASE + 0x7ff4;
const SCREEN_IMAGE_STRUCT: u32 = MEMORY_BLOCK_PTR + 0x408;
const SCREEN_IMAGE: u32 = SCREEN_IMAGE_STRUCT + 24;
const SCREEN_IS_IN_QUIT: u32 = MANAGER_BASE + 0x7fe0;

/// Executable image metadata stored at the beginning of a CBE file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CbeExecutable {
    pub preferred_code_address: u32,
    pub code_image_size: u32,
    pub preferred_data_address: u32,
    pub data_image_size: u32,
    pub code_offset: usize,
    pub code_size: usize,
    pub data_offset: usize,
    pub initialized_data_size: usize,
    pub embedded_package_offset: usize,
    pub embedded_package_size: usize,
    pub resource_package_offset: usize,
    pub resource_package_size: usize,
    pub big_endian: bool,
}

impl CbeExecutable {
    /// Parse the executable header and verify all three segment checksums.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 0x98 {
            bail!("CBE executable header is truncated");
        }
        let read_be = |offset: usize| -> Result<u32> {
            let bytes = data
                .get(offset..offset + 4)
                .with_context(|| format!("CBE header field at 0x{offset:X} is truncated"))?;
            Ok(u32::from_be_bytes(bytes.try_into().unwrap()))
        };
        for index in 0..5 {
            let offset = index * 12;
            if data.get(offset..offset + 8) != Some(&[0xfe; 8]) {
                bail!("invalid CBE header marker at 0x{offset:X}");
            }
        }

        let preferred_code_address = read_be(8)?;
        let code_image_size = read_be(20)?;
        let preferred_data_address = read_be(32)?;
        let data_image_size = read_be(44)?;
        let variable_header_size = read_be(56)? as usize;
        let segment_header = 68usize
            .checked_add(variable_header_size)
            .context("CBE segment header offset overflow")?;
        for index in 0..6 {
            let offset = segment_header + index * 12;
            if data.get(offset..offset + 8) != Some(&[0xfe; 8]) {
                bail!("invalid CBE segment marker at 0x{offset:X}");
            }
        }

        let code_size = read_be(segment_header + 8)? as usize;
        let code_checksum = read_be(segment_header + 20)?;
        let initialized_data_size = read_be(segment_header + 32)? as usize;
        let data_checksum = read_be(segment_header + 44)?;
        let embedded_package_size = read_be(segment_header + 56)? as usize;
        let embedded_checksum = read_be(segment_header + 68)?;

        let mut cursor = 0x98;
        let code_offset = cursor;
        cursor = checked_advance(cursor, code_size, data.len(), "code")?;
        cursor = skip_marker(data, cursor)?;
        let data_offset = cursor;
        cursor = checked_advance(
            cursor,
            initialized_data_size,
            data.len(),
            "initialized data",
        )?;
        cursor = skip_marker(data, cursor)?;
        let embedded_package_offset = cursor;
        cursor = checked_advance(
            cursor,
            embedded_package_size,
            data.len(),
            "embedded package",
        )?;
        cursor = skip_marker(data, cursor)?;
        let resource_package_size = read_be(cursor)? as usize;
        cursor = checked_advance(cursor, 4, data.len(), "resource package length")?;
        cursor = skip_marker(data, cursor)?;
        let resource_package_offset = cursor;
        checked_advance(
            cursor,
            resource_package_size,
            data.len(),
            "resource package",
        )?;

        let code = &data[code_offset..code_offset + code_size];
        let initialized = &data[data_offset..data_offset + initialized_data_size];
        let embedded =
            &data[embedded_package_offset..embedded_package_offset + embedded_package_size];
        let little_matches = checksum_le(code) == code_checksum;
        let big_matches = checksum_be(code) == code_checksum;
        let big_endian = match (little_matches, big_matches) {
            (true, _) => false,
            (false, true) => true,
            _ => bail!("CBE code checksum mismatch"),
        };
        let checksum = if big_endian { checksum_be } else { checksum_le };
        if checksum(initialized) != data_checksum {
            bail!("CBE initialized-data checksum mismatch");
        }
        if checksum(embedded) != embedded_checksum {
            bail!("CBE embedded-package checksum mismatch");
        }

        Ok(Self {
            preferred_code_address,
            code_image_size,
            preferred_data_address,
            data_image_size,
            code_offset,
            code_size,
            data_offset,
            initialized_data_size,
            embedded_package_offset,
            embedded_package_size,
            resource_package_offset,
            resource_package_size,
            big_endian,
        })
    }

    pub fn code_address(&self) -> u32 {
        if self.preferred_code_address == 0 {
            ROM_BASE
        } else {
            self.preferred_code_address
        }
    }

    pub fn data_address(&self) -> u32 {
        if self.preferred_code_address == 0 {
            self.code_address().wrapping_add(self.code_image_size)
        } else {
            self.preferred_data_address
        }
    }
}

fn checked_advance(cursor: usize, size: usize, file_size: usize, name: &str) -> Result<usize> {
    let end = cursor
        .checked_add(size)
        .with_context(|| format!("{name} range overflow"))?;
    if end > file_size {
        bail!("{name} extends beyond the CBE file");
    }
    Ok(end)
}

fn skip_marker(data: &[u8], mut cursor: usize) -> Result<usize> {
    let start = cursor;
    while data.get(cursor) == Some(&0xfe) {
        cursor += 1;
    }
    if cursor == start {
        bail!("missing CBE segment separator at 0x{start:X}");
    }
    Ok(cursor)
}

fn checksum_le(data: &[u8]) -> u32 {
    data.chunks_exact(4).fold(0u32, |sum, bytes| {
        sum.wrapping_add(u32::from_le_bytes(bytes.try_into().unwrap()))
    })
}

fn checksum_be(data: &[u8]) -> u32 {
    data.chunks_exact(4).fold(0u32, |sum, bytes| {
        sum.wrapping_add(u32::from_be_bytes(bytes.try_into().unwrap()))
    })
}

fn signed_coord(value: u32) -> i32 {
    value as u16 as i16 as i32
}

fn clip_axis(
    source: &mut i32,
    size: &mut i32,
    destination: &mut i32,
    source_limit: i32,
    destination_limit: i32,
) {
    if *source < 0 {
        *destination -= *source;
        *size += *source;
        *source = 0;
    }
    if *destination < 0 {
        *source -= *destination;
        *size += *destination;
        *destination = 0;
    }
    *size = (*size)
        .min(source_limit.saturating_sub(*source))
        .min(destination_limit.saturating_sub(*destination));
}

fn service_trace_enabled(group: u32, index: u32) -> bool {
    let Ok(value) = std::env::var("CBE_TRACE") else {
        return false;
    };
    if matches!(value.as_str(), "1" | "all") {
        return true;
    }
    let service = format!("{group}:{index}");
    value.split(',').any(|filter| filter.trim() == service)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineState {
    Created,
    Initializing,
    Ready,
    Halted,
    Faulted,
}

struct Region {
    base: u32,
    data: Vec<u8>,
    read_only: bool,
}

struct MachineMemory {
    regions: Vec<Region>,
    bad_accesses: BTreeSet<u32>,
}

impl MachineMemory {
    fn new() -> Self {
        Self {
            regions: Vec::new(),
            bad_accesses: BTreeSet::new(),
        }
    }

    fn map(&mut self, base: u32, size: usize, read_only: bool) {
        self.regions.push(Region {
            base,
            data: vec![0; size],
            read_only,
        });
    }

    fn load(&mut self, address: u32, data: &[u8]) -> Result<()> {
        let region = self
            .region_mut(address, data.len())
            .context("load address is not mapped")?;
        let offset = (address - region.base) as usize;
        region.data[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    fn write_bytes(&mut self, address: u32, data: &[u8]) -> bool {
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

    fn region(&self, address: u32, size: usize) -> Option<&Region> {
        self.regions.iter().find(|region| {
            let offset = address.wrapping_sub(region.base) as usize;
            address >= region.base
                && offset
                    .checked_add(size)
                    .is_some_and(|end| end <= region.data.len())
        })
    }

    fn region_mut(&mut self, address: u32, size: usize) -> Option<&mut Region> {
        self.regions.iter_mut().find(|region| {
            let offset = address.wrapping_sub(region.base) as usize;
            address >= region.base
                && offset
                    .checked_add(size)
                    .is_some_and(|end| end <= region.data.len())
        })
    }

    fn read(&mut self, address: u32, size: usize) -> u32 {
        if let Some(region) = self.region(address, size) {
            let offset = (address - region.base) as usize;
            let mut bytes = [0u8; 4];
            bytes[..size].copy_from_slice(&region.data[offset..offset + size]);
            u32::from_le_bytes(bytes)
        } else {
            self.bad_accesses.insert(address);
            0
        }
    }

    fn write(&mut self, address: u32, value: u32, size: usize) {
        if let Some(region) = self.region_mut(address, size) {
            if region.read_only {
                self.bad_accesses.insert(address);
                return;
            }
            let offset = (address - region.base) as usize;
            region.data[offset..offset + size].copy_from_slice(&value.to_le_bytes()[..size]);
        } else {
            self.bad_accesses.insert(address);
        }
    }
}

#[derive(Debug, Clone)]
struct HostResource {
    name: String,
    data: Vec<u8>,
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

/// A platform-independent ARM machine for executable CBE games.
pub struct NicaiMachine {
    cpu: Cpu,
    memory: MachineMemory,
    executable: CbeExecutable,
    state: MachineState,
    heap_cursor: u32,
    app_main: u32,
    app_exit: u32,
    service_calls: HashMap<(u32, u32), u64>,
    instruction_count: u64,
    last_pc: u32,
    recent_pcs: VecDeque<u32>,
    pending_screen: u32,
    active_screen: u32,
    screen_initialized: bool,
    resource_load_pending: bool,
    key_down: u32,
    key_held: u32,
    resources: Vec<HostResource>,
    resource_data: Vec<u32>,
    resource_names: Vec<u32>,
}

impl std::fmt::Debug for NicaiMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NicaiMachine")
            .field("executable", &self.executable)
            .field("state", &self.state)
            .field("app_main", &format_args!("0x{:08X}", self.app_main))
            .field("instruction_count", &self.instruction_count)
            .field("last_pc", &format_args!("0x{:08X}", self.last_pc))
            .field(
                "recent_pcs",
                &self
                    .recent_pcs
                    .iter()
                    .map(|pc| format!("0x{pc:08X}"))
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl NicaiMachine {
    pub fn new(archive: &CbeArchive) -> Result<Self> {
        let executable = CbeExecutable::parse(archive.bytes())?;
        if executable.big_endian {
            bail!("big-endian CBE executables are not supported by the ARM core");
        }
        let code_address = executable.code_address();
        let data_address = executable.data_address();
        let resource_package_offset = executable.resource_package_offset;
        let rom_size = executable
            .code_image_size
            .saturating_add(executable.data_image_size)
            .max(executable.code_size as u32)
            .next_multiple_of(0x1000) as usize;
        let mut memory = MachineMemory::new();
        memory.map(code_address, rom_size, false);
        memory.map(STACK_BASE, STACK_SIZE, false);
        memory.map(HEAP_BASE, HEAP_SIZE, false);
        memory.map(MANAGER_BASE, MANAGER_SIZE, false);
        memory.load(
            code_address,
            &archive.bytes()[executable.code_offset..executable.code_offset + executable.code_size],
        )?;
        memory.load(
            data_address,
            &archive.bytes()
                [executable.data_offset..executable.data_offset + executable.initialized_data_size],
        )?;

        let mut machine = Self {
            cpu: Cpu::new(),
            memory,
            executable,
            state: MachineState::Created,
            heap_cursor: HEAP_BASE,
            app_main: 0,
            app_exit: 0,
            service_calls: HashMap::new(),
            instruction_count: 0,
            last_pc: 0,
            recent_pcs: VecDeque::with_capacity(32),
            pending_screen: 0,
            active_screen: 0,
            screen_initialized: false,
            resource_load_pending: false,
            key_down: 0,
            key_held: 0,
            resources: archive
                .sections()
                .iter()
                .find(|section| section.header.file_offset as usize + 8 == resource_package_offset)
                .map(|section| {
                    section
                        .resources
                        .iter()
                        .filter_map(|resource| {
                            archive
                                .read_resource_bytes(resource)
                                .ok()
                                .map(|data| HostResource {
                                    name: resource.name.clone(),
                                    data: data.to_vec(),
                                })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            resource_data: Vec::new(),
            resource_names: Vec::new(),
        };
        machine.initialize_tables();
        machine.initialize_screen();
        Ok(machine)
    }

    fn initialize_tables(&mut self) {
        for table_index in 0..27u32 {
            let table = MANAGER_BASE + TABLE_STRIDE * (table_index + 1);
            let service = SERVICE_BASE + TABLE_STRIDE * table_index;
            self.populate_table(table, service, TABLE_STRIDE / 4);
        }
        self.memory
            .w32(MANAGER_BASE + 8, MANAGER_BASE + TABLE_STRIDE);
        self.memory
            .w32(MANAGER_BASE + 12, SERVICE_BASE + SERVICE_SIZE - 4);
        self.memory.w32(MANAGER_BASE + 16, MANAGER_BASE + 0x9000);
    }

    fn initialize_screen(&mut self) {
        self.memory.w32(SCREEN_IMAGE_STRUCT, SCREEN_IMAGE);
        self.memory.w16(SCREEN_IMAGE_STRUCT + 4, 240);
        self.memory.w16(SCREEN_IMAGE_STRUCT + 6, 400);
        self.memory.w16(SCREEN_IMAGE_STRUCT + 8, 240);
    }

    fn populate_table(&mut self, table: u32, service: u32, count: u32) {
        for index in 0..count {
            self.memory.w32(table + index * 4, service + index * 4);
        }
    }

    pub fn boot(&mut self, instruction_limit: u64) -> Result<()> {
        self.state = MachineState::Initializing;
        let code_address = self.executable.code_address();
        let data_address = self.executable.data_address();
        self.cpu.reg_set(Mode::User, 0, MANAGER_BASE);
        self.cpu.reg_set(Mode::User, 9, data_address);
        self.cpu
            .reg_set(Mode::User, reg::SP, STACK_BASE + STACK_SIZE as u32);
        self.cpu.reg_set(Mode::User, reg::LR, EXIT_ADDRESS | 1);
        self.cpu.reg_set(Mode::User, reg::PC, code_address);
        self.cpu.reg_set(Mode::User, reg::CPSR, 0x30);
        self.run_until_return(instruction_limit)?;
        self.app_main = self.memory.r32(MANAGER_BASE);
        self.app_exit = self.memory.r32(MANAGER_BASE + 4);
        if self.app_main == 0 {
            self.state = MachineState::Faulted;
            bail!("CBE initializer returned without registering an application entry");
        }
        debug!("CBE application entry: 0x{:08X}", self.app_main);
        self.cpu.reg_set(Mode::User, reg::LR, EXIT_ADDRESS | 1);
        self.cpu.reg_set(Mode::User, reg::PC, self.app_main & !1);
        let mut cpsr = self.cpu.reg_get(Mode::User, reg::CPSR);
        if self.app_main & 1 != 0 {
            cpsr |= 1 << 5
        } else {
            cpsr &= !(1 << 5)
        }
        self.cpu.reg_set(Mode::User, reg::CPSR, cpsr);
        self.run_until_return(instruction_limit)?;
        self.state = MachineState::Ready;
        Ok(())
    }

    fn run_until_return(&mut self, instruction_limit: u64) -> Result<()> {
        for _ in 0..instruction_limit {
            let pc = self.cpu.reg_get(Mode::User, reg::PC);
            self.last_pc = pc;
            if self.recent_pcs.len() == 32 {
                self.recent_pcs.pop_front();
            }
            self.recent_pcs.push_back(pc);
            if pc & !1 == EXIT_ADDRESS {
                return Ok(());
            }
            if (SERVICE_BASE..SERVICE_BASE + SERVICE_SIZE).contains(&pc) {
                self.handle_service(pc)?;
            } else if self.cpu.thumb_mode() && self.memory.r16(pc) == 0xdfab {
                self.handle_semihosting(pc)?;
            } else if self.cpu.thumb_mode() && self.handle_thumb_blx(pc) {
            } else if self
                .memory
                .region(pc, if self.cpu.thumb_mode() { 2 } else { 4 })
                .is_none()
            {
                self.state = MachineState::Faulted;
                bail!("instruction fetch from unmapped address 0x{pc:08X}");
            } else if !self.cpu.step(&mut self.memory) {
                self.state = MachineState::Faulted;
                bail!("unsupported ARM instruction at 0x{pc:08X}");
            }
            self.instruction_count += 1;
        }
        self.state = MachineState::Faulted;
        bail!(
            "CBE execution exceeded {instruction_limit} instructions at 0x{:08X}",
            self.last_pc
        )
    }

    fn handle_thumb_blx(&mut self, pc: u32) -> bool {
        self.handle_thumb_blx_register(pc) || self.handle_thumb_blx_immediate(pc)
    }

    fn handle_thumb_blx_register(&mut self, pc: u32) -> bool {
        let instruction = self.memory.r16(pc);
        if instruction & 0xff87 != 0x4780 {
            return false;
        }
        let source = ((instruction >> 3) & 0x0f) as u8;
        let target = self.register(source);
        self.cpu.reg_set(Mode::User, reg::LR, pc + 3);
        self.cpu.reg_set(Mode::User, reg::PC, target & !1);
        let mut cpsr = self.register(reg::CPSR);
        if target & 1 != 0 {
            cpsr |= 1 << 5
        } else {
            cpsr &= !(1 << 5)
        }
        self.cpu.reg_set(Mode::User, reg::CPSR, cpsr);
        true
    }

    fn handle_thumb_blx_immediate(&mut self, pc: u32) -> bool {
        let prefix = self.memory.r16(pc);
        let suffix = self.memory.r16(pc + 2);
        if prefix & 0xf800 != 0xf000 || suffix & 0xf800 != 0xe800 {
            return false;
        }
        let raw_offset = (((prefix & 0x07ff) as u32) << 12) | (((suffix & 0x07ff) as u32) << 1);
        let offset = ((raw_offset << 9) as i32 >> 9) as u32;
        let target = pc.wrapping_add(4).wrapping_add(offset) & !3;
        self.cpu.reg_set(Mode::User, reg::LR, pc + 5);
        self.cpu.reg_set(Mode::User, reg::PC, target);
        let cpsr = self.register(reg::CPSR) & !(1 << 5);
        self.cpu.reg_set(Mode::User, reg::CPSR, cpsr);
        true
    }

    fn handle_semihosting(&mut self, pc: u32) -> Result<()> {
        let reason = self.register(0);
        let argument = self.register(1);
        match reason {
            3 => {
                if std::env::var("CBE_TRACE").is_ok() {
                    eprint!("{}", self.memory.r8(argument) as char);
                }
            }
            4 => {
                let mut bytes = Vec::new();
                for offset in 0..4096u32 {
                    let byte = self.memory.r8(argument.wrapping_add(offset));
                    if byte == 0 {
                        break;
                    }
                    bytes.push(byte);
                }
                let message = String::from_utf8_lossy(&bytes);
                if std::env::var("CBE_TRACE").is_ok() {
                    eprint!("{message}");
                }
            }
            _ => bail!("unhandled semihosting call reason={reason} at 0x{pc:08X}"),
        }
        self.cpu.reg_set(Mode::User, reg::PC, pc + 2);
        Ok(())
    }

    fn handle_service(&mut self, address: u32) -> Result<()> {
        if (MEMORY_BLOCK_SERVICE..MEMORY_BLOCK_SERVICE + 12).contains(&address) {
            let index = (address - MEMORY_BLOCK_SERVICE) / 4;
            *self.service_calls.entry((27, index)).or_default() += 1;
            self.handle_memory_block_service(index);
            self.return_from_service();
            return Ok(());
        }
        let offset = address - SERVICE_BASE;
        let group = offset / TABLE_STRIDE;
        let index = (offset % TABLE_STRIDE) / 4;
        *self.service_calls.entry((group, index)).or_default() += 1;
        let trace_service = service_trace_enabled(group, index);
        if trace_service {
            eprintln!(
                "service group={group} index={index} r0={:08X} r1={:08X} r2={:08X} r3={:08X} lr={:08X}",
                self.register(0),
                self.register(1),
                self.register(2),
                self.register(3),
                self.register(reg::LR),
            );
        }
        match group {
            0 => self.handle_root_service(index),
            1 => self.handle_system_service(index),
            2 => self.handle_memory_service(index),
            3 => self.handle_game_service(index),
            4 => self.handle_lcd_service(index),
            6 => self.handle_stdio_service(index),
            10 => self.handle_game_util_service(index),
            16 => self.handle_game_lcd_service(index),
            21 => self.handle_data_package_service(index),
            _ => self.set_result(0),
        }
        if trace_service {
            eprintln!("service result r0={:08X}", self.register(0));
        }
        self.return_from_service();
        Ok(())
    }

    fn handle_system_service(&mut self, index: u32) {
        match index {
            30 => self.set_result(46),
            33 => self.set_result(1002),
            _ => self.set_result(0),
        }
    }

    fn handle_root_service(&mut self, index: u32) {
        let table_group = match index {
            0 | 1 => Some(5),
            2 | 3 => Some(4),
            4 | 5 => Some(7),
            6 | 7 => Some(8),
            8 | 9 => Some(2),
            10 | 11 => Some(12),
            12 | 13 => Some(14),
            14 | 15 => Some(9),
            16 | 17 => Some(13),
            18 | 19 => Some(1),
            20 | 21 => Some(15),
            22 | 23 => Some(16),
            24 | 25 => Some(10),
            26 | 27 => Some(11),
            28 | 29 => Some(17),
            30 | 31 => Some(18),
            32 | 33 => Some(3),
            35 | 36 => Some(19),
            37 | 38 => Some(6),
            45 | 46 => Some(20),
            _ => None,
        };
        if let Some(table_group) = table_group {
            let table = MANAGER_BASE + TABLE_STRIDE * (table_group + 1);
            let service = SERVICE_BASE + TABLE_STRIDE * table_group;
            let is_initializer = matches!(
                index,
                0 | 2
                    | 4
                    | 6
                    | 8
                    | 10
                    | 12
                    | 14
                    | 16
                    | 18
                    | 20
                    | 22
                    | 24
                    | 26
                    | 28
                    | 30
                    | 32
                    | 35
                    | 37
                    | 45
            );
            if is_initializer {
                let destination = self.register(0);
                if destination != 0 {
                    self.populate_table(destination, service, TABLE_STRIDE / 4);
                }
                self.set_result(destination);
            } else {
                self.set_result(table);
            }
        } else {
            self.set_result(0);
        }
    }

    fn handle_memory_service(&mut self, index: u32) {
        match index {
            2 => {
                let destination = self.register(0);
                let size = self.register(1).max(2);
                let pointer = self.allocate(size);
                self.memory.w32(destination, pointer);
                self.set_result(1);
            }
            8 => self.set_result(MEMORY_BLOCK_PTR),
            9 => {
                self.initialize_memory_block(MEMORY_BLOCK_PTR, 0x40_0000);
                self.set_result(MEMORY_BLOCK_PTR);
            }
            13 => {
                let size = self.register(0).max(1);
                let pointer = self.allocate(size);
                self.set_result(pointer);
            }
            14 => self.set_result(0),
            _ => self.set_result(0),
        }
    }

    fn handle_stdio_service(&mut self, index: u32) {
        match index {
            3 => {
                let destination = self.register(0);
                let format = self.read_c_bytes(self.register(1), 4096);
                let output = self.format_c_string(&format);
                for (offset, byte) in output.iter().enumerate() {
                    self.memory
                        .w8(destination.wrapping_add(offset as u32), *byte);
                }
                self.memory
                    .w8(destination.wrapping_add(output.len() as u32), 0);
                self.set_result(output.len() as u32);
            }
            _ => self.set_result(0),
        }
    }

    fn format_c_string(&mut self, format: &[u8]) -> Vec<u8> {
        let mut output = Vec::new();
        let mut position = 0usize;
        let mut argument_index = 0u32;
        while position < format.len() {
            if format[position] != b'%' {
                output.push(format[position]);
                position += 1;
                continue;
            }
            position += 1;
            if format.get(position) == Some(&b'%') {
                output.push(b'%');
                position += 1;
                continue;
            }
            let zero_pad = format.get(position) == Some(&b'0');
            if zero_pad {
                position += 1;
            }
            let mut width = 0usize;
            while let Some(digit @ b'0'..=b'9') = format.get(position) {
                width = width * 10 + (digit - b'0') as usize;
                position += 1;
            }
            while matches!(format.get(position), Some(b'l' | b'h')) {
                position += 1;
            }
            let Some(specifier) = format.get(position).copied() else {
                break;
            };
            position += 1;
            let argument = match argument_index {
                0 => self.register(2),
                1 => self.register(3),
                index => self
                    .memory
                    .r32(self.register(reg::SP).wrapping_add((index - 2) * 4)),
            };
            argument_index += 1;
            let mut formatted = match specifier {
                b's' => self.read_c_bytes(argument, 4096),
                b'c' => vec![argument as u8],
                b'd' | b'i' => (argument as i32).to_string().into_bytes(),
                b'u' => argument.to_string().into_bytes(),
                b'x' => format!("{argument:x}").into_bytes(),
                b'X' => format!("{argument:X}").into_bytes(),
                _ => vec![b'%', specifier],
            };
            if formatted.len() < width {
                let padding = vec![if zero_pad { b'0' } else { b' ' }; width - formatted.len()];
                output.extend_from_slice(&padding);
            }
            output.append(&mut formatted);
        }
        output
    }

    fn handle_game_service(&mut self, index: u32) {
        match index {
            0 => {
                let source = self.resource_by_id(self.register(0));
                let result = self.create_image_from_stream(source, 0);
                self.set_result(result);
            }
            58 => {
                let block = self.register(0);
                let size = self.register(1);
                self.initialize_memory_block(block, size);
                self.set_result(block);
            }
            11 => {
                let mask = self.register(0);
                self.set_result(u32::from(self.key_down & mask != 0));
            }
            12 => {
                let mask = self.register(0);
                self.set_result(u32::from(self.key_held & mask != 0));
            }
            60 => {
                self.pending_screen = self.register(0);
                self.memory.w32(SCREEN_IS_IN_QUIT, 0);
                self.set_result(SCREEN_IS_IN_QUIT);
            }
            62 => {
                self.resource_load_pending = true;
                self.set_result(0);
            }
            68 => self.set_result(self.key_down),
            80 => {
                self.memory.w32(DREAM_FACTORY_PACKAGE_SLOT, 0);
                self.memory
                    .w32(DREAM_FACTORY_MEMORY_BLOCK_SLOT, MEMORY_BLOCK_PTR);
                self.set_result(0);
            }
            81 => {
                self.memory
                    .w32(DREAM_FACTORY_PACKAGE_SLOT, self.register(0));
                self.set_result(0);
            }
            82 => {
                let package = self.memory.r32(DREAM_FACTORY_PACKAGE_SLOT);
                self.set_result(package);
            }
            83 => {
                let result = self.resource_by_id(self.register(0));
                self.set_result(result);
            }
            84 => {
                let result = self.resource_by_name(self.register(0));
                self.set_result(result);
            }
            85 => {
                let result = self.resource_name_by_id(self.register(0));
                self.set_result(result);
            }
            86 => {
                let result = self.resource_id_by_name(self.register(0));
                self.set_result(result.unwrap_or(u32::MAX));
            }
            87 | 88 => {
                let result = self.resource_by_name(self.register(0));
                self.set_result(result);
            }
            90 => {
                let left = self.read_c_bytes(self.register(0), 4096);
                let right = self.read_c_bytes(self.register(1), 4096);
                self.set_result(u32::from(left == right));
            }
            91 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                let value = self.memory.r16(buffer.wrapping_add(offset));
                self.memory.w32(cursor, offset.wrapping_add(2));
                self.set_result(value as u32);
            }
            92 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                let value = self.memory.r32(buffer.wrapping_add(offset));
                self.memory.w32(cursor, offset.wrapping_add(4));
                self.set_result(value);
            }
            110 => {
                let package = self.register(0);
                let capacity = self.register(1);
                self.initialize_data_package(package, capacity);
            }
            _ => self.set_result(0),
        }
    }

    fn initialize_memory_block(&mut self, block: u32, size: u32) {
        let base = if block == MEMORY_BLOCK_PTR {
            MEMORY_BLOCK_POOL
        } else {
            self.allocate(size)
        };
        self.memory.w32(block, base);
        self.memory.w32(block + 4, 0);
        self.memory.w32(block + 8, size);
        self.memory.w32(block + 12, MEMORY_BLOCK_SERVICE);
        self.memory.w32(block + 16, MEMORY_BLOCK_SERVICE + 4);
        self.memory.w32(block + 20, MEMORY_BLOCK_SERVICE + 8);
    }

    fn initialize_data_package(&mut self, package: u32, capacity: u32) {
        for offset in [4, 8, 12, 16, 24, 28] {
            self.memory.w32(package + offset, 0);
        }
        self.memory.w8(package, 1);
        let entries = self.allocate(capacity.saturating_mul(4).max(4));
        self.memory.w32(package + 28, entries);
        self.memory.w32(package + 92, u32::MAX);
        self.memory.w32(package + 100, 0);
        for index in 0..=10 {
            self.memory.w32(
                package + 32 + index * 4,
                SERVICE_BASE + TABLE_STRIDE * 21 + index * 4,
            );
        }
        self.memory
            .w32(package + 80, SERVICE_BASE + TABLE_STRIDE * 21 + 11 * 4);
        self.set_result(SERVICE_BASE + TABLE_STRIDE * 21 + 10 * 4);
    }

    fn handle_data_package_service(&mut self, index: u32) {
        let package = self.register(0);
        match index {
            0 => {
                if self.register(1) == 0 {
                    self.memory.w8(package, 0);
                    self.set_result(0);
                } else {
                    self.set_result(0);
                }
            }
            4 => {
                self.load_main_resource_package(package);
                self.set_result(0);
            }
            5 => self.set_result(package),
            6 => {
                let result = self.resource_by_name(self.register(1));
                self.set_result(result);
            }
            7 => {
                let result = self.resource_by_id(self.register(1));
                self.set_result(result);
            }
            8 => {
                let result = self.resource_name_by_id(self.register(1));
                self.set_result(result);
            }
            9 => {
                let result = self.resource_id_by_name(self.register(1));
                self.set_result(result.unwrap_or(u32::MAX));
            }
            _ => self.set_result(0),
        }
    }

    fn load_main_resource_package(&mut self, package: u32) {
        if self.resources.is_empty() {
            warn!("CBE main resource package is empty");
            return;
        }
        if self.resource_data.is_empty() {
            let resources = self.resources.clone();
            self.resource_data.reserve(resources.len());
            self.resource_names.reserve(resources.len());
            for resource in resources {
                let data = self.allocate(resource.data.len().max(1) as u32);
                self.memory.write_bytes(data, &resource.data);
                self.resource_data.push(data);

                let name = self.allocate(resource.name.len() as u32 + 1);
                self.memory.write_bytes(name, resource.name.as_bytes());
                self.memory.w8(name + resource.name.len() as u32, 0);
                self.resource_names.push(name);
            }
        }

        let count = self.resources.len().min(u16::MAX as usize);
        let names = self.allocate((count * 4) as u32);
        let offsets = self.allocate((count * 4) as u32);
        let ids = self.allocate((count * 2) as u32);
        for index in 0..count {
            self.memory
                .w32(names + index as u32 * 4, self.resource_names[index]);
            self.memory
                .w32(offsets + index as u32 * 4, self.resource_data[index]);
            self.memory.w16(ids + index as u32 * 2, index as u16);
        }
        self.memory.w8(package, 1);
        self.memory.w16(package + 8, count as u16);
        self.memory.w32(package + 12, names);
        self.memory.w32(package + 16, offsets);
        self.memory.w32(package + 20, ids);
        self.memory.w32(package + 24, 0);
        self.memory.w32(package + 96, 0);
        debug!("loaded {count} CBE resources into guest memory");
    }

    fn resource_by_id(&self, id: u32) -> u32 {
        self.resource_data.get(id as usize).copied().unwrap_or(0)
    }

    fn resource_name_by_id(&self, id: u32) -> u32 {
        self.resource_names.get(id as usize).copied().unwrap_or(0)
    }

    fn resource_by_name(&mut self, name: u32) -> u32 {
        let Some(id) = self.resource_id_by_name(name) else {
            return 0;
        };
        self.resource_by_id(id)
    }

    fn resource_id_by_name(&mut self, address: u32) -> Option<u32> {
        let name = self.read_c_string(address, 256);
        self.resources
            .iter()
            .position(|resource| resource.name.eq_ignore_ascii_case(&name))
            .map(|index| index as u32)
    }

    fn read_c_string(&mut self, address: u32, limit: u32) -> String {
        String::from_utf8_lossy(&self.read_c_bytes(address, limit)).into_owned()
    }

    fn read_c_bytes(&mut self, address: u32, limit: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        for offset in 0..limit {
            let byte = self.memory.r8(address.wrapping_add(offset));
            if byte == 0 {
                break;
            }
            bytes.push(byte);
        }
        bytes
    }

    fn handle_memory_block_service(&mut self, index: u32) {
        let block = self.register(0);
        match index {
            0 => {
                let requested = self.register(1);
                let aligned = requested.saturating_add(3) & !3;
                let base = self.memory.r32(block);
                let offset = self.memory.r32(block + 4);
                let size = self.memory.r32(block + 8);
                if offset.saturating_add(aligned) <= size {
                    self.memory.w32(block + 4, offset + aligned);
                    self.set_result(base + offset);
                } else {
                    self.set_result(0);
                }
            }
            1 => {
                self.memory.w32(block + 4, 0);
                self.set_result(block);
            }
            2 => {
                self.memory.w32(block, 0);
                self.memory.w32(block + 4, 0);
                self.memory.w32(block + 8, 0);
                self.set_result(0);
            }
            _ => self.set_result(0),
        }
    }

    fn handle_lcd_service(&mut self, index: u32) {
        match index {
            0 => self.set_result(SCREEN_IMAGE_STRUCT),
            1 => self.set_result(SCREEN_IMAGE),
            5 => self.set_result(if self.register(0) == 0 { 8 } else { 16 }),
            6 => self.set_result(16),
            7 => {
                let bytes = self.read_c_bytes(self.register(0), 4096);
                let (text, _, _) = GBK.decode(&bytes);
                let width = text
                    .chars()
                    .map(|character| {
                        unifont::get_glyph(character)
                            .map(|glyph| glyph.get_width() as u32)
                            .unwrap_or(16)
                    })
                    .sum();
                self.set_result(width);
            }
            10 => {
                let string = self.register(1);
                let x = signed_coord(self.register(2));
                let y = signed_coord(self.register(3));
                let color = self.memory.r16(self.register(reg::SP));
                if service_trace_enabled(4, 10) {
                    let bytes = self.read_c_bytes(string, 256);
                    let (text, _, _) = GBK.decode(&bytes);
                    eprintln!("draw text x={x} y={y} color={color:04X} text={text:?}");
                }
                self.draw_text(string, x, y, color);
                self.set_result(1);
            }
            24 => {
                self.draw_image_clip(false);
                self.set_result(1);
            }
            25 => {
                self.draw_image_clip(true);
                self.set_result(1);
            }
            49 => {
                let result = self.create_image_from_stream(self.register(0), self.register(1));
                self.set_result(result);
            }
            _ => self.set_result(0),
        }
    }

    fn create_image_from_stream(&mut self, source: u32, output: u32) -> u32 {
        if source == 0 {
            return 0;
        }
        let Some(resource_index) = self
            .resource_data
            .iter()
            .position(|pointer| *pointer == source)
        else {
            if service_trace_enabled(4, 49) {
                eprintln!("image stream 0x{source:08X} did not match a resource");
            }
            warn!("image stream at 0x{source:08X} is not a CBE resource");
            return 0;
        };
        let resource = self.resources[resource_index].data.clone();
        let encoded = match resource.first() {
            Some(1 | 3) => &resource[1..],
            Some(_) => resource.as_slice(),
            None => return 0,
        };
        let decoded = match image_decoder::decode_image(encoded) {
            Ok(decoded) => decoded,
            Err(error) => {
                if service_trace_enabled(4, 49) {
                    eprintln!(
                        "image resource {} decode failed (head={:02X?}): {error:#}",
                        self.resources[resource_index].name,
                        &resource[..resource.len().min(12)]
                    );
                }
                warn!("failed to decode CBE image resource: {error:#}");
                return 0;
            }
        };
        if decoded.width == 0
            || decoded.height == 0
            || decoded.width > u16::MAX as u32
            || decoded.height > u16::MAX as u32
        {
            return 0;
        }
        let pitch = decoded.width.next_multiple_of(4);
        let pixels = self.allocate(pitch.saturating_mul(decoded.height).saturating_mul(2));
        if pixels == 0 {
            return 0;
        }
        for y in 0..decoded.height {
            for x in 0..decoded.width {
                let offset = ((y * decoded.width + x) * 4) as usize;
                let red = decoded.data[offset] as u16;
                let green = decoded.data[offset + 1] as u16;
                let blue = decoded.data[offset + 2] as u16;
                let alpha = decoded.data[offset + 3];
                let color = if alpha < 128 {
                    0
                } else {
                    ((red & 0xf8) << 8) | ((green & 0xfc) << 3) | (blue >> 3)
                };
                self.memory.w16(pixels + (y * pitch + x) * 2, color);
            }
        }
        let image = if output == 0 {
            self.allocate(12)
        } else {
            output
        };
        self.memory.w32(image, pixels);
        self.memory.w16(image + 4, decoded.width as u16);
        self.memory.w16(image + 6, decoded.height as u16);
        self.memory.w8(image + 8, 1);
        image
    }

    fn draw_image_clip(&mut self, transparent: bool) {
        let mut destination = self.register(0);
        let source = self.register(1);
        let mut source_x = signed_coord(self.register(2));
        let mut source_y = signed_coord(self.register(3));
        let stack = self.register(reg::SP);
        let mut width = signed_coord(self.memory.r32(stack));
        let mut height = signed_coord(self.memory.r32(stack + 4));
        let mut destination_x = signed_coord(self.memory.r32(stack + 8));
        let mut destination_y = signed_coord(self.memory.r32(stack + 12));

        let source_pixels = self.memory.r32(source);
        let source_width = self.memory.r16(source + 4) as i32;
        let source_height = self.memory.r16(source + 6) as i32;
        let mut destination_pixels = self.memory.r32(destination);
        let mut destination_width = self.memory.r16(destination + 4) as i32;
        let mut destination_height = self.memory.r16(destination + 6) as i32;
        if destination == SCREEN_IMAGE_STRUCT
            || destination_pixels == 0
            || destination_width <= 0
            || destination_height <= 0
            || destination_width > 240
            || destination_height > 400
        {
            destination = SCREEN_IMAGE_STRUCT;
            destination_pixels = SCREEN_IMAGE;
            destination_width = 240;
            destination_height = 400;
        }
        if source_pixels == 0
            || source_width <= 0
            || source_height <= 0
            || width <= 0
            || height <= 0
        {
            return;
        }

        clip_axis(
            &mut source_x,
            &mut width,
            &mut destination_x,
            source_width,
            destination_width,
        );
        clip_axis(
            &mut source_y,
            &mut height,
            &mut destination_y,
            source_height,
            destination_height,
        );
        if width <= 0 || height <= 0 {
            return;
        }
        let source_pitch = ((source_width + 3) & !3) as u32;
        let destination_pitch = ((destination_width + 3) & !3) as u32;
        for row in 0..height as u32 {
            for column in 0..width as u32 {
                let source_offset =
                    ((source_y as u32 + row) * source_pitch + source_x as u32 + column) * 2;
                let color = self.memory.r16(source_pixels + source_offset);
                if !transparent || color != 0 {
                    let destination_offset = ((destination_y as u32 + row) * destination_pitch
                        + destination_x as u32
                        + column)
                        * 2;
                    self.memory
                        .w16(destination_pixels + destination_offset, color);
                }
            }
        }
        let _ = destination;
    }

    fn draw_text(&mut self, address: u32, x: i32, y: i32, color: u16) {
        let bytes = self.read_c_bytes(address, 4096);
        let (text, _, _) = GBK.decode(&bytes);
        let mut pen_x = x;
        for character in text.chars() {
            let Some(glyph) = unifont::get_glyph(character) else {
                pen_x += 16;
                continue;
            };
            for glyph_y in 0..16i32 {
                let screen_y = y + glyph_y;
                if !(0..400).contains(&screen_y) {
                    continue;
                }
                for glyph_x in 0..glyph.get_width() as i32 {
                    let screen_x = pen_x + glyph_x;
                    if (0..240).contains(&screen_x)
                        && glyph.get_pixel(glyph_x as usize, glyph_y as usize)
                    {
                        let offset = (screen_y as u32 * 240 + screen_x as u32) * 2;
                        self.memory.w16(SCREEN_IMAGE + offset, color);
                    }
                }
            }
            pen_x += glyph.get_width() as i32;
        }
    }

    fn handle_game_util_service(&mut self, index: u32) {
        match index {
            9 => {
                self.memory
                    .w32(DREAM_FACTORY_PACKAGE_SLOT, self.register(0));
                self.set_result(0);
            }
            10 => {
                let package = self.memory.r32(DREAM_FACTORY_PACKAGE_SLOT);
                self.set_result(package);
            }
            11 => {
                let result = self.resource_by_id(self.register(0));
                self.set_result(result);
            }
            12 | 15 | 16 => {
                let result = self.resource_by_name(self.register(0));
                self.set_result(result);
            }
            13 => {
                let result = self.resource_name_by_id(self.register(0));
                self.set_result(result);
            }
            14 => {
                let result = self.resource_id_by_name(self.register(0));
                self.set_result(result.unwrap_or(u32::MAX));
            }
            18 => {
                let left = self.read_c_string(self.register(0), 4096);
                let right = self.read_c_string(self.register(1), 4096);
                self.set_result(u32::from(left == right));
            }
            19 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                let value = self.memory.r16(buffer.wrapping_add(offset));
                self.memory.w32(cursor, offset.wrapping_add(2));
                self.set_result(value as u32);
            }
            20 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                let value = self.memory.r32(buffer.wrapping_add(offset));
                self.memory.w32(cursor, offset.wrapping_add(4));
                self.set_result(value);
            }
            23 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                self.memory
                    .w16(buffer.wrapping_add(offset), self.register(2) as u16);
                self.memory.w32(cursor, offset.wrapping_add(2));
                self.set_result(offset.wrapping_add(2));
            }
            24 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                self.memory
                    .w32(buffer.wrapping_add(offset), self.register(2));
                self.memory.w32(cursor, offset.wrapping_add(4));
                self.set_result(offset.wrapping_add(4));
            }
            25 => {
                let result =
                    self.read_length_prefixed_string(self.register(0), self.register(1), false);
                self.set_result(result);
            }
            29 => {
                let result =
                    self.read_length_prefixed_string(self.register(0), self.register(1), true);
                self.set_result(result);
            }
            30 => self.set_result(MEMORY_BLOCK_PTR),
            _ => self.set_result(0),
        }
    }

    fn read_length_prefixed_string(&mut self, buffer: u32, cursor: u32, wide_length: bool) -> u32 {
        let mut offset = self.memory.r32(cursor);
        let length = if wide_length {
            let length = self.memory.r32(buffer.wrapping_add(offset));
            offset = offset.wrapping_add(4);
            length
        } else {
            let length = self.memory.r8(buffer.wrapping_add(offset)) as u32;
            offset = offset.wrapping_add(1);
            length
        };
        if length > HEAP_SIZE as u32 {
            return 0;
        }
        let result = self.allocate(length.saturating_add(1).max(1));
        for index in 0..length {
            let byte = self.memory.r8(buffer.wrapping_add(offset + index));
            self.memory.w8(result + index, byte);
        }
        self.memory.w8(result + length, 0);
        self.memory.w32(cursor, offset.wrapping_add(length));
        result
    }

    fn handle_game_lcd_service(&mut self, index: u32) {
        match index {
            11 => {
                let image = self.register(0);
                if image != 0 {
                    for offset in (0..12).step_by(4) {
                        self.memory.w32(image + offset, 0);
                    }
                }
                self.set_result(0);
            }
            20 => {
                let result = self.decode_resource_stream(self.register(0));
                self.set_result(result);
            }
            _ => self.set_result(0),
        }
    }

    fn decode_resource_stream(&mut self, source: u32) -> u32 {
        if source == 0 {
            return 0;
        }
        let compressed_size = u32::from_be_bytes([
            self.memory.r8(source + 1),
            self.memory.r8(source + 2),
            self.memory.r8(source + 3),
            self.memory.r8(source + 4),
        ]);
        let output_size = u32::from_be_bytes([
            self.memory.r8(source + 5),
            self.memory.r8(source + 6),
            self.memory.r8(source + 7),
            self.memory.r8(source + 8),
        ]) & 0x7fff_ffff;
        if service_trace_enabled(16, 20) {
            let name = self
                .resource_data
                .iter()
                .position(|pointer| *pointer == source)
                .map(|index| self.resources[index].name.as_str())
                .unwrap_or("<unknown>");
            eprintln!(
                "decode stream source={source:08X} name={name:?} compressed={compressed_size} output={output_size}"
            );
        }
        if compressed_size == 0 || output_size == 0 || output_size > HEAP_SIZE as u32 {
            return 0;
        }
        let output = self.allocate(output_size);
        if output == 0 {
            return 0;
        }
        let mut source_offset = 0u32;
        let mut output_offset = 0u32;
        while source_offset < compressed_size && output_offset < output_size {
            let command = self.memory.r8(source + 9 + source_offset);
            if command & 0x80 != 0 {
                let count = (command & 0x7f) as u32;
                if count == 0 || source_offset + 1 + count > compressed_size {
                    break;
                }
                let count = count.min(output_size - output_offset);
                for index in 0..count {
                    let byte = self.memory.r8(source + 10 + source_offset + index);
                    self.memory.w8(output + output_offset + index, byte);
                }
                source_offset += count + 1;
                output_offset += count;
            } else {
                if source_offset + 1 >= compressed_size {
                    break;
                }
                let count = (command >> 1) as u32;
                let distance = (((command as u32) << 8) & 0x1ff)
                    | self.memory.r8(source + 10 + source_offset) as u32;
                if count == 0 || distance == 0 || distance > output_offset {
                    break;
                }
                let count = count.min(output_size - output_offset);
                for index in 0..count {
                    let byte = self.memory.r8(output + output_offset - distance + index);
                    self.memory.w8(output + output_offset + index, byte);
                }
                source_offset += 2;
                output_offset += count;
            }
        }
        if service_trace_enabled(16, 20) {
            eprintln!("decode stream wrote={output_offset}");
        }
        if output_offset == 0 {
            0
        } else {
            output
        }
    }

    fn allocate(&mut self, size: u32) -> u32 {
        let aligned = size.saturating_add(7) & !7;
        let pointer = self.heap_cursor;
        let end = pointer.saturating_add(aligned);
        if end > HEAP_BASE + HEAP_SIZE as u32 {
            warn!("CBE heap exhausted while allocating {size} bytes");
            0
        } else {
            self.heap_cursor = end;
            pointer
        }
    }

    fn register(&self, register: armv4t_emu::reg::Reg) -> u32 {
        self.cpu.reg_get(Mode::User, register)
    }

    fn set_result(&mut self, value: u32) {
        self.cpu.reg_set(Mode::User, 0, value);
    }

    fn return_from_service(&mut self) {
        let target = self.register(reg::LR);
        self.cpu.reg_set(Mode::User, reg::PC, target & !1);
        let mut cpsr = self.register(reg::CPSR);
        if target & 1 != 0 {
            cpsr |= 1 << 5
        } else {
            cpsr &= !(1 << 5)
        }
        self.cpu.reg_set(Mode::User, reg::CPSR, cpsr);
    }

    fn invoke_callback(
        &mut self,
        entry: u32,
        r0: u32,
        r1: u32,
        r2: u32,
        instruction_limit: u64,
    ) -> Result<()> {
        if entry == 0 {
            return Ok(());
        }
        self.cpu.reg_set(Mode::User, 0, r0);
        self.cpu.reg_set(Mode::User, 1, r1);
        self.cpu.reg_set(Mode::User, 2, r2);
        self.cpu.reg_set(Mode::User, reg::LR, EXIT_ADDRESS | 1);
        self.cpu.reg_set(Mode::User, reg::PC, entry & !1);
        let mut cpsr = self.cpu.reg_get(Mode::User, reg::CPSR);
        if entry & 1 != 0 {
            cpsr |= 1 << 5;
        } else {
            cpsr &= !(1 << 5);
        }
        self.cpu.reg_set(Mode::User, reg::CPSR, cpsr);
        self.run_until_return(instruction_limit)
    }

    /// Execute one screen update and render pass.
    pub fn run_frame(&mut self, instruction_limit: u64) -> Result<()> {
        if self.state != MachineState::Ready {
            bail!("CBE machine is not ready");
        }
        if self.pending_screen != 0 && self.pending_screen != self.active_screen {
            self.active_screen = self.pending_screen;
            self.screen_initialized = false;
            self.resource_load_pending = false;
        }
        if self.active_screen == 0 {
            bail!("CBE application has no active screen");
        }

        let screen = self.active_screen;
        let screen_this = self.screen_call_parameter(screen);
        if !self.screen_initialized {
            let init = self.memory.r32(screen);
            self.invoke_callback(init, screen_this, 0, 0, instruction_limit)?;
            self.screen_initialized = true;
        }
        if self.resource_load_pending {
            self.resource_load_pending = false;
            let load_resource = self.memory.r32(screen + 24);
            self.invoke_callback(load_resource, screen_this, 0, 0, instruction_limit)?;
        }
        if self.pending_screen != 0 && self.pending_screen != screen {
            self.key_down = 0;
            return Ok(());
        }

        let logic = self.memory.r32(screen + 8);
        self.invoke_callback(logic, screen_this, 6, 0, instruction_limit)?;
        if self.pending_screen == 0 || self.pending_screen == screen {
            let render = self.memory.r32(screen + 12);
            if render == 0 {
                bail!("CBE screen at 0x{screen:08X} has no render callback");
            }
            self.invoke_callback(render, screen_this, 0, 0, instruction_limit)?;
        }
        self.key_down = 0;
        Ok(())
    }

    fn screen_call_parameter(&self, screen: u32) -> u32 {
        if screen >= self.executable.data_address() {
            screen.saturating_sub(0x18)
        } else {
            screen
        }
    }

    /// Set a guest key state. Key codes use the platform ABI values (0-20).
    pub fn set_key(&mut self, key: u8, pressed: bool) {
        if key >= 31 {
            return;
        }
        let mask = 1u32 << key;
        if pressed {
            if self.key_held & mask == 0 {
                self.key_down |= mask;
            }
            self.key_held |= mask;
        } else {
            self.key_held &= !mask;
        }
    }

    /// Copy the current 240x400 RGB565 screen into 0x00RRGGBB pixels.
    pub fn frame_pixels(&mut self) -> Vec<u32> {
        let mut pixels = Vec::with_capacity(240 * 400);
        for index in 0..(240 * 400) as u32 {
            let color = self.memory.r16(SCREEN_IMAGE + index * 2);
            let red = ((color >> 11) & 0x1f) as u32;
            let green = ((color >> 5) & 0x3f) as u32;
            let blue = (color & 0x1f) as u32;
            pixels.push(((red * 255 / 31) << 16) | ((green * 255 / 63) << 8) | (blue * 255 / 31));
        }
        pixels
    }

    pub fn state(&self) -> MachineState {
        self.state
    }
    pub fn executable(&self) -> &CbeExecutable {
        &self.executable
    }
    pub fn app_main(&self) -> u32 {
        self.app_main
    }
    pub fn last_pc(&self) -> u32 {
        self.last_pc
    }
    pub fn instruction_count(&self) -> u64 {
        self.instruction_count
    }
    pub fn service_calls(&self) -> &HashMap<(u32, u32), u64> {
        &self.service_calls
    }
    pub fn bad_accesses(&self) -> &BTreeSet<u32> {
        &self.memory.bad_accesses
    }
    pub fn active_screen(&self) -> u32 {
        self.active_screen
    }
    pub fn pending_screen(&self) -> u32 {
        self.pending_screen
    }
    pub fn register_value(&self, register: u8) -> u32 {
        self.register(register)
    }
    pub fn read_u32(&mut self, address: u32) -> u32 {
        self.memory.r32(address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_ignores_partial_words() {
        assert_eq!(checksum_le(&[1, 0, 0, 0, 9]), 1);
        assert_eq!(checksum_be(&[0, 0, 0, 1, 9]), 1);
    }
}
