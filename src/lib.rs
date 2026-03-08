#[cfg(target_os = "macos")]
mod codesign;
mod error;
pub mod executor;
mod functions;
mod hot_application;
mod hot_program;
mod hot_state;
mod into_result;
mod lib_reloader;
mod message;
mod panic_hook;
mod reloader;
mod winit;

pub use hot_application::application;
pub use hot_ice_macros::{hot_fn, hot_state};
pub use reloader::ReloaderSettings;

pub mod macro_use {
    pub use super::error::{HotIceError, HotResult};
    pub use super::hot_state::{DynState, HotState};
    pub use super::message::{DynMessage, HotMessage};
    pub use super::panic_hook::catch_panic;
    pub use iced_futures;
    pub use iced_graphics::text::font_system;
}

/// Re-export iced so downstream cdylib crates can use `hot_ice::iced` to
/// ensure they link against the exact same iced version as the host binary.
/// This prevents subtle ABI mismatches when the cdylib and host use
/// different iced versions.
pub use iced;
