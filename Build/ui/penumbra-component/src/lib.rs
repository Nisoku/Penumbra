//! App-agnostic Slint component library and design used across Penumbra.
//!
//! This crate carries only `.slint` markup.

/// Absolute path to the directory that holds this library's `.slint` sources.
pub const SLINT_LIBRARY_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/components");
