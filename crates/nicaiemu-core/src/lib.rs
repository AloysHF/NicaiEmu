//! NicaiEmu Core
//!
//! Platform-independent emulator core for Nicai/MStar CBE format games.
//! This crate provides:
//! - CBE archive loading and resource parsing
//! - SCE/MAP/Actor resource decoders
//! - XSE script VM (planned)
//! - Runtime state management
//! - Image decoding

pub mod cbe;
pub mod image_decoder;
pub mod runtime;

// Re-export commonly used types
pub use cbe::{CbeArchive, ResourceEntry, ResourceType};
pub use image_decoder::DecodedImage;
pub use runtime::NicaiRuntime;
