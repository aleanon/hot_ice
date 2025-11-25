#![feature(downcast_unchecked)]

mod boot;
#[cfg(target_os = "macos")]
mod codesign;
mod error;
mod hot_application;
mod hot_fn;
mod hot_program;
mod hot_subscription;
mod hot_theme;
mod hot_update;
mod hot_view;
mod lib_reloader;
mod message;
mod reloader;

pub use hot_application::hot_application;
pub use message::{DynMessage, HotMessage};

pub use error::HotFunctionError;
pub use hot_ice_macros::{subscription, update, view};
