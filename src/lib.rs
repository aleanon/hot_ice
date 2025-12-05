#![feature(downcast_unchecked)]

mod boot;
#[cfg(target_os = "macos")]
mod codesign;
mod error;
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
mod lib_reloader;
mod message;
mod reloader;

//Re-export
pub use serde;
pub use type_hash;

pub use boot::IntoBoot;
pub use error::HotFunctionError;
pub use hot_application::hot_application;
pub use hot_ice_macros::{boot, hot_state, subscription, update, view};
pub use hot_state::HotState;
pub use message::{DynMessage, HotMessage};

pub const SERIALIZE_STATE_FUNCTION_NAME: &str = "serialize_state";
pub const DESERIALIZE_STATE_FUNCTION_NAME: &str = "deserialize_state";
