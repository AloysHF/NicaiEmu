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
pub mod save_state;

// Experimental scene-level HLE runtime. Kept crate-internal for now: it is
// not wired to any frontend, which executes guest code through NicaiMachine.
mod runtime;

// Re-export commonly used types
pub use audio_engine::{AudioDiagnostics, AudioEngine, AUDIO_SAMPLE_RATE};
pub use cbe::{CbeArchive, ResourceEntry, ResourceType};
pub use image_decoder::DecodedImage;
pub use machine::{CbeExecutable, MachineState, NicaiMachine};
pub use save_state::{decode_machine, encode_machine, SERIALIZED_SIZE};
