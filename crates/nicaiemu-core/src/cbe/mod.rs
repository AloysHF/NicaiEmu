//! CBE Format Module
//!
//! This module provides parsing and handling for the CBE (Cool Bar Engine) game format.
//! CBE files are container archives used by Nicai/MStar mobile phones.

pub mod archive;
pub mod resource;
pub mod sce;
pub mod map;
pub mod actor;

// Re-export main types
pub use archive::CbeArchive;
pub use resource::{ResourceEntry, ResourceType};
