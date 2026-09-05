//! NicaiEmu Core
//!
//! Platform-independent emulator core for Nicai/MStar CBE format games.
//! This crate provides:
//! - CBE archive loading and resource parsing
//! - SCE/MAP/Actor resource decoders
//! - Native ARM/Thumb execution and firmware service bridging
//! - Runtime and framebuffer state management
//! - Image decoding

pub mod audio_engine;
pub mod cbe;
pub mod image_decoder;
pub mod machine;
pub mod rotation_profile;
pub mod save_state;

// Experimental scene-level HLE runtime. Kept crate-internal for now: it is
// not wired to any frontend, which executes guest code through NicaiMachine.
mod runtime;

// Re-export commonly used types
pub use audio_engine::{AudioDiagnostics, AudioEngine, AUDIO_SAMPLE_RATE};
pub use cbe::{CbeArchive, ResourceEntry, ResourceType};
pub use image_decoder::DecodedImage;
pub use machine::{CbeExecutable, MachineState, NicaiMachine, Rotation, FRAME_HEIGHT, FRAME_WIDTH};
pub use rotation_profile::{
    load_rotation_overrides, parse_rotation_overrides, register_rotation_overrides,
    rotation_for_archive,
};
pub use save_state::{decode_machine, encode_machine, SERIALIZED_SIZE};

/// Guest screen scheduler frequency used by the original runtime.
pub const GUEST_FRAME_RATE: u32 = 10;

/// Default instruction budget for each guest callback.
///
/// Some games legitimately draw very heavy single frames; for example 雷电
/// renders its stage-intro animation (100 blend passes) in one callback,
/// which needs roughly 70M guest instructions. The budget must stay high
/// enough for those frames while still bounding runaway guest loops.
pub const DEFAULT_INSTRUCTION_LIMIT: u64 = 100_000_000;
