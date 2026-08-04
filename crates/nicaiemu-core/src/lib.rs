//! NicaiEmu Core
//!
//! Platform-independent emulator core for Nicai/MStar CBE format games.
//! This crate provides:
//! - CBE archive loading and resource parsing
//! - SCE/MAP/Actor resource decoders
//! - XSE script VM (planned)
//! - Runtime state management

pub mod cbe;
pub mod runtime;

// Re-export commonly used types
pub use cbe::{CbeArchive, ResourceEntry, ResourceType};
pub use runtime::NicaiRuntime;
