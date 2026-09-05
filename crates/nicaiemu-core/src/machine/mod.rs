//! CBE machine root: executable parsing, boot, frame loop, and input state.
//!
//! The machine is split across submodules by responsibility:
//! - [`memory`]: sparse guest memory regions and endian-aware access
//! - [`packages`]: native/flat/grouped guest resource package parsing
//! - [`cpu_bridge`]: execution loop, interworking branches, service dispatch
//! - [`drawing`]: framebuffer drawing, blits, rects, and text
//! - [`services`]: firmware service handlers grouped by manager

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use anyhow::{bail, Context, Result};
use armv4t_emu::{reg, Cpu, Memory, Mode};
use log::{debug, warn};
use serde::{Deserialize, Serialize};

use crate::audio_engine::{AudioDiagnostics, AudioEngine};
use crate::cbe::CbeArchive;

mod cpu_bridge;
mod drawing;
mod memory;
mod packages;
mod services;
mod virtual_fs;

use memory::MachineMemory;
pub use memory::MemoryRegionInfo;
use packages::{
    grouped_package_resources, named_package_resources, native_package_resources, HostResource,
    HostResourcePackage,
};
use virtual_fs::VirtualFileSystem;

/// Native guest framebuffer width in pixels.
pub const FRAME_WIDTH: u32 = 240;
/// Native guest framebuffer height in pixels.
pub const FRAME_HEIGHT: u32 = 400;

/// Display rotation applied to the guest framebuffer before presentation.
///
/// Some games render a landscape (400x240) layout into the portrait
/// 240x400 framebuffer, matching the LCD rotation used by the original
/// phone hardware. The emulator presents the raw framebuffer, so those
/// games need a 90-degree rotation to display upright.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rotation {
    /// Resolve automatically from the guest's rendering (default).
    #[default]
    Auto,
    /// Present the framebuffer as-is (portrait 240x400).
    None,
    /// Rotate the framebuffer 90 degrees clockwise (400x240 output).
    Cw,
    /// Rotate the framebuffer 90 degrees counterclockwise (400x240 output).
    Ccw,
}

impl Rotation {
    /// Whether this rotation changes the output dimensions.
    pub fn swaps_dimensions(self) -> bool {
        matches!(self, Rotation::Cw | Rotation::Ccw)
    }

    /// Map a presented display coordinate back to a guest framebuffer
    /// coordinate, used to translate pointer input on rotated output.
    pub fn unrotate(self, display_x: i32, display_y: i32) -> (i32, i32) {
        match self {
            Rotation::Auto | Rotation::None => (display_x, display_y),
            Rotation::Cw => (display_y, (FRAME_HEIGHT - 1) as i32 - display_x),
            Rotation::Ccw => ((FRAME_WIDTH - 1) as i32 - display_y, display_x),
        }
    }
}

/// Content-identity rotation profile for the local CBE corpus.
///
/// These games draw landscape (400x240) art pre-rotated into the portrait
/// 240x400 framebuffer and rely on the original phone's rotated LCD output,
/// so the emulator must present the raw framebuffer rotated 90 degrees
/// counterclockwise. Keying by archive CRC-32 plus byte length instead of the
/// file name keeps the profile valid across renames.
const LANDSCAPE_ROTATION_PROFILE: &[(u32, u64)] = &[
    (0xEE5A53AC, 341737),  // 暴力摩托
    (0x7A5C0A30, 728876),  // 捕鱼猎人
    (0x50528857, 961146),  // 法老祖玛2
    (0x9C5E0674, 958874),  // 愤怒的小鸟
    (0x52DAD535, 611925),  // 疯狂捕鸟
    (0xF3283516, 606493),  // 疯狂斗地主
    (0x7BCDA1EB, 396952),  // 疯狂企鹅大冒险
    (0x4A849388, 910806),  // 机场指挥部
    (0x701C7D4B, 539016),  // 僵尸先生
    (0x5F320C34, 1413319), // 开心大富翁
    (0x8EDDE44F, 1292332), // 美女桌球
    (0x282FE73D, 1143317), // 三国群殴传
    (0xC6488351, 400101),  // 士兵突袭
    (0xBC3CD75C, 734986),  // 水果达人
    (0x2CB6103B, 1074317), // 吸血鬼猎人
    (0x145C46B4, 1016330), // 小鸟愤怒冬季版
    (0x5E8B5904, 319424),  // 幸运扑克机
];

/// Resolve the automatic rotation from the content-identity profile.
pub fn rotation_for_archive(bytes: &[u8]) -> Rotation {
    let crc = crc32fast::hash(bytes);
    let length = bytes.len() as u64;
    if LANDSCAPE_ROTATION_PROFILE
        .iter()
        .any(|&(expected_crc, expected_length)| crc == expected_crc && length == expected_length)
    {
        Rotation::Ccw
    } else {
        Rotation::None
    }
}

/// Rotate a row-major pixel buffer 90 degrees for presentation.
///
/// The output keeps the same pixel count but swaps dimensions: a `width` x
/// `height` source becomes `height` x `width`. Clockwise rotation maps source
/// (x, y) to output (height - 1 - y, x); counterclockwise maps it to
/// (y, width - 1 - x).
pub(crate) fn rotate_frame(
    pixels: &[u32],
    width: u32,
    height: u32,
    rotation: Rotation,
) -> Vec<u32> {
    match rotation {
        Rotation::Auto | Rotation::None => pixels.to_vec(),
        Rotation::Cw => {
            let mut rotated = Vec::with_capacity(pixels.len());
            for y in 0..width {
                for x in 0..height {
                    rotated.push(pixels[((height - 1 - x) * width + y) as usize]);
                }
            }
            rotated
        }
        Rotation::Ccw => {
            let mut rotated = Vec::with_capacity(pixels.len());
            for y in 0..width {
                for x in 0..height {
                    rotated.push(pixels[(x * width + (width - 1 - y)) as usize]);
                }
            }
            rotated
        }
    }
}

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
const DREAM_FACTORY_FORMAT_BUFFER: u32 = MANAGER_BASE + 0x7f80;
const DREAM_FACTORY_FORMAT_BUFFER_SIZE: usize = 64;
const KEY_EVENT_ARG: u32 = MANAGER_BASE + 0x7fdc;
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

#[derive(Debug, PartialEq, Eq)]
enum VariadicArgument {
    Register(u8),
    Stack(u32),
}

fn variadic_argument_location(first_register: u32, argument_index: u32) -> VariadicArgument {
    let register_index = first_register + argument_index;
    if register_index <= 3 {
        VariadicArgument::Register(register_index as u8)
    } else {
        VariadicArgument::Stack((register_index - 4) * 4)
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
    data.as_chunks::<4>().0.iter().fold(0u32, |sum, bytes| {
        sum.wrapping_add(u32::from_le_bytes(*bytes))
    })
}

fn checksum_be(data: &[u8]) -> u32 {
    data.as_chunks::<4>().0.iter().fold(0u32, |sum, bytes| {
        sum.wrapping_add(u32::from_be_bytes(*bytes))
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

fn update_key_bits(key_down: &mut u32, key_held: &mut u32, key: u8, pressed: bool) {
    if key >= 31 {
        return;
    }
    let mask = 1u32 << key;
    if pressed {
        if *key_held & mask == 0 {
            *key_down |= mask;
        }
        *key_held |= mask;
    } else {
        *key_held &= !mask;
    }
}

fn update_physical_key_bits(key_held: &mut u32, key: u8, pressed: bool) -> bool {
    if key >= 31 {
        return false;
    }
    let mask = 1u32 << key;
    let was_pressed = *key_held & mask != 0;
    if pressed {
        *key_held |= mask;
    } else {
        *key_held &= !mask;
    }
    was_pressed != pressed
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyEvent {
    key: u8,
    pressed: bool,
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
    #[serde(skip, default)]
    heap_allocations: BTreeMap<u32, u32>,
    #[serde(skip, default)]
    free_heap_blocks: Vec<(u32, u32)>,
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
    // Retained to preserve the version-1 save-state field layout.
    _legacy_key_press_frame: [u32; 31],
    _legacy_key_frame_counter: u32,
    #[serde(skip, default)]
    pending_key_events: VecDeque<KeyEvent>,
    // Auto-BGM compatibility layer: plays the first packaged MIDI when the
    // guest never touches the audio manager. Deliberately excluded from save
    // states; frontends re-apply it after load/reset.
    #[serde(skip, default)]
    auto_bgm: bool,
    #[serde(skip, default)]
    auto_bgm_data: Option<Vec<u8>>,
    #[serde(skip, default)]
    auto_bgm_gave_way: bool,
    // Display rotation is a frontend presentation concern rather than guest
    // state; frontends re-apply it after load/reset.
    #[serde(skip, default)]
    rotation: Rotation,
    /// Resolved rotation after automatic landscape detection.
    #[serde(skip, default)]
    effective_rotation: Rotation,
    pointer: PointerState,
    timers: Vec<GuestTimer>,
    resources: Vec<HostResource>,
    #[serde(skip, default)]
    resource_packages: Vec<HostResourcePackage>,
    resource_data: Vec<u32>,
    resource_names: Vec<u32>,
    app_image_package: u32,
    inner_image_package: u32,
    current_image_package: u32,
    native_app_parser: u32,
    native_app_init: u32,
    native_system_info: u32,
    native_property_info: u32,
    #[serde(skip, default)]
    virtual_fs: VirtualFileSystem,
}

impl std::fmt::Debug for NicaiMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NicaiMachine")
            .field("executable", &self.executable)
            .field("state", &self.state)
            .field("app_main", &format_args!("0x{:08X}", self.app_main))
            .field("instruction_count", &self.instruction_count)
            .field("resource_package_count", &self.resource_packages.len())
            .field("virtual_file_count", &self.virtual_fs.file_count())
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
            heap_allocations: BTreeMap::new(),
            free_heap_blocks: Vec::new(),
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
            _legacy_key_press_frame: [u32::MAX; 31],
            _legacy_key_frame_counter: 0,
            pending_key_events: VecDeque::new(),
            auto_bgm: false,
            auto_bgm_data: None,
            auto_bgm_gave_way: false,
            rotation: Rotation::Auto,
            effective_rotation: Rotation::None,
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
            resource_packages: named_package_resources(
                archive.bytes(),
                resource_package_offset,
                resource_package_size,
            ),
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
            virtual_fs: VirtualFileSystem::default(),
        };
        machine.initialize_tables();
        machine.initialize_screen();
        // Resolve the default Auto rotation from the content-identity profile.
        machine.effective_rotation = rotation_for_archive(archive.bytes());
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
        if self.state == MachineState::Halted {
            return Ok(());
        }
        if self.uses_native_dispatch_abi() {
            if self.native_app_parser == 0 {
                self.state = MachineState::Faulted;
                bail!("CBE initializer returned without registering a native application entry");
            }
            self.app_main = self.native_app_parser;
            self.invoke_callback(self.native_app_parser, 0, 0, 0, instruction_limit)?;
            if self.state == MachineState::Halted {
                return Ok(());
            }
            self.invoke_callback(self.native_app_init, 0, 0, 0, instruction_limit)?;
            if self.state == MachineState::Halted {
                return Ok(());
            }
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
        if self.state == MachineState::Halted {
            return Ok(());
        }
        self.state = MachineState::Ready;
        Ok(())
    }

    /// Rebuild the machine from an archive and re-run the boot sequence.
    ///
    /// Frontends use this for reset so a game restarts from a clean runtime
    /// state without reloading the file from disk.
    pub fn reset(&mut self, archive: &CbeArchive, instruction_limit: u64) -> Result<()> {
        let volume = self.audio.volume();
        let auto_bgm = self.auto_bgm;
        let mut rebuilt = NicaiMachine::new(archive)?;
        rebuilt.audio.set_volume(volume);
        rebuilt.auto_bgm = auto_bgm;
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

    fn allocate(&mut self, size: u32) -> u32 {
        if size == 0 {
            return 0;
        }
        let aligned = size.saturating_add(7) & !7;
        if let Some(index) = self
            .free_heap_blocks
            .iter()
            .position(|(_, available)| *available >= aligned)
        {
            let (pointer, available) = self.free_heap_blocks.remove(index);
            if available > aligned {
                self.free_heap_blocks
                    .insert(index, (pointer + aligned, available - aligned));
            }
            self.heap_allocations.insert(pointer, aligned);
            return pointer;
        }
        let pointer = self.heap_cursor;
        let end = pointer.saturating_add(aligned);
        if end > MEMORY_BLOCK_POOL {
            warn!("CBE heap exhausted while allocating {size} bytes");
            0
        } else {
            self.heap_cursor = end;
            self.heap_allocations.insert(pointer, aligned);
            pointer
        }
    }

    fn deallocate(&mut self, pointer: u32) {
        let Some(size) = self.heap_allocations.remove(&pointer) else {
            return;
        };
        self.free_heap_blocks.push((pointer, size));
        self.free_heap_blocks.sort_unstable_by_key(|block| block.0);

        let mut index = 0;
        while index + 1 < self.free_heap_blocks.len() {
            let (start, length) = self.free_heap_blocks[index];
            let (next_start, next_length) = self.free_heap_blocks[index + 1];
            if start.saturating_add(length) == next_start {
                self.free_heap_blocks[index].1 = length.saturating_add(next_length);
                self.free_heap_blocks.remove(index + 1);
            } else {
                index += 1;
            }
        }
    }

    fn allocation_size(&self, address: u32) -> Option<u32> {
        let (&start, &size) = self.heap_allocations.range(..=address).next_back()?;
        let offset = address.checked_sub(start)?;
        (offset < size).then_some(size - offset)
    }

    /// Execute one screen update and render pass.
    pub fn run_frame(&mut self, instruction_limit: u64) -> Result<()> {
        if self.state == MachineState::Halted {
            self.key_down = 0;
            self.pointer.end_frame();
            return Ok(());
        }
        if self.state != MachineState::Ready {
            bail!("CBE machine is not ready");
        }
        self.maybe_run_auto_bgm();
        self.frame_count = self.frame_count.wrapping_add(1);
        let had_screen_before_timers =
            self.active_screen != 0 || self.pending_screen != 0 || !self.screen_stack.is_empty();
        self.dispatch_timers(instruction_limit)?;
        if self.finish_halted_frame() {
            return Ok(());
        }
        if had_screen_before_timers && self.finish_screen_callback_frame() {
            return Ok(());
        }
        if self.uses_native_dispatch_abi() {
            self.key_down = 0;
            if let Some(event) = self.pending_key_events.pop_front() {
                update_key_bits(
                    &mut self.key_down,
                    &mut self.key_held,
                    event.key,
                    event.pressed,
                );
            }
            let result = self.invoke_callback(self.native_app_parser, 0, 0, 0, instruction_limit);
            self.key_down = 0;
            return result;
        }
        if self.pending_screen != 0 && self.pending_screen != self.active_screen {
            self.active_screen = self.pending_screen;
            self.screen_initialized = false;
        }
        if self.active_screen == 0 {
            if self.timers.iter().any(|timer| timer.active) {
                self.key_down = 0;
                self.pointer.end_frame();
                return Ok(());
            }
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
            if self.finish_screen_callback_frame() {
                return Ok(());
            }
            self.screen_initialized = true;
        }
        if self.resource_load_pending
            && (self.resource_load_screen == 0 || self.resource_load_screen == screen)
        {
            self.resource_load_pending = false;
            self.resource_load_screen = 0;
            let load_resource = self.memory.r32(screen + 24);
            self.invoke_callback(load_resource, screen_this, 0, 0, instruction_limit)?;
            if self.finish_screen_callback_frame() {
                return Ok(());
            }
        }
        if self.pending_screen != 0 && self.pending_screen != screen {
            self.key_down = 0;
            self.pointer.end_frame();
            return Ok(());
        }

        self.key_down = 0;
        let key_event = self.pending_key_events.pop_front();
        if let Some(event) = key_event {
            update_key_bits(
                &mut self.key_down,
                &mut self.key_held,
                event.key,
                event.pressed,
            );
        }

        let logic = self.memory.r32(screen + 8);
        if let Some(event) = key_event {
            self.memory.w32(KEY_EVENT_ARG, 1u32 << event.key);
            self.invoke_callback(
                logic,
                screen_this,
                u32::from(!event.pressed),
                KEY_EVENT_ARG,
                instruction_limit,
            )?;
            if self.finish_screen_callback_frame() {
                return Ok(());
            }
            self.key_down = 0;
            if self.pending_screen != 0 && self.pending_screen != screen {
                self.pointer.end_frame();
                return Ok(());
            }
        }
        self.invoke_callback(logic, screen_this, 6, 0, instruction_limit)?;
        if self.finish_screen_callback_frame() {
            return Ok(());
        }
        if self.pending_screen == 0 || self.pending_screen == screen {
            let render = self.memory.r32(screen + 12);
            if render == 0 {
                bail!("CBE screen at 0x{screen:08X} has no render callback");
            }
            self.invoke_callback(render, screen_this, 0, 0, instruction_limit)?;
            if self.finish_screen_callback_frame() {
                return Ok(());
            }
        }
        self.key_down = 0;
        self.pointer.end_frame();
        Ok(())
    }

    fn finish_halted_frame(&mut self) -> bool {
        if self.state != MachineState::Halted {
            return false;
        }
        self.key_down = 0;
        self.pointer.end_frame();
        true
    }

    fn finish_screen_callback_frame(&mut self) -> bool {
        if self.state != MachineState::Halted
            && self.active_screen == 0
            && self.pending_screen == 0
            && self.screen_stack.is_empty()
        {
            self.state = MachineState::Halted;
        }
        self.finish_halted_frame()
    }

    fn screen_call_parameter(&self, screen: u32) -> u32 {
        if screen >= self.executable.data_address() {
            screen.saturating_sub(0x18)
        } else {
            screen
        }
    }

    /// Play the first packaged MIDI resource as background music when the
    /// guest never touches the audio manager.
    ///
    /// Some games (for example the local 魔塔 build) ship `.mid` resources but
    /// never call any audio-manager service, so the packaged soundtrack stays
    /// silent on real hardware-adjacent emulation. This compatibility layer
    /// restarts the first MIDI once the previous pass is consumed, and hands
    /// audio back to the game as soon as the guest issues its own audio call.
    fn maybe_run_auto_bgm(&mut self) {
        if !self.auto_bgm || self.auto_bgm_gave_way {
            return;
        }
        // State 1 (playing) with an empty queue means the previous pass was
        // fully consumed; state 0 means nothing was ever queued. In both cases
        // (re)start the soundtrack.
        if self.audio.state() == 1 && self.audio.buffered_frames() != 0 {
            return;
        }
        if self.auto_bgm_data.is_none() {
            self.auto_bgm_data = self
                .resources
                .iter()
                .find(|resource| {
                    let name = resource.name.to_ascii_lowercase();
                    name.ends_with(".mid") || name.ends_with(".midi")
                })
                .map(|resource| resource.data.clone());
        }
        let Some(data) = self.auto_bgm_data.clone() else {
            return;
        };
        match self.audio.play_bytes(&data) {
            Ok(()) => log::debug!("Auto BGM started ({} bytes)", data.len()),
            Err(error) => log::warn!("Auto BGM rejected: {error:#}"),
        }
    }

    /// Enable or disable the packaged-MIDI auto-BGM compatibility layer.
    ///
    /// The guest audio manager always wins: the layer stops permanently as
    /// soon as the game issues any audio-manager call of its own.
    pub fn set_auto_bgm(&mut self, enabled: bool) {
        self.auto_bgm = enabled;
        if enabled {
            // Re-enabling the layer also lets it take over again after the
            // guest previously issued its own audio-manager calls.
            self.auto_bgm_gave_way = false;
        } else {
            self.audio.stop();
        }
    }

    /// Whether the packaged-MIDI auto-BGM compatibility layer is enabled.
    pub fn auto_bgm(&self) -> bool {
        self.auto_bgm
    }

    /// Set a guest key state. Key codes use the platform ABI values (0-20).
    pub fn set_key(&mut self, key: u8, pressed: bool) {
        if update_physical_key_bits(&mut self.key_held_physical, key, pressed) {
            self.pending_key_events.push_back(KeyEvent { key, pressed });
        }
    }

    /// Bitmask of guest keys physically held down (key code as bit index).
    pub fn held_keys(&self) -> u32 {
        self.key_held_physical
    }

    pub(crate) fn normalize_input_after_load(&mut self) {
        self.key_down = 0;
        self.key_held = self.key_held_physical;
        self.pending_key_events.clear();
    }

    /// Set the playback volume, clamped to 0-100.
    pub fn set_volume(&mut self, volume: u32) {
        self.audio.set_volume(volume);
    }

    /// Set the guest touch/pointer state in screen coordinates.
    pub fn set_pointer(&mut self, x: i32, y: i32, down: bool) {
        self.pointer.set(x, y, down);
    }

    /// Set the display rotation applied by [`NicaiMachine::frame_pixels`].
    pub fn set_rotation(&mut self, rotation: Rotation) {
        self.rotation = rotation;
        if rotation != Rotation::Auto {
            self.effective_rotation = rotation;
        }
    }

    /// Re-resolve the automatic display rotation from the content-identity
    /// profile.
    ///
    /// Only a requested `Auto` mode consults the profile; explicit `None`/`Cw`/
    /// `Ccw` overrides win. Presentation state is skipped by the save-state
    /// codec, so frontends call this after restoring a saved machine or
    /// applying a frontend rotation setting.
    pub fn resolve_auto_rotation(&mut self, archive: &CbeArchive) {
        if self.rotation == Rotation::Auto {
            self.effective_rotation = rotation_for_archive(archive.bytes());
        }
    }

    /// The display rotation applied by [`NicaiMachine::frame_pixels`].
    pub fn rotation(&self) -> Rotation {
        self.rotation
    }

    /// The resolved rotation currently applied to output.
    pub fn effective_rotation(&self) -> Rotation {
        self.effective_rotation
    }

    /// Presented output size in pixels after rotation.
    pub fn display_size(&self) -> (u32, u32) {
        if self.effective_rotation.swaps_dimensions() {
            (FRAME_HEIGHT, FRAME_WIDTH)
        } else {
            (FRAME_WIDTH, FRAME_HEIGHT)
        }
    }

    /// Map a presented display coordinate back to guest framebuffer space.
    pub fn display_to_framebuffer(&self, display_x: i32, display_y: i32) -> (i32, i32) {
        self.effective_rotation.unrotate(display_x, display_y)
    }

    /// Copy the current RGB565 framebuffer into 0x00RRGGBB pixels, applying
    /// the configured display rotation.
    pub fn frame_pixels(&mut self) -> Vec<u32> {
        let mut pixels = Vec::with_capacity(240 * 400);
        for index in 0..(240 * 400) as u32 {
            let color = self.memory.r16(SCREEN_IMAGE + index * 2);
            let red = ((color >> 11) & 0x1f) as u32;
            let green = ((color >> 5) & 0x3f) as u32;
            let blue = (color & 0x1f) as u32;
            pixels.push(((red * 255 / 31) << 16) | ((green * 255 / 63) << 8) | (blue * 255 / 31));
        }
        rotate_frame(&pixels, FRAME_WIDTH, FRAME_HEIGHT, self.effective_rotation)
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
    fn rotate_frame_swaps_dimensions_and_keeps_orientation() {
        // 2x3 source: rows map to columns when rotated.
        // Layout: (0,0)=1 (1,0)=2 / (0,1)=3 (1,1)=4 / (0,2)=5 (1,2)=6.
        let source = vec![1, 2, 3, 4, 5, 6];

        // Clockwise: source (x, y) -> output (height - 1 - y, x).
        let cw = rotate_frame(&source, 2, 3, Rotation::Cw);
        assert_eq!(cw.len(), 6);
        assert_eq!(cw, vec![5, 3, 1, 6, 4, 2]);

        // Counterclockwise: source (x, y) -> output (y, width - 1 - x).
        let ccw = rotate_frame(&source, 2, 3, Rotation::Ccw);
        assert_eq!(ccw.len(), 6);
        assert_eq!(ccw, vec![2, 4, 6, 1, 3, 5]);

        // No rotation returns an untouched copy.
        assert_eq!(rotate_frame(&source, 2, 3, Rotation::None), source);
    }

    #[test]
    fn rotation_unrotate_maps_display_back_to_framebuffer() {
        assert_eq!(Rotation::None.unrotate(10, 20), (10, 20));
        // Clockwise output pixel (x, y) came from framebuffer (y, 399 - x).
        assert_eq!(Rotation::Cw.unrotate(10, 20), (20, 389));
        assert_eq!(Rotation::Cw.unrotate(399, 239), (239, 0));
        // Counterclockwise output pixel (x, y) came from framebuffer
        // (239 - y, x).
        assert_eq!(Rotation::Ccw.unrotate(10, 20), (219, 10));
        assert_eq!(Rotation::Ccw.unrotate(399, 239), (0, 399));
    }

    #[test]
    fn rotation_profile_rejects_unknown_content() {
        assert_eq!(
            rotation_for_archive(b"arbitrary game bytes"),
            Rotation::None
        );
        assert_eq!(rotation_for_archive(b""), Rotation::None);
        // The profile lookup is content-keyed: a different length with the
        // same CRC must not match.
        assert_ne!(rotation_for_archive(&[0; 341737]), Rotation::Ccw);
    }

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
    fn variadic_arguments_continue_from_registers_to_stack() {
        assert_eq!(
            variadic_argument_location(1, 0),
            VariadicArgument::Register(1)
        );
        assert_eq!(
            variadic_argument_location(1, 2),
            VariadicArgument::Register(3)
        );
        assert_eq!(variadic_argument_location(1, 3), VariadicArgument::Stack(0));
        assert_eq!(variadic_argument_location(2, 2), VariadicArgument::Stack(0));
    }

    #[test]
    fn held_keys_remain_visible_without_repeating_down_edges() {
        let mut down = 0;
        let mut held = 0;
        let mut physical = 0;

        assert!(update_physical_key_bits(&mut physical, 16, true));
        update_key_bits(&mut down, &mut held, 16, true);
        assert_eq!(down, 1 << 16);
        assert_eq!(held, 1 << 16);

        down = 0;
        assert!(!update_physical_key_bits(&mut physical, 16, true));
        assert_eq!(down, 0);
        assert_eq!(held, 1 << 16);

        assert!(update_physical_key_bits(&mut physical, 16, false));
        update_key_bits(&mut down, &mut held, 16, false);
        assert_eq!(physical, 0);
        assert_eq!(held, 0);
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
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_loads_resources_after_legacy_screen_transition() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        for game in ["暴力摩托.CBE", "恶魔城.CBE", "雷霆战机.CBE"] {
            let game_path = std::path::PathBuf::from(&game_dir).join(game);
            assert!(game_path.is_file(), "missing {}", game_path.display());

            let archive = CbeArchive::load(&game_path).unwrap();
            let mut machine = NicaiMachine::new(&archive).unwrap();
            machine.boot(5_000_000).unwrap();
            for _ in 0..2 {
                machine.run_frame(5_000_000).unwrap();
            }

            assert_eq!(machine.state(), MachineState::Ready, "{game} faulted");
        }
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_shooter_advances_projectiles_and_registers_hits() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = std::path::PathBuf::from(game_dir).join("雷霆战机.CBE");
        assert!(game_path.is_file(), "missing {}", game_path.display());

        let archive = CbeArchive::load(&game_path).unwrap();
        let mut machine = NicaiMachine::new(&archive).unwrap();
        machine.boot(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
        for frame in 0..1_200 {
            let confirm = matches!(frame, 5 | 50 | 100 | 150);
            if confirm {
                machine.set_key(14, true);
            }
            machine.run_frame(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
            if confirm {
                machine.set_key(14, false);
            }
        }

        assert_eq!(machine.state(), MachineState::Ready);
        for index in 103..=106 {
            assert!(
                machine
                    .service_calls()
                    .get(&(3, index))
                    .copied()
                    .unwrap_or(0)
                    > 0,
                "game never called required game-math service {index}"
            );
        }
        let score = machine
            .memory
            .r32(machine.executable.data_address() + 0x396c);
        assert!(score > 0, "projectiles never registered a hit");
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_leidian_attack_flow_stays_ready_with_default_budget() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = std::path::PathBuf::from(game_dir).join("雷电.CBE");
        assert!(game_path.is_file(), "missing {}", game_path.display());

        let archive = CbeArchive::load(&game_path).unwrap();
        let mut machine = NicaiMachine::new(&archive).unwrap();
        machine.boot(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
        // Navigate: menu -> flight select -> confirm popup -> sortie ->
        // second confirm popup -> sortie. The second sortie starts the stage
        // intro, whose single heavy render frame previously exceeded the
        // default callback budget and aborted the machine.
        let taps = [
            (30, 0, 0, true),      // key 14 opens the flight-select screen.
            (60, 30, 375, false),  // Tap the bottom-left confirm checkmark.
            (90, 38, 214, false),  // Tap the sortie button in the popup.
            (150, 30, 375, false), // Tap the checkmark again.
            (180, 38, 214, false), // Tap sortie to start the stage.
        ];
        for frame in 0..300 {
            let event = taps
                .iter()
                .find(|&&(event_frame, _, _, _)| event_frame == frame)
                .copied();
            if let Some((_, x, y, key_press)) = event {
                if key_press {
                    machine.set_key(14, true);
                } else {
                    machine.set_pointer(x, y, true);
                }
            }
            machine.run_frame(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
            if let Some((_, x, y, key_press)) = event {
                if key_press {
                    machine.set_key(14, false);
                } else {
                    machine.set_pointer(x, y, false);
                }
            }
        }

        assert_eq!(machine.state(), MachineState::Ready);
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_completes_high_cost_idle_frames_with_default_budget() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        for (game, frames) in [
            ("疯狂捕鸟.CBE", 1),
            ("疯狂企鹅大冒险.CBE", 28),
            ("僵尸先生.CBE", 1),
        ] {
            let game_path = std::path::PathBuf::from(&game_dir).join(game);
            assert!(game_path.is_file(), "missing {}", game_path.display());

            let archive = CbeArchive::load(&game_path).unwrap();
            let mut machine = NicaiMachine::new(&archive).unwrap();
            machine.boot(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
            for _ in 0..frames {
                machine.run_frame(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
            }

            assert_eq!(machine.state(), MachineState::Ready, "{game} faulted");
        }
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_installs_and_enters_zombie_game() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = std::path::PathBuf::from(game_dir).join("僵尸先生.CBE");
        assert!(game_path.is_file(), "missing {}", game_path.display());

        let archive = CbeArchive::load(&game_path).unwrap();
        let mut machine = NicaiMachine::new(&archive).unwrap();
        machine.boot(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
        machine.run_frame(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
        let installer_screen = machine.active_screen();
        for _ in 0..330 {
            machine.run_frame(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
        }
        assert_eq!(
            machine
                .virtual_fs
                .file("PlantsZombies/bwandou_zidan.actor")
                .map(<[u8]>::len),
            Some(324),
            "installer wrote a resource with the wrong boundary"
        );
        for _ in 0..50 {
            machine.run_frame(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
        }

        assert_eq!(machine.state(), MachineState::Ready);
        assert_ne!(machine.active_screen(), installer_screen);
        assert!(machine
            .virtual_fs
            .file("PlantsZombies/title1.gif")
            .is_some());
        assert!(machine.virtual_fs.file("PlantsZombies/花园1.map").is_some());
        assert!(machine.virtual_fs.paths().len() > 300);
        assert!(
            machine
                .frame_pixels()
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len()
                > 3,
            "game screen did not render its resources"
        );
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_waits_for_timer_driven_first_screen() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = std::path::PathBuf::from(game_dir).join("魔鬼理发师.CBE");
        assert!(game_path.is_file(), "missing {}", game_path.display());

        let archive = CbeArchive::load(&game_path).unwrap();
        let mut machine = NicaiMachine::new(&archive).unwrap();
        machine.boot(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
        for _ in 0..8 {
            machine.run_frame(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
        }

        assert_eq!(machine.state(), MachineState::Ready);
        assert_ne!(machine.active_screen(), 0);
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_fixed_address_game_populates_standard_runtime_services() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = std::path::PathBuf::from(game_dir).join("武林外传(新品).CBE");
        assert!(game_path.is_file(), "missing {}", game_path.display());

        let archive = CbeArchive::load(&game_path).unwrap();
        let mut machine = NicaiMachine::new(&archive).unwrap();
        machine.boot(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
        machine.run_frame(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();

        assert_eq!(machine.state(), MachineState::Ready);
        let system_info = machine
            .memory
            .r32(machine.executable.data_address() + 0x1724);
        assert_eq!(
            machine.memory.r32(system_info + 0x220),
            SERVICE_BASE + TABLE_STRIDE * 6 + 5 * 4
        );
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_terminal_loop_halts_without_frame_errors() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = std::path::PathBuf::from(game_dir).join("武林外传(新品).CBE");
        assert!(game_path.is_file(), "missing {}", game_path.display());

        let archive = CbeArchive::load(&game_path).unwrap();
        let mut machine = NicaiMachine::new(&archive).unwrap();
        machine.boot(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
        for _ in 0..400 {
            machine.run_frame(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
        }

        assert_eq!(machine.state(), MachineState::Halted);
        assert!(
            machine.service_calls().get(&(6, 5)).copied().unwrap_or(0) > 0,
            "game never called the random-number service"
        );
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_removing_last_screen_halts_without_frame_errors() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = std::path::PathBuf::from(game_dir).join("在线书城.CBE");
        assert!(game_path.is_file(), "missing {}", game_path.display());

        let archive = CbeArchive::load(&game_path).unwrap();
        let mut machine = NicaiMachine::new(&archive).unwrap();
        machine.boot(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
        for _ in 0..400 {
            machine.run_frame(crate::DEFAULT_INSTRUCTION_LIMIT).unwrap();
        }

        assert_eq!(machine.state(), MachineState::Halted);
        assert!(
            machine.service_calls().get(&(14, 6)).copied().unwrap_or(0) > 0,
            "game never removed its final screen"
        );
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_auto_bgm_plays_packaged_midi() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = std::path::PathBuf::from(game_dir).join("魔塔.CBE");
        assert!(game_path.is_file(), "missing {}", game_path.display());

        let archive = CbeArchive::load(&game_path).unwrap();
        let mut machine = NicaiMachine::new(&archive).unwrap();
        machine.boot(5_000_000).unwrap();
        machine.set_auto_bgm(true);
        for _ in 0..3 {
            machine.run_frame(5_000_000).unwrap();
        }

        let diagnostics = machine.audio_diagnostics();
        assert!(
            diagnostics.decoded_frames > 0,
            "packaged MIDI never reached the engine via auto BGM"
        );
        assert!(
            diagnostics.nonzero_samples > 0,
            "auto BGM produced only silence"
        );

        // The guest never calls the audio manager, so auto BGM must not have
        // given way to the game.
        assert!(
            !machine
                .service_calls()
                .keys()
                .any(|(group, _)| *group == 18),
            "guest unexpectedly used the audio manager"
        );
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_opens_island_help_without_error() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = std::path::PathBuf::from(game_dir).join("孤岛.CBE");
        assert!(game_path.is_file(), "missing {}", game_path.display());

        let archive = CbeArchive::load(&game_path).unwrap();
        let mut machine = NicaiMachine::new(&archive).unwrap();
        machine.boot(5_000_000).unwrap();
        for frame in 0..60 {
            let key = match frame {
                20 | 22 => Some(18),
                24 => Some(14),
                _ => None,
            };
            if let Some(key) = key {
                machine.set_key(key, true);
            }
            machine.run_frame(5_000_000).unwrap();
            if let Some(key) = key {
                machine.set_key(key, false);
            }
        }

        assert_eq!(machine.state(), MachineState::Ready);
        assert!(
            machine
                .frame_pixels()
                .windows(2)
                .any(|pixels| pixels[0] != pixels[1]),
            "help screen remained blank"
        );
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_pointer_opens_same_island_help_screen_as_keys() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = std::path::PathBuf::from(game_dir).join("孤岛.CBE");
        assert!(game_path.is_file(), "missing {}", game_path.display());

        let archive = CbeArchive::load(&game_path).unwrap();
        let mut keyed = NicaiMachine::new(&archive).unwrap();
        keyed.boot(5_000_000).unwrap();
        for frame in 0..60 {
            let key = match frame {
                20 | 22 => Some(18),
                24 => Some(14),
                _ => None,
            };
            if let Some(key) = key {
                keyed.set_key(key, true);
            }
            keyed.run_frame(5_000_000).unwrap();
            if let Some(key) = key {
                keyed.set_key(key, false);
            }
        }

        let mut pointed = NicaiMachine::new(&archive).unwrap();
        pointed.boot(5_000_000).unwrap();
        for frame in 0..60 {
            if matches!(frame, 20 | 22) {
                pointed.set_pointer(120, 243, true);
            }
            pointed.run_frame(5_000_000).unwrap();
            if matches!(frame, 20 | 22) {
                pointed.set_pointer(120, 243, false);
            }
        }

        assert_eq!(pointed.state(), MachineState::Ready);
        assert_eq!(pointed.frame_pixels(), keyed.frame_pixels());
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

    /// A minimal MIDI resource in the packaged CBE audio format: u16 type
    /// 0x000A followed by a u24 big-endian payload length.
    fn midi_resource_header(payload: &[u8]) -> Vec<u8> {
        let mut data = vec![0x0A, 0x00, 0, 0, 0];
        let length = payload.len() as u32;
        data[2] = (length >> 16) as u8;
        data[3] = (length >> 8) as u8;
        data[4] = length as u8;
        data.extend_from_slice(payload);
        data
    }

    fn tiny_midi_payload() -> Vec<u8> {
        let mut midi = Vec::new();
        midi.extend_from_slice(b"MThd");
        midi.extend_from_slice(&6u32.to_be_bytes());
        midi.extend_from_slice(&0u16.to_be_bytes());
        midi.extend_from_slice(&1u16.to_be_bytes());
        midi.extend_from_slice(&48u16.to_be_bytes());

        let mut track = Vec::new();
        track.extend_from_slice(&[0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20]);
        track.extend_from_slice(&[0x00, 0x90, 0x3C, 0x64]);
        track.extend_from_slice(&[0x30, 0x80, 0x3C, 0x00]);
        track.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);

        midi.extend_from_slice(b"MTrk");
        midi.extend_from_slice(&(track.len() as u32).to_be_bytes());
        midi.extend_from_slice(&track);
        midi
    }

    #[test]
    fn auto_bgm_midi_resource_decodes_and_restarts_when_consumed() {
        let payload = tiny_midi_payload();
        let resource = HostResource {
            name: "bgm.mid".to_string(),
            data: midi_resource_header(&payload),
        };
        let mut engine = crate::audio_engine::AudioEngine::new();
        engine.play_bytes(&resource.data).unwrap();
        let first = engine.diagnostics();
        assert!(first.decoded_frames > 0);

        // Drain every buffered frame; the engine then reports playing with an
        // empty queue, which is exactly the state `maybe_run_auto_bgm` treats
        // as "previous pass consumed".
        let frames = engine.buffered_frames();
        let drained = engine.pull_samples(frames);
        assert_eq!(drained.len(), frames * 2);
        assert_eq!(engine.buffered_frames(), 0);
        assert_eq!(engine.state(), 1);

        engine.play_bytes(&resource.data).unwrap();
        assert!(engine.buffered_frames() > 0);
        assert_eq!(engine.state(), 1);
    }
}
