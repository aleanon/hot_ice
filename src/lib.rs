#![feature(downcast_unchecked)]
#![feature(min_specialization)]

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

//Re-export
pub use iced;
pub use serde;
pub use serde_derive;

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

// Re-export for the export_executor! macro (needs $crate::executor::... paths)
// The executor module is already pub, so ErasedWorkerContext and
// erased_worker_loop are accessible via $crate::executor::
