#[cfg(not(target_family = "wasm"))]
mod native;
#[cfg(target_family = "wasm")]
mod wasm;

pub use web_time::Instant;

#[cfg(not(target_family = "wasm"))]
pub use native::{block_on, spawn, vault_root};
#[cfg(target_family = "wasm")]
pub use wasm::{block_on, spawn, vault_root};
