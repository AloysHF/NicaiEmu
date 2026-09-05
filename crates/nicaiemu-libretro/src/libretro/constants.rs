// libretro constants used by the NicaiEmu core.

/// libretro API version implemented by this core.
pub const RETRO_API_VERSION: u32 = 1;

/// Device types.
pub const RETRO_DEVICE_NONE: u32 = 0;
pub const RETRO_DEVICE_JOYPAD: u32 = 1;
pub const RETRO_DEVICE_POINTER: u32 = 6;

/// RetroPad button identifiers.
pub const RETRO_DEVICE_ID_JOYPAD_B: u32 = 0;
pub const RETRO_DEVICE_ID_JOYPAD_Y: u32 = 1;
pub const RETRO_DEVICE_ID_JOYPAD_SELECT: u32 = 2;
pub const RETRO_DEVICE_ID_JOYPAD_START: u32 = 3;
pub const RETRO_DEVICE_ID_JOYPAD_UP: u32 = 4;
pub const RETRO_DEVICE_ID_JOYPAD_DOWN: u32 = 5;
pub const RETRO_DEVICE_ID_JOYPAD_LEFT: u32 = 6;
pub const RETRO_DEVICE_ID_JOYPAD_RIGHT: u32 = 7;
pub const RETRO_DEVICE_ID_JOYPAD_A: u32 = 8;
pub const RETRO_DEVICE_ID_JOYPAD_X: u32 = 9;
pub const RETRO_DEVICE_ID_JOYPAD_L: u32 = 10;
pub const RETRO_DEVICE_ID_JOYPAD_R: u32 = 11;

/// Pointer device identifiers.
pub const RETRO_DEVICE_ID_POINTER_X: u32 = 0;
pub const RETRO_DEVICE_ID_POINTER_Y: u32 = 1;
pub const RETRO_DEVICE_ID_POINTER_PRESSED: u32 = 2;
pub const RETRO_DEVICE_ID_POINTER_RELEASED: u32 = 3;

/// Environment callback commands used by this core.
pub const RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL: u32 = 8;
pub const RETRO_ENVIRONMENT_SET_PIXEL_FORMAT: u32 = 10;
pub const RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS: u32 = 11;
pub const RETRO_ENVIRONMENT_GET_VARIABLE: u32 = 15;
pub const RETRO_ENVIRONMENT_SET_VARIABLES: u32 = 16;
pub const RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE: u32 = 17;
pub const RETRO_ENVIRONMENT_GET_LOG_INTERFACE: u32 = 27;
/// Query the frontend's system directory for core support files.
pub const RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY: u32 = 45;
pub const RETRO_ENVIRONMENT_SET_MEMORY_MAPS: u32 = 36 | 0x1_0000;
/// Notification that the presented game geometry (base size or aspect)
/// changed while content is running.
pub const RETRO_ENVIRONMENT_SET_GEOMETRY: u32 = 59;

/// Region identifiers.
pub const RETRO_REGION_NTSC: u32 = 0;

/// Memory types.
pub const RETRO_MEMORY_MASK: u32 = 0xFF;
pub const RETRO_MEMORY_SAVE_RAM: u32 = 0;
pub const RETRO_MEMORY_SYSTEM_RAM: u32 = 2;
pub const RETRO_MEMORY_VIDEO_RAM: u32 = 3;

/// Memory descriptor flags.
pub const RETRO_MEMDESC_SYSTEM_RAM: u64 = 1 << 2;
pub const RETRO_MEMDESC_VIDEO_RAM: u64 = 1 << 4;
