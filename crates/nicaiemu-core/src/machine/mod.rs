//! ARM/Thumb execution and firmware service bridge.

use std::collections::{BTreeSet, HashMap, VecDeque};

use anyhow::{bail, Context, Result};
use armv4t_emu::{reg, Cpu, Memory, Mode};
use encoding_rs::GBK;
use log::{debug, warn};
use serde::{Deserialize, Serialize};

use crate::audio_engine::{AudioDiagnostics, AudioEngine};
use crate::cbe::CbeArchive;
use crate::image_decoder;

mod cpu_bridge;
mod memory;
mod packages;
mod services;

use memory::MachineMemory;
pub use memory::MemoryRegionInfo;
use packages::{grouped_package_resources, native_package_resources, HostResource};

const ROM_BASE: u32 = 0x0100_0000;
const STACK_BASE: u32 = 0x0200_0000;
const STACK_SIZE: usize = 0x10_0000;
const HEAP_BASE: u32 = 0x0500_0000;
const HEAP_SIZE: usize = 0x100_0000;
const MANAGER_BASE: u32 = 0x0a00_0000;
const MANAGER_SIZE: usize = 0x10_0000;
const SERVICE_BASE: u32 = 0x0c00_0000;
const SERVICE_SIZE: u32 = 0x10_0000;
const LOG_NOOP_SERVICE: u32 = SERVICE_BASE + SERVICE_SIZE - 4;
const EXIT_ADDRESS: u32 = 0x0f00_0000;
const FIXED_MANAGER_INIT: u32 = SERVICE_BASE + 0xe000;
const FIXED_MANAGER_DIRECTORY: u32 = MANAGER_BASE + 0xa000;
const FIXED_GAMEOLD_OBJECT_SERVICE: u32 = SERVICE_BASE + 0xd000;
const FIXED_GAMEOLD_REGION_SERVICE: u32 = SERVICE_BASE + 0xd100;
const NATIVE_DISPATCH_SERVICE: u32 = SERVICE_BASE + 0xf000;
const NATIVE_SYSTEM_TIME_SERVICE: u32 = SERVICE_BASE + 0xf100;

const TABLE_STRIDE: u32 = 0x400;
const MAX_TIMERS: usize = 20;
const TIMER_BASE_ID: u32 = 100;
const TIMER_FRAME_MS: u32 = 100;
const MEMORY_BLOCK_POOL: u32 = HEAP_BASE + 0x40_0000;
const MEMORY_BLOCK_PTR: u32 = HEAP_BASE + 0x80_0000;
const MEMORY_BLOCK_SERVICE: u32 = SERVICE_BASE + 0x6c48;
const DREAM_FACTORY_PACKAGE_SLOT: u32 = MANAGER_BASE + 0x7ff0;
const DREAM_FACTORY_MEMORY_BLOCK_SLOT: u32 = MANAGER_BASE + 0x7ff4;
const DATA_PACKAGE_SIZE: u32 = 108;
const SCREEN_IMAGE_STRUCT: u32 = MEMORY_BLOCK_PTR + 0x408;
const SCREEN_IMAGE: u32 = SCREEN_IMAGE_STRUCT + 24;
const SCREEN_IS_IN_QUIT: u32 = MANAGER_BASE + 0x7fe0;
const DL_LOAD_MANAGER: u32 = MANAGER_BASE + 0xe_0000;
const VIDEO_MANAGER: u32 = MANAGER_BASE + 0xe_0400;
const DL_PAY_MANAGER: u32 = MANAGER_BASE + 0xe_0800;
const DL_RESOURCE_MANAGER: u32 = MANAGER_BASE + 0xe_0c00;
const DL_IMAGE_MANAGER: u32 = MANAGER_BASE + 0xe_1000;
const APP_STORE_MANAGER: u32 = MANAGER_BASE + 0xe_1400;

fn fixed_manager_specs() -> &'static [(u32, u32, u32)] {
    &[
        (0x00, 5, 0x60 / 4),
        (0x08, 4, 0xfc / 4),
        (0x10, 7, 0x18 / 4),
        (0x18, 8, 0x3c / 4),
        (0x20, 2, 0x48 / 4),
        (0x28, 12, 0x40 / 4),
        (0x30, 14, 0x2c / 4),
        (0x38, 9, 0x1c / 4),
        (0x40, 13, 0x30 / 4),
        (0x48, 1, 0x6c / 4),
        (0x50, 15, 0x58 / 4),
        (0x58, 16, 0xa0 / 4),
        (0x60, 10, 0xa0 / 4),
        (0x68, 11, 0x3c / 4),
        (0x70, 17, 0xf0 / 4),
        (0x78, 18, 0x24 / 4),
        (0x80, 3, 0x27c / 4),
        (0x8c, 19, 0x1c / 4),
    ]
}

fn manager_initializer_count(index: u32) -> Option<u32> {
    Some(match index {
        0 => 30,
        2 => 95,
        4 => 10,
        6 => 21,
        8 => 27,
        10 => 38,
        12 => 12,
        14 => 43,
        16 => 11,
        18 => 115,
        20 => TABLE_STRIDE / 4,
        22 => 24,
        24 => 40,
        30 => 31,
        32 => 144,
        35 => 11,
        37 => 22,
        45 => 6,
        _ => return None,
    })
}

fn game_service_string_uses_wide_length(index: u32) -> Option<bool> {
    match index {
        97 => Some(false),
        101 => Some(true),
        _ => None,
    }
}

/// Executable image metadata stored at the beginning of a CBE file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

        let mut cursor = segment_header
            .checked_add(6 * 12)
            .context("CBE executable header size overflow")?;
        let (code_offset, big_endian) =
            locate_checked_segment(data, cursor, code_size, code_checksum, None, "code")?;
        cursor = code_offset;
        cursor = checked_advance(cursor, code_size, data.len(), "code")?;
        let (data_offset, _) = locate_checked_segment(
            data,
            cursor,
            initialized_data_size,
            data_checksum,
            Some(big_endian),
            "initialized data",
        )?;
        cursor = data_offset;
        cursor = checked_advance(
            cursor,
            initialized_data_size,
            data.len(),
            "initialized data",
        )?;
        let (embedded_package_offset, _) = locate_checked_segment(
            data,
            cursor,
            embedded_package_size,
            embedded_checksum,
            Some(big_endian),
            "embedded package",
        )?;
        cursor = embedded_package_offset;
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

fn locate_checked_segment(
    data: &[u8],
    separator_start: usize,
    size: usize,
    expected_checksum: u32,
    big_endian: Option<bool>,
    name: &str,
) -> Result<(usize, bool)> {
    let mut separator_end = separator_start;
    while data.get(separator_end) == Some(&0xfe) {
        separator_end += 1;
    }
    let separator_len = separator_end.saturating_sub(separator_start);
    if separator_len < 8 {
        bail!("missing CBE segment separator at 0x{separator_start:X}");
    }

    let candidates = if size == 0 {
        separator_len..=separator_len
    } else {
        8..=separator_len
    };
    for separator_size in candidates {
        let offset = separator_start + separator_size;
        let Some(segment) = data.get(offset..offset.saturating_add(size)) else {
            continue;
        };
        match big_endian {
            Some(true) if checksum_be(segment) == expected_checksum => return Ok((offset, true)),
            Some(false) if checksum_le(segment) == expected_checksum => return Ok((offset, false)),
            None if checksum_le(segment) == expected_checksum => return Ok((offset, false)),
            None if checksum_be(segment) == expected_checksum => return Ok((offset, true)),
            _ => {}
        }
    }

    bail!("CBE {name} checksum mismatch")
}

fn skip_marker(data: &[u8], mut cursor: usize) -> Result<usize> {
    let start = cursor;
    while data.get(cursor) == Some(&0xfe) {
        cursor += 1;
    }
    if cursor.saturating_sub(start) < 8 {
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

fn arm_blx_immediate_target(pc: u32, instruction: u32) -> Option<u32> {
    if instruction & 0xfe00_0000 != 0xfa00_0000 {
        return None;
    }
    let offset = ((instruction & 0x00ff_ffff) << 2) | ((instruction >> 23) & 2);
    let signed_offset = ((offset << 6) as i32) >> 6;
    Some(pc.wrapping_add(8).wrapping_add(signed_offset as u32))
}

fn thumb_add_pc_target(pc: u32, instruction: u16, source_value: u32) -> Option<u32> {
    if instruction & 0xff87 != 0x4487 {
        return None;
    }
    Some(pc.wrapping_add(4).wrapping_add(source_value) & !1)
}

fn ascii_uppercase(value: u16) -> u16 {
    if (b'a' as u16..=b'z' as u16).contains(&value) {
        value - (b'a' - b'A') as u16
    } else {
        value
    }
}

fn image_payload(resource: &[u8]) -> &[u8] {
    if resource.first() == Some(&3)
        && resource.len() >= 17
        && (image::guess_format(&resource[9..]).is_ok() || &resource[9..17] == b"\x89PNGGAME")
    {
        &resource[9..]
    } else if matches!(resource.first(), Some(1 | 3)) {
        &resource[1..]
    } else {
        resource
    }
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

fn default_key_repeat_delay() -> u32 {
    10
}

fn default_key_repeat_on() -> u32 {
    1
}

fn default_key_repeat_off() -> u32 {
    14
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MachineState {
    Created,
    Initializing,
    Ready,
    Halted,
    Faulted,
}

/// Guest touch/pointer state with press and release edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct PointerState {
    held: bool,
    down: bool,
    up: bool,
    x: i32,
    y: i32,
    press_x: i32,
    press_y: i32,
}

/// One scheduled guest timer callback.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct GuestTimer {
    active: bool,
    callback: u32,
    context: u32,
    remaining_frames: u32,
}

impl PointerState {
    fn new() -> Self {
        Self {
            held: false,
            down: false,
            up: false,
            x: 0,
            y: 0,
            press_x: 0,
            press_y: 0,
        }
    }

    /// Update the pointer from a frontend source; edges are derived here.
    fn set(&mut self, x: i32, y: i32, down: bool) {
        self.x = x.clamp(0, 239);
        self.y = y.clamp(0, 399);
        if down && !self.held {
            self.down = true;
            self.press_x = self.x;
            self.press_y = self.y;
        }
        if !down && self.held {
            self.up = true;
        }
        self.held = down;
    }

    /// Clear per-frame edges after the guest frame consumed them.
    fn end_frame(&mut self) {
        self.down = false;
        self.up = false;
    }

    /// A held pointer that moved away from its press position.
    fn dragging(&self) -> bool {
        self.held && (self.x != self.press_x || self.y != self.press_y)
    }
}

/// A platform-independent ARM machine for executable CBE games.
#[derive(Serialize, Deserialize)]
pub struct NicaiMachine {
    cpu: Cpu,
    memory: MachineMemory,
    audio: AudioEngine,
    executable: CbeExecutable,
    state: MachineState,
    heap_cursor: u32,
    app_main: u32,
    app_exit: u32,
    service_calls: HashMap<(u32, u32), u64>,
    recent_services: VecDeque<(u32, u32, u32, u32)>,
    instruction_count: u64,
    frame_count: u64,
    last_pc: u32,
    recent_pcs: VecDeque<u32>,
    pending_screen: u32,
    active_screen: u32,
    screen_stack: Vec<u32>,
    screen_initialized: bool,
    resource_load_pending: bool,
    resource_load_screen: u32,
    key_down: u32,
    key_held: u32,
    key_held_physical: u32,
    key_press_frame: [u32; 31],
    key_frame_counter: u32,
    // Frontend configuration; deliberately excluded from save states.
    #[serde(skip, default = "default_key_repeat_delay")]
    key_repeat_delay: u32,
    #[serde(skip, default = "default_key_repeat_on")]
    key_repeat_on: u32,
    #[serde(skip, default = "default_key_repeat_off")]
    key_repeat_off: u32,
    pointer: PointerState,
    timers: Vec<GuestTimer>,
    resources: Vec<HostResource>,
    resource_data: Vec<u32>,
    resource_names: Vec<u32>,
    app_image_package: u32,
    inner_image_package: u32,
    current_image_package: u32,
    native_app_parser: u32,
    native_app_init: u32,
    native_system_info: u32,
    native_property_info: u32,
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
            .field("recent_services", &self.recent_services)
            .finish()
    }
}

impl NicaiMachine {
    pub fn new(archive: &CbeArchive) -> Result<Self> {
        let executable = CbeExecutable::parse(archive.bytes())?;
        let code_address = executable.code_address();
        let data_address = executable.data_address();
        let resource_package_offset = executable.resource_package_offset;
        let resource_package_size = executable.resource_package_size;
        let rom_size = executable
            .code_image_size
            .saturating_add(executable.data_image_size)
            .max(executable.code_size as u32)
            .next_multiple_of(0x1000) as usize;
        let mut memory = MachineMemory::new(executable.big_endian);
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
            audio: AudioEngine::new(),
            executable,
            state: MachineState::Created,
            heap_cursor: HEAP_BASE,
            app_main: 0,
            app_exit: 0,
            service_calls: HashMap::new(),
            recent_services: VecDeque::with_capacity(16),
            instruction_count: 0,
            frame_count: 0,
            last_pc: 0,
            recent_pcs: VecDeque::with_capacity(32),
            pending_screen: 0,
            active_screen: 0,
            screen_stack: Vec::new(),
            screen_initialized: false,
            resource_load_pending: false,
            resource_load_screen: 0,
            key_down: 0,
            key_held: 0,
            key_held_physical: 0,
            key_press_frame: [u32::MAX; 31],
            key_frame_counter: 0,
            key_repeat_delay: Self::KEY_REPEAT_DELAY,
            key_repeat_on: Self::KEY_REPEAT_ON_FRAMES,
            key_repeat_off: Self::KEY_REPEAT_OFF_FRAMES,
            pointer: PointerState::new(),
            timers: vec![
                GuestTimer {
                    active: false,
                    callback: 0,
                    context: 0,
                    remaining_frames: 0,
                };
                MAX_TIMERS
            ],
            resources: {
                let mut native = native_package_resources(archive.bytes(), resource_package_offset);
                if native.is_empty() {
                    native = grouped_package_resources(
                        archive.bytes(),
                        resource_package_offset,
                        resource_package_size,
                    );
                }
                if native.is_empty() {
                    archive
                        .sections()
                        .iter()
                        .find(|section| {
                            section.header.file_offset as usize + 8 == resource_package_offset
                        })
                        .map(|section| {
                            section
                                .resources
                                .iter()
                                .filter_map(|resource| {
                                    archive.read_resource_bytes(resource).ok().map(|data| {
                                        HostResource {
                                            name: resource.name.clone(),
                                            data: data.to_vec(),
                                        }
                                    })
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                } else {
                    native
                }
            },
            resource_data: Vec::new(),
            resource_names: Vec::new(),
            app_image_package: 0,
            inner_image_package: 0,
            current_image_package: 0,
            native_app_parser: 0,
            native_app_init: 0,
            native_system_info: 0,
            native_property_info: 0,
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
        self.memory.w32(MANAGER_BASE + 12, LOG_NOOP_SERVICE);
        self.memory.w32(MANAGER_BASE + 16, MANAGER_BASE + 0x9000);
        if self.uses_fixed_manager_abi() {
            for (index, &(offset, _, _)) in fixed_manager_specs().iter().enumerate() {
                self.memory.w32(
                    FIXED_MANAGER_DIRECTORY + offset,
                    FIXED_MANAGER_INIT + index as u32 * 4,
                );
            }
            self.memory.w32(MANAGER_BASE + 8, FIXED_MANAGER_DIRECTORY);
        }
        self.initialize_download_managers();
    }

    fn initialize_download_managers(&mut self) {
        self.populate_table(DL_LOAD_MANAGER, SERVICE_BASE + TABLE_STRIDE * 22, 11);
        self.populate_table(VIDEO_MANAGER, SERVICE_BASE + TABLE_STRIDE * 23, 38);
        self.populate_table(DL_PAY_MANAGER, SERVICE_BASE + TABLE_STRIDE * 24, 16);
        self.populate_table(DL_RESOURCE_MANAGER, SERVICE_BASE + TABLE_STRIDE * 25, 20);
        self.populate_table(DL_IMAGE_MANAGER, SERVICE_BASE + TABLE_STRIDE * 26, 12);
        self.populate_table(APP_STORE_MANAGER, SERVICE_BASE + TABLE_STRIDE * 28, 40);
    }

    fn uses_fixed_manager_abi(&self) -> bool {
        self.executable.big_endian
            && self.executable.preferred_code_address != 0
            && self.executable.code_image_size as usize > self.executable.code_size
    }

    fn uses_native_dispatch_abi(&self) -> bool {
        self.executable.big_endian
            && self.executable.preferred_code_address != 0
            && !self.uses_fixed_manager_abi()
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
        let application_interface = if self.uses_native_dispatch_abi() {
            NATIVE_DISPATCH_SERVICE | 1
        } else {
            MANAGER_BASE
        };
        self.cpu.reg_set(Mode::User, 0, application_interface);
        self.cpu.reg_set(Mode::User, 9, data_address);
        self.cpu
            .reg_set(Mode::User, reg::SP, STACK_BASE + STACK_SIZE as u32);
        self.cpu.reg_set(Mode::User, reg::LR, EXIT_ADDRESS | 1);
        self.cpu.reg_set(Mode::User, reg::PC, code_address);
        self.cpu.reg_set(Mode::User, reg::CPSR, 0x30);
        self.run_until_return(instruction_limit)?;
        if self.uses_native_dispatch_abi() {
            if self.native_app_parser == 0 {
                self.state = MachineState::Faulted;
                bail!("CBE initializer returned without registering a native application entry");
            }
            self.app_main = self.native_app_parser;
            self.invoke_callback(self.native_app_parser, 0, 0, 0, instruction_limit)?;
            self.invoke_callback(self.native_app_init, 0, 0, 0, instruction_limit)?;
            self.state = MachineState::Ready;
            return Ok(());
        }
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

    /// Rebuild the machine from an archive and re-run the boot sequence.
    ///
    /// Frontends use this for reset so a game restarts from a clean runtime
    /// state without reloading the file from disk.
    pub fn reset(&mut self, archive: &CbeArchive, instruction_limit: u64) -> Result<()> {
        let key_repeat_delay = self.key_repeat_delay;
        let key_repeat_on = self.key_repeat_on;
        let key_repeat_off = self.key_repeat_off;
        let volume = self.audio.volume();
        let mut rebuilt = NicaiMachine::new(archive)?;
        rebuilt.key_repeat_delay = key_repeat_delay;
        rebuilt.key_repeat_on = key_repeat_on;
        rebuilt.key_repeat_off = key_repeat_off;
        rebuilt.audio.set_volume(volume);
        rebuilt.boot(instruction_limit)?;
        *self = rebuilt;
        Ok(())
    }

    /// Base address of the guest heap, exposed as system RAM to frontends.
    pub fn system_ram_base() -> u32 {
        HEAP_BASE
    }

    /// Size of the guest heap in bytes.
    pub fn system_ram_size() -> usize {
        HEAP_SIZE
    }

    /// Base address of the guest screen framebuffer, exposed as video RAM.
    pub fn video_ram_base() -> u32 {
        SCREEN_IMAGE
    }

    /// Size of the guest screen framebuffer in bytes.
    pub fn video_ram_size() -> usize {
        240 * 400 * 2
    }

    /// Snapshot of the mapped guest memory regions for frontend memory maps.
    pub fn memory_regions(&self) -> Vec<MemoryRegionInfo> {
        self.memory
            .regions
            .iter()
            .map(|region| MemoryRegionInfo {
                base: region.base,
                size: region.data.len(),
                read_only: region.read_only,
            })
            .collect()
    }

    /// Mutable pointer to guest memory at a mapped address, if any.
    ///
    /// Region backing buffers are allocated once at map time, so the pointer
    /// stays valid for the lifetime of the machine.
    pub fn memory_pointer(&mut self, address: u32) -> Option<*mut u8> {
        let region = self.memory.region_mut(address, 1)?;
        let offset = address.checked_sub(region.base)? as usize;
        Some(unsafe { region.data.as_mut_ptr().add(offset) })
    }

    /// Mutable pointer to the guest heap, or null if it is not mapped.
    pub fn system_ram_pointer(&mut self) -> Option<*mut u8> {
        self.memory_pointer(Self::system_ram_base())
    }

    /// Mutable pointer to the guest screen framebuffer, or null if unmapped.
    pub fn video_ram_pointer(&mut self) -> Option<*mut u8> {
        self.memory_pointer(Self::video_ram_base())
    }

    fn handle_file_service(&mut self, index: u32) {
        match index {
            3 | 17 => self.set_result(1),
            13 | 14 => self.set_result(u32::MAX),
            16 => self.set_result(64 * 1024 * 1024),
            _ => self.set_result(0),
        }
    }

    fn handle_stdio_service(&mut self, index: u32) {
        match index {
            0 => {
                let destination = self.register(0);
                let source = self.register(1);
                let count = self.register(2);
                let bytes: Vec<u8> = (0..count)
                    .map(|offset| self.memory.r8(source + offset))
                    .collect();
                self.memory.write_bytes(destination, &bytes);
                self.set_result(destination);
            }
            1 => {
                let length = self.read_c_bytes(self.register(0), 0x1_0000).len() as u32;
                self.set_result(length);
            }
            2 => {
                let destination = self.register(0);
                let value = self.register(1) as u8;
                let count = self.register(2);
                for offset in 0..count {
                    self.memory.w8(destination + offset, value);
                }
                self.set_result(destination);
            }
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
                self.set_result(destination);
            }
            4 => self.set_result(0),
            5 => self.set_result(self.instruction_count as u32),
            7 => {
                let destination = self.register(0);
                let source = self.register(1);
                let count = self.register(2);
                for offset in 0..count {
                    let value = self.memory.r8(source + offset);
                    self.memory.w8(destination + offset, value);
                    if value == 0 {
                        for remainder in offset + 1..count {
                            self.memory.w8(destination + remainder, 0);
                        }
                        break;
                    }
                }
                self.set_result(destination);
            }
            8 | 9 => {
                let destination = self.register(0);
                let source = self.register(1);
                let destination_offset = if index == 9 {
                    self.read_c_bytes(destination, 0x1_0000).len() as u32
                } else {
                    0
                };
                let bytes = self.read_c_bytes(source, 0x1_0000);
                self.memory
                    .write_bytes(destination + destination_offset, &bytes);
                self.memory
                    .w8(destination + destination_offset + bytes.len() as u32, 0);
                self.set_result(destination);
            }
            10 | 12 => {
                let text = self.read_c_string(self.register(0), 256);
                let value = text.trim().parse::<i32>().unwrap_or(0);
                self.set_result(value as u32);
            }
            11 => {
                let destination = self.register(0);
                let source = self.register(1);
                let count = self.register(2);
                let bytes: Vec<u8> = (0..count)
                    .map(|offset| self.memory.r8(source + offset))
                    .collect();
                self.memory.write_bytes(destination, &bytes);
                self.set_result(destination);
            }
            13 => {
                let left = f32::from_bits(self.register(0));
                let right = f32::from_bits(self.register(1));
                self.set_result(left.powf(right).to_bits());
            }
            14..=16 => {
                let limit = if index == 14 {
                    0x1_0000
                } else {
                    self.register(2)
                };
                let stop_at_nul = index != 15;
                let result = self.compare_c_bytes(
                    self.register(0),
                    self.register(1),
                    limit,
                    stop_at_nul,
                    false,
                );
                self.set_result(result as u32);
            }
            20 | 21 => {
                let limit = if index == 20 {
                    0x1_0000
                } else {
                    self.register(2)
                };
                let result =
                    self.compare_c_bytes(self.register(0), self.register(1), limit, true, true);
                self.set_result(result as u32);
            }
            _ => self.set_result(0),
        }
    }

    fn compare_c_bytes(
        &mut self,
        left: u32,
        right: u32,
        limit: u32,
        stop_at_nul: bool,
        ignore_ascii_case: bool,
    ) -> i32 {
        for offset in 0..limit {
            let mut a = self.memory.r8(left + offset);
            let mut b = self.memory.r8(right + offset);
            if ignore_ascii_case {
                a = a.to_ascii_lowercase();
                b = b.to_ascii_lowercase();
            }
            if a != b {
                return a as i32 - b as i32;
            }
            if stop_at_nul && (a == 0 || b == 0) {
                break;
            }
        }
        0
    }

    fn handle_timer_service(&mut self, index: u32) {
        match index {
            // vMStartTimer(delay_ms, callback, context) -> timer handle.
            0 => {
                let delay_ms = self.register(0);
                let callback = self.register(1);
                let context = self.register(2);
                if callback == 0 {
                    self.set_result(0);
                    return;
                }
                let frames = delay_ms.saturating_add(TIMER_FRAME_MS - 1) / TIMER_FRAME_MS;
                let handle = self.timers.iter().position(|timer| !timer.active);
                match handle {
                    Some(slot) => {
                        self.timers[slot] = GuestTimer {
                            active: true,
                            callback,
                            context,
                            remaining_frames: frames.max(1),
                        };
                        self.set_result(TIMER_BASE_ID + slot as u32);
                    }
                    None => self.set_result(0),
                }
            }
            // vMStopTimer(handle).
            1 => {
                let handle = self.register(0);
                if handle >= TIMER_BASE_ID && (handle - TIMER_BASE_ID) < MAX_TIMERS as u32 {
                    self.timers[(handle - TIMER_BASE_ID) as usize].active = false;
                }
                self.set_result(0);
            }
            // vMGetTickCount() -> milliseconds since the frame counter started.
            2 => self.set_result((self.frame_count * TIMER_FRAME_MS as u64) as u32),
            // vMGetTotalSeconds / vMGetCurrentTime -> synthetic epoch seconds.
            3 | 4 => self.set_result(
                1_786_080_000u32
                    .wrapping_add((self.frame_count * TIMER_FRAME_MS as u64 / 1000) as u32),
            ),
            // vMSysSleep.
            5 => self.set_result(0),
            _ => self.set_result(0),
        }
    }

    /// Advance scheduled timers by one frame and fire due callbacks.
    fn dispatch_timers(&mut self, instruction_limit: u64) -> Result<()> {
        let mut due = Vec::new();
        for timer in &mut self.timers {
            if !timer.active {
                continue;
            }
            timer.remaining_frames = timer.remaining_frames.saturating_sub(1);
            if timer.remaining_frames == 0 {
                timer.active = false;
                due.push((timer.callback, timer.context));
            }
        }
        for (callback, context) in due {
            self.invoke_callback(callback, context, 0, 0, instruction_limit)?;
        }
        Ok(())
    }

    fn handle_ucs2_service(&mut self, index: u32) {
        match index {
            0 => {
                let length = self.ucs2_len(self.register(0), 0x8000);
                self.set_result(length);
            }
            1 => {
                let destination = self.register(0);
                let source = self.register(1);
                let length = self.ucs2_len(source, 0x8000);
                self.copy_ucs2(destination, source, length + 1);
                self.set_result(destination);
            }
            2 => {
                let destination = self.register(0);
                let source = self.register(1);
                let destination_length = self.ucs2_len(destination, 512);
                let source_length = self.ucs2_len(source, 0x8000);
                self.copy_ucs2(
                    destination + destination_length * 2,
                    source,
                    source_length + 1,
                );
                self.set_result(destination);
            }
            3 => {
                let destination = self.register(0);
                let bytes = self.read_c_bytes(self.register(1), 0x8000);
                let (text, _, _) = GBK.decode(&bytes);
                for (index, unit) in text.encode_utf16().enumerate() {
                    self.memory.w16(destination + index as u32 * 2, unit);
                }
                let length = text.encode_utf16().count() as u32;
                self.memory.w16(destination + length * 2, 0);
                self.set_result(destination);
            }
            4 => {
                let destination = self.register(0);
                let source = self.register(1);
                let count = self.register(2);
                self.copy_ucs2(destination, source, count);
                self.set_result(destination);
            }
            5 | 6 => {
                let destination = self.register(0);
                let source = self.register(1);
                let count = self.register(2);
                for offset in 0..count {
                    let byte = self.memory.r8(source + offset);
                    self.memory.w16(destination + offset * 2, byte as u16);
                    if index == 5 && byte == 0 {
                        for remainder in offset + 1..count {
                            self.memory.w16(destination + remainder * 2, 0);
                        }
                        break;
                    }
                }
                self.set_result(destination);
            }
            7 => {
                let source = self.register(0);
                let character = self.register(1) as u16;
                let mut result = 0;
                for offset in 0..0x8000 {
                    let value = self.memory.r16(source + offset * 2);
                    if value == character {
                        result = source + offset * 2;
                        break;
                    }
                    if value == 0 {
                        break;
                    }
                }
                self.set_result(result);
            }
            8..=10 => {
                let left = self.register(0);
                let right = self.register(1);
                let limit = if index == 10 {
                    self.register(2).min(0x8000)
                } else {
                    0x8000
                };
                let mut result = 0i32;
                for offset in 0..limit {
                    let mut a = self.memory.r16(left + offset * 2);
                    let mut b = self.memory.r16(right + offset * 2);
                    if index == 9 {
                        a = ascii_uppercase(a);
                        b = ascii_uppercase(b);
                    }
                    result = a as i32 - b as i32;
                    if result != 0 || a == 0 || b == 0 {
                        break;
                    }
                }
                self.set_result(result as u32);
            }
            _ => self.set_result(0),
        }
    }

    fn ucs2_len(&mut self, address: u32, limit: u32) -> u32 {
        for offset in 0..limit {
            if self.memory.r16(address + offset * 2) == 0 {
                return offset;
            }
        }
        limit
    }

    fn copy_ucs2(&mut self, destination: u32, source: u32, count: u32) {
        for offset in 0..count {
            let value = self.memory.r16(source + offset * 2);
            self.memory.w16(destination + offset * 2, value);
            if value == 0 {
                for remainder in offset + 1..count {
                    self.memory.w16(destination + remainder * 2, 0);
                }
                break;
            }
        }
    }

    fn handle_df_engine_service(&mut self, index: u32) {
        match index {
            8 => {
                self.memory.w32(DREAM_FACTORY_PACKAGE_SLOT, 0);
                self.memory
                    .w32(DREAM_FACTORY_MEMORY_BLOCK_SLOT, MEMORY_BLOCK_PTR);
                self.set_result(0);
            }
            10 => {
                let package = self.register(0);
                let capacity = self.register(1);
                self.initialize_data_package(package, capacity);
            }
            _ => self.set_result(0),
        }
    }

    fn handle_screen_service(&mut self, index: u32) {
        match index {
            0 | 2 | 3 => {
                let screen = self.register(0);
                if screen != 0 {
                    if let Some(current) = self.screen_stack.last_mut() {
                        *current = screen;
                    } else {
                        self.screen_stack.push(screen);
                    }
                    self.pending_screen = screen;
                    self.memory.w32(SCREEN_IS_IN_QUIT, 0);
                }
                self.set_result(SCREEN_IS_IN_QUIT);
            }
            1 | 7 | 8 => {
                let requested = self.register(0);
                self.resource_load_screen = if requested != 0 {
                    requested
                } else {
                    self.pending_screen
                };
                self.resource_load_pending = true;
                self.set_result(0);
            }
            4 | 5 => {
                let screen = self.register(0);
                if screen != 0 {
                    self.screen_stack.push(screen);
                    self.pending_screen = screen;
                    self.memory.w32(SCREEN_IS_IN_QUIT, 0);
                }
                self.set_result(0);
            }
            6 => {
                let screen = self.register(0);
                let removed = self
                    .screen_stack
                    .iter()
                    .rposition(|candidate| *candidate == screen)
                    .map(|position| {
                        self.screen_stack.remove(position);
                        true
                    })
                    .unwrap_or(false);
                if removed && (self.active_screen == screen || self.pending_screen == screen) {
                    self.pending_screen = self.screen_stack.last().copied().unwrap_or(0);
                    self.active_screen = self.pending_screen;
                    self.screen_initialized = false;
                }
                self.set_result(u32::from(removed));
            }
            9 => self.set_result(u32::from(
                self.screen_stack.last().copied() == Some(self.register(0)),
            )),
            10 => self.set_result(u32::from(
                self.screen_stack.first().copied() == Some(self.register(0)),
            )),
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
        if let Some(wide_length) = game_service_string_uses_wide_length(index) {
            let result =
                self.read_length_prefixed_string(self.register(0), self.register(1), wide_length);
            self.set_result(result);
            return;
        }
        match index {
            0 => {
                let source = self.resource_by_id(self.register(0));
                let result = self.create_image_from_stream(source, 0);
                self.set_result(result);
            }
            1 | 2 if self.uses_fixed_manager_abi() => {
                let source = self.register(0);
                let source_x = signed_coord(self.register(1));
                let source_y = signed_coord(self.register(2));
                let width = signed_coord(self.register(3));
                let stack = self.register(reg::SP);
                let height = signed_coord(self.memory.r32(stack));
                let destination_x = signed_coord(self.memory.r32(stack + 4));
                let destination_y = signed_coord(self.memory.r32(stack + 8));
                self.blit_image(
                    SCREEN_IMAGE_STRUCT,
                    source,
                    source_x,
                    source_y,
                    width,
                    height,
                    destination_x,
                    destination_y,
                    index == 2,
                );
                self.set_result(0);
            }
            23 => {
                let result = self.decode_resource_stream(self.register(0));
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
            14 if self.uses_fixed_manager_abi() => {
                self.set_result(self.register(1).wrapping_add(self.register(3)));
            }
            15 if self.uses_fixed_manager_abi() => {
                let image = self.register(0);
                let height = if image == 0 {
                    0
                } else {
                    self.memory.r16(image + 6) as u32
                };
                self.set_result(height);
            }
            16 if self.uses_fixed_manager_abi() => {
                let image = self.register(0);
                let width = if image == 0 {
                    0
                } else {
                    self.memory.r16(image + 4) as u32
                };
                self.set_result(width);
            }
            17 if self.uses_fixed_manager_abi() => {
                let red = self.register(0) as u16;
                let green = self.register(1) as u16;
                let blue = self.register(2) as u16;
                self.set_result(
                    (((red & 0xf8) << 8) | ((green & 0xfc) << 3) | ((blue & 0xf8) >> 3)) as u32,
                );
            }
            60 => {
                self.pending_screen = self.register(0);
                self.memory.w32(SCREEN_IS_IN_QUIT, 0);
                self.set_result(SCREEN_IS_IN_QUIT);
            }
            61 => {
                self.resource_load_screen = self.pending_screen;
                self.resource_load_pending = true;
                self.set_result(0);
            }
            62 => self.set_result(u32::from(self.pointer.held)),
            63 => self.set_result(u32::from(self.pointer.down)),
            64 => self.set_result(u32::from(self.pointer.up)),
            65 => self.set_result(u32::from(self.pointer.dragging())),
            66 => self.set_result(self.pointer.x as u32),
            67 => self.set_result(self.pointer.y as u32),
            68 => self.set_result(self.key_down),
            75 if self.uses_fixed_manager_abi() => {
                let object = self.register(0);
                let capacity = self.register(1) & 0xffff;
                let scanline = self.allocate(240 * 2);
                let resource_ids = self.allocate(capacity.saturating_mul(2).max(2));
                let pictures = self.allocate(capacity.saturating_mul(4).max(4));
                self.memory.w32(object, scanline);
                self.memory.w16(object + 8, capacity as u16);
                self.memory.w32(object + 12, resource_ids);
                self.memory.w32(object + 16, pictures);
                self.memory.w16(object + 20, 0);
                for method in 0..15 {
                    self.memory.w32(
                        object + 0x18 + method * 4,
                        FIXED_GAMEOLD_OBJECT_SERVICE + method * 4,
                    );
                }
                self.set_result(u32::from(
                    scanline != 0 && resource_ids != 0 && pictures != 0,
                ));
            }
            79 if self.uses_fixed_manager_abi() => {
                self.initialize_fixed_gameold_region();
            }
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
            95 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                self.memory
                    .w16(buffer.wrapping_add(offset), self.register(2) as u16);
                self.memory.w32(cursor, offset.wrapping_add(2));
                self.set_result(offset.wrapping_add(2));
            }
            96 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                self.memory
                    .w32(buffer.wrapping_add(offset), self.register(2));
                self.memory.w32(cursor, offset.wrapping_add(4));
                self.set_result(offset.wrapping_add(4));
            }
            102 => self.set_result(MEMORY_BLOCK_PTR),
            110 => {
                let package = self.register(0);
                let capacity = self.register(1);
                self.initialize_data_package(package, capacity);
            }
            _ => self.set_result(0),
        }
    }

    fn handle_fixed_gameold_object_service(&mut self, index: u32) {
        if index == 4 {
            let x = signed_coord(self.register(1));
            let y = signed_coord(self.register(2));
            let width = signed_coord(self.register(3));
            let stack = self.register(reg::SP);
            let height = signed_coord(self.memory.r32(stack));
            let color = self.memory.r32(stack + 4) as u16;
            self.fill_screen_rect(x, y, width, height, color);
        }
        self.set_result(0);
    }

    fn handle_fixed_gameold_region_service(&mut self, index: u32) {
        let object = self.register(0);
        match index {
            0 => {
                let slot = self.register(2);
                if slot <= 1 {
                    self.memory.w32(object + 0x20 + slot * 4, self.register(1));
                }
                self.set_result(object);
            }
            4 => {
                self.memory.w32(object + 4, 0);
                self.set_result(0);
            }
            5 => {
                let rectangle = self.register(2);
                let used = self.memory.r32(object + 4);
                let capacity = self.memory.r32(object + 8);
                let entries = self.memory.r32(object + 12);
                if rectangle != 0 && entries != 0 && used < capacity {
                    let entry = self.memory.r32(entries + used * 4);
                    if entry != 0 {
                        for offset in (0..8).step_by(2) {
                            let value = self.memory.r16(rectangle + offset);
                            self.memory.w16(entry + offset, value);
                        }
                        self.memory.w32(object + 4, used + 1);
                    }
                }
                self.set_result(0);
            }
            _ => self.set_result(0),
        }
    }

    fn initialize_fixed_gameold_region(&mut self) {
        let object = self.register(0);
        let first_bounds = self.register(1);
        let second_bounds = self.register(2);
        let owner_a = self.register(3);
        let stack = self.register(reg::SP);
        let owner_b = self.memory.r32(stack);
        let capacity = self.memory.r32(stack + 4);
        let entries = self.allocate(capacity.saturating_mul(4).max(4));
        for index in 0..capacity {
            let rectangle = self.allocate(8);
            self.memory.w32(entries + index * 4, rectangle);
        }
        self.memory.w32(object + 4, 0);
        self.memory.w32(object + 8, capacity);
        self.memory.w32(object + 12, entries);
        self.memory.w32(object + 16, owner_a);
        self.memory.w32(object + 20, owner_b);
        self.memory.w32(object + 24, first_bounds);
        self.memory.w32(object + 28, second_bounds);
        self.memory.w32(object + 32, 0);
        self.memory.w32(object + 36, 0);
        for method in 0..8 {
            self.memory.w32(
                object + 0x28 + method * 4,
                FIXED_GAMEOLD_REGION_SERVICE + method * 4,
            );
        }
        if capacity != 0 {
            let first = self.memory.r32(entries);
            self.memory.w32(first, first_bounds);
            self.memory.w32(first + 4, second_bounds);
            self.memory.w32(object + 4, 1);
        }
        self.set_result(u32::from(entries != 0));
    }

    fn handle_native_dispatch_service(&mut self) {
        let id = self.register(0);
        let argument = self.register(1);
        let code_start = self.executable.code_address();
        let code_end = code_start.saturating_add(self.executable.code_image_size);
        let data_start = self.executable.data_address();
        let data_end = data_start.saturating_add(self.executable.data_image_size);
        if (code_start..code_end).contains(&id)
            || (data_start..data_end).contains(&id)
            || (HEAP_BASE..HEAP_BASE + HEAP_SIZE as u32).contains(&id)
        {
            self.set_result(0);
            return;
        }
        match id {
            0x79e => {
                if argument != 0 {
                    self.native_app_parser = self.memory.r32(argument);
                    self.native_app_init = self.memory.r32(argument + 4);
                    self.memory.w32(argument + 8, NATIVE_DISPATCH_SERVICE | 1);
                }
                self.set_result(NATIVE_DISPATCH_SERVICE | 1);
            }
            0x52 => {
                if argument != 0 {
                    self.memory
                        .w32(self.executable.data_address() + 0x1724, argument);
                }
                self.set_result(0);
            }
            0x8e | 0x8f | 0x97 | 0xac | 0x421 | 0x41a => self.set_result(id),
            0x3ed => {
                if argument != 0 {
                    self.memory.w8(argument, 0);
                }
                self.set_result(0);
            }
            0x3ec | 0x3ee => {
                if argument != 0 {
                    for offset in 0..4 {
                        self.memory.w8(argument + offset, 0);
                    }
                }
                self.set_result(0);
            }
            0x7d1 => {
                self.handle_native_interface_request(argument);
                self.set_result(0);
            }
            _ => self.set_result(0),
        }
    }

    fn handle_native_interface_request(&mut self, argument: u32) {
        if argument == 0 {
            return;
        }
        let output = self.memory.r32(argument);
        let handle = self.memory.r32(argument + 4);
        let size = self.memory.r32(argument + 8);
        if output == 0 || size < 4 {
            return;
        }
        self.memory.w32(output, 0);
        match handle {
            0x8f => {
                if self.native_system_info == 0 {
                    self.native_system_info = self.allocate(0x400);
                    let info = self.native_system_info;
                    self.memory
                        .w32(info + 0x9c, SERVICE_BASE + TABLE_STRIDE * 2 + 13 * 4);
                    self.memory
                        .w32(info + 0xa0, SERVICE_BASE + TABLE_STRIDE * 2 + 14 * 4);
                    self.memory
                        .w32(info + 0x24, SERVICE_BASE + TABLE_STRIDE * 4 + 9 * 4);
                    self.memory
                        .w32(info + 0x58, SERVICE_BASE + TABLE_STRIDE * 4 + 19 * 4);
                    self.memory
                        .w32(info + 0x70, SERVICE_BASE + TABLE_STRIDE * 4 + 5 * 4);
                    self.memory
                        .w32(info + 0x74, SERVICE_BASE + TABLE_STRIDE * 4 + 5 * 4);
                    self.memory
                        .w32(info + 0x78, SERVICE_BASE + TABLE_STRIDE * 4 + 6 * 4);
                    self.memory
                        .w32(info + 0x20c, SERVICE_BASE + TABLE_STRIDE * 6);
                    self.memory
                        .w32(info + 0x210, SERVICE_BASE + TABLE_STRIDE * 6 + 4);
                    self.memory
                        .w32(info + 0x214, SERVICE_BASE + TABLE_STRIDE * 6 + 8);
                    self.memory
                        .w32(info + 0x22c, SERVICE_BASE + TABLE_STRIDE * 6 + 8 * 4);
                    for (offset, index) in [
                        (0xa4, 2),
                        (0xa8, 1),
                        (0xac, 0),
                        (0xb0, 3),
                        (0xb4, 4),
                        (0xb8, 5),
                    ] {
                        self.memory
                            .w32(info + offset, NATIVE_SYSTEM_TIME_SERVICE + index * 4);
                    }
                    self.memory.w32(info + 0xf0, NATIVE_DISPATCH_SERVICE | 1);
                }
                self.memory.w32(output, self.native_system_info);
            }
            0x8e => {
                if self.native_property_info == 0 {
                    self.native_property_info = self.allocate(0x100);
                    self.memory.w32(
                        self.native_property_info + 0x14,
                        NATIVE_DISPATCH_SERVICE | 1,
                    );
                }
                self.memory.w32(output, self.native_property_info);
            }
            0x41a => self.memory.w32(output, u32::MAX),
            _ => {}
        }
    }

    fn fill_screen_rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: u16) {
        let left = x.clamp(0, 240);
        let top = y.clamp(0, 400);
        let right = x.saturating_add(width).clamp(0, 240);
        let bottom = y.saturating_add(height).clamp(0, 400);
        for screen_y in top..bottom {
            for screen_x in left..right {
                self.memory.w16(
                    SCREEN_IMAGE + (screen_y as u32 * 240 + screen_x as u32) * 2,
                    color,
                );
            }
        }
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

    fn ensure_image_package(&mut self, inner: bool) -> u32 {
        let existing = if inner {
            self.inner_image_package
        } else {
            self.app_image_package
        };
        if existing != 0 {
            return existing;
        }

        let package = self.allocate(DATA_PACKAGE_SIZE);
        if inner {
            self.inner_image_package = package;
        } else {
            self.app_image_package = package;
        }
        package
    }

    fn initialize_image_data_page(&mut self, inner: bool) -> u32 {
        let package = self.ensure_image_package(inner);
        if package == 0 {
            return 0;
        }
        self.current_image_package = package;
        self.memory.w32(DREAM_FACTORY_PACKAGE_SLOT, package);
        let count = self.memory.r16(package + 8) as u32;
        if count != 0 {
            return count;
        }
        self.initialize_data_package(package, 5);
        self.load_main_resource_package(package);
        self.memory.r16(package + 8) as u32
    }

    fn create_image_from_data_package(&mut self, image_id: u32, package: u32, output: u32) -> u32 {
        if package == 0 || image_id >= self.memory.r16(package + 8) as u32 {
            return 0;
        }
        self.create_image_from_resource_index(image_id as usize, output)
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
            9 => {
                let string = self.register(0);
                let x = signed_coord(self.register(1));
                let y = signed_coord(self.register(2));
                let color = self.register(3) as u16;
                self.draw_text(string, x, y, color);
                self.set_result(1);
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
            11..=13 => {
                let r0 = self.register(0);
                let r0_is_string = self
                    .memory
                    .region(r0, 1)
                    .is_some_and(|region| region.data[(r0 - region.base) as usize] != 0);
                let (string, x, y, color) = if r0_is_string {
                    (
                        r0,
                        signed_coord(self.register(1)),
                        signed_coord(self.register(2)),
                        self.memory.r32(self.register(reg::SP) + 4) as u16,
                    )
                } else {
                    (
                        self.register(1),
                        signed_coord(self.register(2)),
                        signed_coord(self.register(3)),
                        self.memory.r16(self.register(reg::SP) + 16),
                    )
                };
                self.draw_text(string, x, y, color);
                self.set_result(1);
            }
            16 => {
                self.draw_rect(false, true);
                self.set_result(1);
            }
            17 => {
                self.draw_rect(true, true);
                self.set_result(1);
            }
            18 => {
                self.draw_rect(false, false);
                self.set_result(1);
            }
            19 => {
                let x = signed_coord(self.register(0));
                let y = signed_coord(self.register(1));
                let width = signed_coord(self.register(2));
                let height = signed_coord(self.register(3));
                if self.register(0) <= u16::MAX as u32
                    && (-239..240).contains(&x)
                    && (-399..400).contains(&y)
                    && (-239..=240).contains(&width)
                    && (-399..=400).contains(&height)
                {
                    let color = self.memory.r32(self.register(reg::SP)) as u16;
                    self.fill_screen_rect(x, y, width, height, color);
                } else {
                    self.draw_rect(true, false);
                }
                self.set_result(1);
            }
            22 => {
                let image_id = self.register(0);
                let output = self.register(1);
                let local_id = if image_id >= 0xfff {
                    image_id - 0xfff
                } else {
                    image_id
                };
                let result = self.create_image_from_resource_index(local_id as usize, output);
                self.set_result(result);
            }
            23 => self.set_result(0),
            24 => {
                self.draw_image_clip(false);
                self.set_result(1);
            }
            25 => {
                self.draw_image_clip(true);
                self.set_result(1);
            }
            26 | 28 => {
                let packed = self.register(1);
                self.draw_image_at(
                    self.register(0),
                    signed_coord(packed),
                    signed_coord(packed >> 16),
                    index == 28,
                );
                self.set_result(1);
            }
            27 => {
                self.draw_image_at(
                    self.register(0),
                    signed_coord(self.register(1)),
                    signed_coord(self.register(2)),
                    false,
                );
                self.set_result(1);
            }
            29 | 31 => {
                self.draw_image_packed(index == 31);
                self.set_result(1);
            }
            30 | 32 => {
                self.draw_image_full_clip(index == 32);
                self.set_result(1);
            }
            33 | 34 => {
                let image = self.register(0);
                let offset = if index == 33 { 4 } else { 6 };
                let result = self.memory.r16(image + offset) as u32;
                self.set_result(result);
            }
            35 => {
                let image = self.register(0);
                if image != 0 {
                    self.memory.w32(image, 0);
                    self.set_result(1);
                } else {
                    self.set_result(0);
                }
            }
            36 => self.set_result(1),
            44 => {
                let result = self.initialize_image_data_page(false);
                self.set_result(result);
            }
            45 => {
                let result = self.initialize_image_data_page(true);
                self.set_result(result);
            }
            46 => {
                self.app_image_package = 0;
                self.inner_image_package = 0;
                self.current_image_package = 0;
                self.memory.w32(DREAM_FACTORY_PACKAGE_SLOT, 0);
                self.set_result(0);
            }
            47 => {
                let package = self.register(2);
                if package == 0 {
                    self.set_result(0);
                } else {
                    self.current_image_package = package;
                    self.memory.w32(DREAM_FACTORY_PACKAGE_SLOT, package);
                    if self.memory.r16(package + 8) == 0 {
                        self.initialize_data_package(package, 5);
                        self.load_main_resource_package(package);
                    }
                    let count = self.memory.r16(package + 8) as u32;
                    self.set_result(count);
                }
            }
            48 => {
                let result = self.create_image_from_data_package(
                    self.register(0),
                    self.register(1),
                    self.register(2),
                );
                self.set_result(result);
            }
            49 => {
                let result = self.create_image_from_stream(self.register(0), self.register(1));
                self.set_result(result);
            }
            54..=56 => self.set_result(1),
            57 => self.set_result(0),
            62 => self.set_result(16),
            90..=92 => self.set_result(0),
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
        self.create_image_from_resource_index(resource_index, output)
    }

    fn create_image_from_resource_index(&mut self, resource_index: usize, output: u32) -> u32 {
        let Some(host_resource) = self.resources.get(resource_index) else {
            return 0;
        };
        let resource = host_resource.data.clone();
        let encoded = image_payload(&resource);
        let decoded = match image_decoder::decode_image(encoded) {
            Ok(decoded) => decoded,
            Err(error) => {
                if service_trace_enabled(4, 49) {
                    eprintln!(
                        "image resource {} decode failed (head={:02X?}): {error:#}",
                        host_resource.name,
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
        if service_trace_enabled(4, 49) {
            let opaque = decoded
                .data
                .chunks_exact(4)
                .filter(|pixel| pixel[3] >= 128)
                .count();
            eprintln!(
                "image resource {} decoded={}x{} opaque={opaque}",
                host_resource.name, decoded.width, decoded.height
            );
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
        let destination = self.register(0);
        let source = self.register(1);
        let source_x = signed_coord(self.register(2));
        let source_y = signed_coord(self.register(3));
        let stack = self.register(reg::SP);
        let width = signed_coord(self.memory.r32(stack));
        let height = signed_coord(self.memory.r32(stack + 4));
        let destination_x = signed_coord(self.memory.r32(stack + 8));
        let destination_y = signed_coord(self.memory.r32(stack + 12));
        self.blit_image(
            destination,
            source,
            source_x,
            source_y,
            width,
            height,
            destination_x,
            destination_y,
            transparent,
        );
    }

    fn draw_image_at(&mut self, source: u32, x: i32, y: i32, transparent: bool) {
        let width = self.memory.r16(source + 4) as i32;
        let height = self.memory.r16(source + 6) as i32;
        self.blit_image(
            SCREEN_IMAGE_STRUCT,
            source,
            0,
            0,
            width,
            height,
            x,
            y,
            transparent,
        );
    }

    fn draw_image_packed(&mut self, transparent: bool) {
        let source = self.register(0);
        let source_start = self.register(1);
        let destination_start = self.register(2);
        let destination_end = self.register(3);
        let source_x = signed_coord(source_start);
        let source_y = signed_coord(source_start >> 16);
        let destination_x = signed_coord(destination_start);
        let destination_y = signed_coord(destination_start >> 16);
        let width = signed_coord(destination_end) - destination_x + 1;
        let height = signed_coord(destination_end >> 16) - destination_y + 1;
        self.blit_image(
            SCREEN_IMAGE_STRUCT,
            source,
            source_x,
            source_y,
            width,
            height,
            destination_x,
            destination_y,
            transparent,
        );
    }

    fn draw_image_full_clip(&mut self, transparent: bool) {
        let source = self.register(0);
        let source_x = signed_coord(self.register(1));
        let source_y = signed_coord(self.register(2));
        let width = signed_coord(self.register(3));
        let stack = self.register(reg::SP);
        let height = signed_coord(self.memory.r32(stack));
        let destination_x = signed_coord(self.memory.r32(stack + 4));
        let destination_y = signed_coord(self.memory.r32(stack + 8));
        self.blit_image(
            SCREEN_IMAGE_STRUCT,
            source,
            source_x,
            source_y,
            width,
            height,
            destination_x,
            destination_y,
            transparent,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn blit_image(
        &mut self,
        mut destination: u32,
        source: u32,
        mut source_x: i32,
        mut source_y: i32,
        mut width: i32,
        mut height: i32,
        mut destination_x: i32,
        mut destination_y: i32,
        transparent: bool,
    ) {
        let source_pixels = self.memory.r32(source);
        let source_width = self.memory.r16(source + 4) as i32;
        let source_height = self.memory.r16(source + 6) as i32;
        let mut destination_pixels = self.memory.r32(destination);
        let mut destination_width = self.memory.r16(destination + 4) as i32;
        let mut destination_height = self.memory.r16(destination + 6) as i32;
        if service_trace_enabled(4, 24)
            || service_trace_enabled(4, if transparent { 26 } else { 25 })
        {
            eprintln!(
                "draw image dst={destination:08X} src={source:08X} pixels={source_pixels:08X} size={source_width}x{source_height} clip={source_x},{source_y} {width}x{height} at={destination_x},{destination_y}"
            );
        }
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

    fn draw_rect(&mut self, has_destination: bool, outline: bool) {
        let (mut destination, mut x, mut y, mut width) = if has_destination {
            (
                self.register(0),
                signed_coord(self.register(1)),
                signed_coord(self.register(2)),
                signed_coord(self.register(3)),
            )
        } else {
            (
                SCREEN_IMAGE_STRUCT,
                signed_coord(self.register(0)),
                signed_coord(self.register(1)),
                signed_coord(self.register(2)),
            )
        };
        let stack = self.register(reg::SP);
        let mut height = signed_coord(self.memory.r32(stack));
        let color = self.memory.r32(stack + 4) as u16;
        let mut pixels = self.memory.r32(destination);
        let mut destination_width = self.memory.r16(destination + 4) as i32;
        let mut destination_height = self.memory.r16(destination + 6) as i32;
        if destination == SCREEN_IMAGE_STRUCT
            || pixels == 0
            || destination_width <= 0
            || destination_height <= 0
            || destination_width > 240
            || destination_height > 400
        {
            destination = SCREEN_IMAGE_STRUCT;
            pixels = SCREEN_IMAGE;
            destination_width = 240;
            destination_height = 400;
        }
        let mut source_x = 0;
        let mut source_y = 0;
        clip_axis(
            &mut source_x,
            &mut width,
            &mut x,
            i32::MAX,
            destination_width,
        );
        clip_axis(
            &mut source_y,
            &mut height,
            &mut y,
            i32::MAX,
            destination_height,
        );
        if width <= 0 || height <= 0 {
            return;
        }
        let pitch = ((destination_width + 3) & !3) as u32;
        for row in 0..height {
            for column in 0..width {
                if outline && row != 0 && row != height - 1 && column != 0 && column != width - 1 {
                    continue;
                }
                let offset = ((y + row) as u32 * pitch + (x + column) as u32) * 2;
                self.memory.w16(pixels + offset, color);
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

    fn handle_download_service(&mut self, index: u32) {
        if index == 4 {
            self.set_result(u32::MAX);
        } else {
            self.set_result(0);
        }
    }

    fn handle_payment_service(&mut self, index: u32) {
        if index == 7 {
            let destination = self.register(0);
            let capacity = self.register(1);
            let identifier = b"111111111111111\0";
            let count = capacity.min(identifier.len() as u32);
            if destination != 0 && count != 0 {
                self.memory
                    .write_bytes(destination, &identifier[..count as usize]);
                self.memory.w8(destination + count - 1, 0);
            }
        }
        self.set_result(0);
    }

    fn handle_download_resource_service(&mut self, index: u32) {
        match index {
            0 => {
                let package = self.memory.r32(DREAM_FACTORY_PACKAGE_SLOT);
                self.set_result(package);
            }
            10 => {
                let result = self.create_image_from_stream(self.register(0), self.register(1));
                self.set_result(result);
            }
            _ => self.set_result(0),
        }
    }

    fn handle_download_image_service(&mut self, index: u32) {
        if index == 4 {
            let pointer = self.allocate(self.register(0));
            self.set_result(pointer);
        } else {
            self.set_result(0);
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

    /// Execute one screen update and render pass.
    pub fn run_frame(&mut self, instruction_limit: u64) -> Result<()> {
        if self.state != MachineState::Ready {
            bail!("CBE machine is not ready");
        }
        self.update_key_state();
        self.frame_count = self.frame_count.wrapping_add(1);
        self.dispatch_timers(instruction_limit)?;
        if self.uses_native_dispatch_abi() {
            return self.invoke_callback(self.native_app_parser, 0, 0, 0, instruction_limit);
        }
        if self.pending_screen != 0 && self.pending_screen != self.active_screen {
            self.active_screen = self.pending_screen;
            self.screen_initialized = false;
        }
        if self.active_screen == 0 {
            bail!("CBE application has no active screen");
        }

        let screen = self.active_screen;
        let screen_this = self.screen_call_parameter(screen);
        if std::env::var("CBE_TRACE").is_ok() {
            eprintln!(
                "screen callbacks screen={screen:08X} this={screen_this:08X} init={:08X} logic={:08X} render={:08X} load={:08X} pending_load={} sp={:08X}",
                self.memory.r32(screen),
                self.memory.r32(screen + 8),
                self.memory.r32(screen + 12),
                self.memory.r32(screen + 24),
                self.resource_load_pending,
                self.register(reg::SP),
            );
        }
        if !self.screen_initialized {
            let init = self.memory.r32(screen);
            self.invoke_callback(init, screen_this, 0, 0, instruction_limit)?;
            self.screen_initialized = true;
        }
        if self.resource_load_pending
            && (self.resource_load_screen == 0 || self.resource_load_screen == screen)
        {
            self.resource_load_pending = false;
            self.resource_load_screen = 0;
            let load_resource = self.memory.r32(screen + 24);
            self.invoke_callback(load_resource, screen_this, 0, 0, instruction_limit)?;
        }
        if self.pending_screen != 0 && self.pending_screen != screen {
            self.key_down = 0;
            self.pointer.end_frame();
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
        self.pointer.end_frame();
        Ok(())
    }

    fn screen_call_parameter(&self, screen: u32) -> u32 {
        if screen >= self.executable.data_address() {
            screen.saturating_sub(0x18)
        } else {
            screen
        }
    }

    /// Frames a held key stays visible for one walk step or repeat pulse.
    const KEY_STEP_FRAMES: u32 = 5;
    /// Default frames a held key is hidden after the initial step before auto-repeat.
    const KEY_REPEAT_DELAY: u32 = 10;
    /// Default frames an auto-repeat step stays visible while a key stays held.
    const KEY_REPEAT_ON_FRAMES: u32 = 1;
    /// Default frames between auto-repeat steps while a key stays held.
    const KEY_REPEAT_OFF_FRAMES: u32 = 14;

    /// Whether a key held for `elapsed` frames is visible to `GAME_isKeyHold`.
    ///
    /// The key is visible once at the end of the initial step window (the frame
    /// where tile-based games complete the first walk step), hidden during the
    /// repeat delay, then visible again in short auto-repeat pulses so holding
    /// the key keeps producing discrete walk steps.
    fn key_visible_in_frame(
        elapsed: u32,
        step: u32,
        delay: u32,
        repeat_on: u32,
        repeat_off: u32,
    ) -> bool {
        if elapsed == step.saturating_sub(1) {
            return true;
        }
        elapsed
            .checked_sub(step + delay)
            .is_some_and(|frames| frames % (repeat_on + repeat_off) < repeat_on)
    }

    /// Refresh the game-visible held state using feature-phone auto-repeat.
    ///
    /// A fresh press exposes the key to `GAME_isKeyHold` for a short step
    /// window so each press advances exactly one tile.  While the physical key
    /// stays held, further step windows fire after a delay so holding keeps
    /// walking instead of flooding the game with one move per frame.
    fn update_key_state(&mut self) {
        self.key_frame_counter = self.key_frame_counter.wrapping_add(1);
        let counter = self.key_frame_counter;
        let mut visible = 0u32;
        for (key, press_frame) in self.key_press_frame.iter().enumerate() {
            if *press_frame == u32::MAX {
                continue;
            }
            let mask = 1u32 << key;
            let elapsed = counter.wrapping_sub(*press_frame);
            let physically_held = self.key_held_physical & mask != 0;
            // A released key stays visible only while its initial step window
            // is still open, so a very quick tap still completes one step.
            let within_step_grace = physically_held || elapsed < Self::KEY_STEP_FRAMES;
            if within_step_grace
                && Self::key_visible_in_frame(
                    elapsed,
                    Self::KEY_STEP_FRAMES,
                    self.key_repeat_delay,
                    self.key_repeat_on,
                    self.key_repeat_off,
                )
            {
                visible |= mask;
            }
        }
        self.key_held = visible;
    }

    /// Configure held-key auto-repeat (delay before repeating, repeat period).
    pub fn set_key_auto_repeat(&mut self, delay: u32, period: u32) {
        self.key_repeat_delay = delay;
        self.key_repeat_on = 1;
        self.key_repeat_off = period.saturating_sub(1).max(1);
    }

    /// Set a guest key state. Key codes use the platform ABI values (0-20).
    pub fn set_key(&mut self, key: u8, pressed: bool) {
        if key >= 31 {
            return;
        }
        let mask = 1u32 << key;
        if pressed {
            if self.key_held_physical & mask == 0 {
                self.key_down |= mask;
                // Frontends that re-report a held key every frame must not
                // restart the step window, or the guest never observes the
                // completed step.  Only a press after the previous step window
                // started a fresh press.
                let fresh_press = self.key_press_frame[key as usize] == u32::MAX
                    || self
                        .key_frame_counter
                        .wrapping_sub(self.key_press_frame[key as usize])
                        >= Self::KEY_STEP_FRAMES;
                if fresh_press {
                    self.key_press_frame[key as usize] = self.key_frame_counter;
                }
            }
            self.key_held_physical |= mask;
        } else {
            self.key_held_physical &= !mask;
        }
    }

    /// Bitmask of guest keys physically held down (key code as bit index).
    pub fn held_keys(&self) -> u32 {
        self.key_held_physical
    }

    /// Set the playback volume, clamped to 0-100.
    pub fn set_volume(&mut self, volume: u32) {
        self.audio.set_volume(volume);
    }

    /// Set the guest touch/pointer state in screen coordinates.
    pub fn set_pointer(&mut self, x: i32, y: i32, down: bool) {
        self.pointer.set(x, y, down);
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

    /// Pull up to `max_frames` stereo frames of guest audio.
    pub fn take_audio_samples(&mut self, max_frames: usize) -> Vec<i16> {
        self.audio.pull_samples(max_frames)
    }

    /// Deterministic evidence about guest audio playback.
    pub fn audio_diagnostics(&self) -> AudioDiagnostics {
        self.audio.diagnostics()
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
    fn manager_initializers_use_firmware_table_lengths() {
        assert_eq!(manager_initializer_count(0), Some(30));
        assert_eq!(manager_initializer_count(18), Some(115));
        assert_eq!(manager_initializer_count(32), Some(144));
        assert_eq!(manager_initializer_count(45), Some(6));
        assert_eq!(manager_initializer_count(26), None);
    }

    #[test]
    fn game_service_string_readers_use_firmware_length_widths() {
        assert_eq!(game_service_string_uses_wide_length(97), Some(false));
        assert_eq!(game_service_string_uses_wide_length(101), Some(true));
        assert_eq!(game_service_string_uses_wide_length(96), None);
        assert_eq!(game_service_string_uses_wide_length(102), None);
    }

    #[test]
    fn key_hold_auto_repeat_produces_bounded_steps() {
        let (step, delay, on, off) = (5u32, 10u32, 1u32, 14u32);
        let visible: Vec<u32> = (0..45)
            .filter(|elapsed| NicaiMachine::key_visible_in_frame(*elapsed, step, delay, on, off))
            .collect();
        // One visible frame for the initial step, then a quiet delay, then
        // single-frame auto-repeat steps while the key stays held.
        assert_eq!(visible, [4, 15, 30]);
    }

    #[test]
    fn key_hold_auto_repeat_hides_between_steps() {
        let (step, delay, on, off) = (5u32, 10u32, 1u32, 14u32);
        assert!(!NicaiMachine::key_visible_in_frame(3, step, delay, on, off));
        assert!(NicaiMachine::key_visible_in_frame(4, step, delay, on, off));
        assert!(!NicaiMachine::key_visible_in_frame(
            24, step, delay, on, off
        ));
    }

    #[test]
    fn type_three_image_payload_skips_stream_metadata() {
        let mut resource = vec![3, 0, 0, 0, 8, 0, 1, 0, 1];
        resource.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        assert_eq!(image_payload(&resource), b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn checksum_ignores_partial_words() {
        assert_eq!(checksum_le(&[1, 0, 0, 0, 9]), 1);
        assert_eq!(checksum_be(&[0, 0, 0, 1, 9]), 1);
    }

    #[test]
    fn segment_locator_distinguishes_padding_from_leading_fe_data() {
        let padded = [
            0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 1, 0, 0, 0,
        ];
        assert_eq!(
            locate_checked_segment(&padded, 0, 4, 1, None, "test").unwrap(),
            (9, false)
        );

        let leading_fe = [
            0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xb5, 0, 0,
        ];
        let expected = checksum_le(&leading_fe[8..12]);
        assert_eq!(
            locate_checked_segment(&leading_fe, 0, 4, expected, None, "test").unwrap(),
            (8, false)
        );
    }

    #[test]
    fn executable_code_follows_variable_header_and_separator() {
        let variable_header_size = 6usize;
        let segment_header = 68 + variable_header_size;
        let header_end = segment_header + 6 * 12;
        let code = [1u8, 0, 0, 0];
        let initialized = [2u8, 0, 0, 0];
        let embedded = [3u8, 0, 0, 0];
        let mut data = vec![0u8; header_end];

        for index in 0..5 {
            data[index * 12..index * 12 + 8].fill(0xfe);
        }
        data[56..60].copy_from_slice(&(variable_header_size as u32).to_be_bytes());
        for index in 0..6 {
            let offset = segment_header + index * 12;
            data[offset..offset + 8].fill(0xfe);
        }
        for (index, value) in [
            code.len() as u32,
            checksum_le(&code),
            initialized.len() as u32,
            checksum_le(&initialized),
            embedded.len() as u32,
            checksum_le(&embedded),
        ]
        .into_iter()
        .enumerate()
        {
            let offset = segment_header + index * 12 + 8;
            data[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        }

        data.extend_from_slice(&[0xfe; 9]);
        let code_offset = data.len();
        data.extend_from_slice(&code);
        data.extend_from_slice(&[0xfe; 8]);
        let data_offset = data.len();
        data.extend_from_slice(&initialized);
        data.extend_from_slice(&[0xfe; 8]);
        let embedded_offset = data.len();
        data.extend_from_slice(&embedded);
        data.extend_from_slice(&[0xfe; 8]);
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&[0xfe; 8]);
        let resource_package_offset = data.len();

        let executable = CbeExecutable::parse(&data).unwrap();
        assert_eq!(executable.code_offset, code_offset);
        assert_eq!(executable.data_offset, data_offset);
        assert_eq!(executable.embedded_package_offset, embedded_offset);
        assert_eq!(executable.resource_package_offset, resource_package_offset);
        assert!(!executable.big_endian);
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_reaches_audio_services() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = std::path::PathBuf::from(game_dir).join("激情砖块.CBE");
        assert!(game_path.is_file(), "missing {}", game_path.display());

        let archive = CbeArchive::load(&game_path).unwrap();
        let mut machine = NicaiMachine::new(&archive).unwrap();
        machine.boot(5_000_000).unwrap();
        for _ in 0..20 {
            let _ = machine.run_frame(5_000_000);
        }

        assert!(
            machine
                .service_calls()
                .keys()
                .any(|(group, _)| *group == 18),
            "game never called the audio manager"
        );
        let diagnostics = machine.audio_diagnostics();
        assert_eq!(diagnostics.channels, 2);
        assert_eq!(
            diagnostics.sample_rate,
            crate::audio_engine::AUDIO_SAMPLE_RATE
        );
        assert!(
            diagnostics.decoded_frames > 0,
            "guest MIDI audio never reached the engine"
        );
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_uses_timer_services_without_error() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = std::path::PathBuf::from(game_dir).join("魔塔.CBE");
        assert!(game_path.is_file(), "missing {}", game_path.display());

        let archive = CbeArchive::load(&game_path).unwrap();
        let mut machine = NicaiMachine::new(&archive).unwrap();
        machine.boot(5_000_000).unwrap();
        for _ in 0..60 {
            machine.run_frame(5_000_000).unwrap();
        }

        assert_eq!(machine.state(), MachineState::Ready);
        assert!(
            machine.service_calls().get(&(6, 3)).copied().unwrap_or(0) > 0,
            "game never used the timer service"
        );
    }

    #[test]
    fn pointer_press_holds_drag_and_release_edges() {
        let mut pointer = PointerState::new();
        assert!(!pointer.held && !pointer.down && !pointer.up);

        pointer.set(120, 200, true);
        assert!(pointer.held);
        assert!(pointer.down);
        assert!(!pointer.up);
        assert!(!pointer.dragging());

        pointer.end_frame();
        assert!(!pointer.down);

        pointer.set(140, 210, true);
        assert!(!pointer.down);
        assert!(pointer.dragging());

        pointer.set(140, 210, false);
        assert!(!pointer.held);
        assert!(pointer.up);
    }

    #[test]
    fn pointer_coordinates_are_clamped_to_the_screen() {
        let mut pointer = PointerState::new();
        pointer.set(-50, 500, true);
        assert_eq!((pointer.x, pointer.y), (0, 399));
        pointer.set(1000, -10, true);
        assert_eq!((pointer.x, pointer.y), (239, 0));
    }
}
