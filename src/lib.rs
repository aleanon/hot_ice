#![feature(downcast_unchecked)]
#![feature(min_specialization)]

mod boot;
#[cfg(target_os = "macos")]
mod codesign;
mod error;
mod executor;
mod hot_application;
mod hot_program;
mod hot_scale_factor;
mod hot_state;
mod hot_style;
mod hot_subscription;
mod hot_theme;
mod hot_title;
mod hot_update;
mod hot_view;
mod into_result;
mod lib_reloader;
mod message;
mod reloader;
mod winit;

//Re-export
pub use iced;
pub use iced_graphics;
pub use serde;
pub use serde_derive;

pub use hot_application::application;
pub use hot_ice_macros::{hot_fn, hot_state};
pub use reloader::ReloaderSettings;

pub mod macro_use {
    pub use super::error::{HotIceError, HotResult};
    pub use super::hot_state::{DynState, HotState};
    pub use super::message::{DynMessage, HotMessage};
}
