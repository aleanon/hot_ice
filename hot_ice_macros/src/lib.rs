//! # Hot Ice Macros
//!
//! Procedural macros for enabling hot reloading in Iced applications.
//!
//! This crate provides two main macros:
//! - [`hot_fn`] - Transforms functions for hot reloading
//! - [`hot_state`] - Enables state serialization for persistence across reloads
//!
//! ## Quick Start
//!
//! ### View-Only Hot Reloading (Simplest)
//!
//! For iterating on UI without full message hot reloading:
//!
//! ```rust,ignore
//! impl State {
//!     #[hot_ice::hot_fn(cold_message)]
//!     pub fn view(&self) -> Element<'_, Message> {
//!         // Your view code - hot reloadable!
//!     }
//!
//!     // Other functions don't need macros
//!     pub fn update(&mut self, message: Message) -> Task<Message> { /* ... */ }
//! }
//! ```
//!
//! ### Message Hot Reloading
//!
//! For hot reloading all message-returning functions:
//!
//! ```rust,ignore
//! impl State {
//!     #[hot_ice::hot_fn]
//!     pub fn boot() -> (State, Task<Message>) { /* ... */ }
//!
//!     #[hot_ice::hot_fn]
//!     pub fn update(&mut self, message: Message) -> Task<Message> { /* ... */ }
//!
//!     #[hot_ice::hot_fn]
//!     pub fn view(&self) -> Element<'_, Message> { /* ... */ }
//!
//!     #[hot_ice::hot_fn]
//!     pub fn subscription(&self) -> Subscription<Message> { /* ... */ }
//!
//!     // Non-message functions don't need macros
//!     pub fn theme(&self) -> Option<Theme> { /* ... */ }
//! }
//! ```
//!
//! ### Full Hot State (State Persistence)
//!
//! For hot reloading with state type changes:
//!
//! ```rust,ignore
//! #[hot_ice::hot_state]
//! #[derive(Debug, Clone)]
//! pub struct State {
//!     counter: i32,
//!     items: Vec<String>,
//! }
//!
//! impl State {
//!     #[hot_ice::hot_fn(hot_state)]
//!     pub fn boot() -> (State, Task<Message>) { /* ... */ }
//!
//!     #[hot_ice::hot_fn(hot_state)]
//!     pub fn update(&mut self, message: Message) -> Task<Message> { /* ... */ }
//!
//!     #[hot_ice::hot_fn(hot_state)]
//!     pub fn view(&self) -> Element<'_, Message> { /* ... */ }
//!
//!     // All functions need the macro with hot_state
//!     #[hot_ice::hot_fn(hot_state)]
//!     pub fn theme(&self) -> Option<Theme> { /* ... */ }
//! }
//! ```

mod hot_fn;
mod hot_state;

/// Marks a struct for hot state serialization and persistence.
///
/// This macro enables your application state to be serialized and deserialized
/// across hot reloads, allowing you to modify the state structure without losing
/// data during development.
///
/// # What It Does
///
/// The macro automatically:
/// 1. Derives `Serialize`, `Deserialize`, and `Default` if not already present
/// 2. Adds `#[serde(default)]` to handle missing fields gracefully
/// 3. Generates serialization/deserialization functions for the hot reload system
///
/// # Requirements
///
/// All types contained within the struct must implement:
/// - `serde::Serialize`
/// - `serde::Deserialize`
/// - `Default` (or use `#[serde(default)]` on fields)
///
/// # Example
///
/// ```rust,ignore
/// use serde::{Deserialize, Serialize};
///
/// #[hot_ice::hot_state]
/// #[derive(Debug, Clone)]
/// pub struct State {
///     counter: i32,
///     items: Vec<String>,
///     settings: Settings,
/// }
///
/// // Nested types must also be serializable
/// #[derive(Debug, Clone, Serialize, Deserialize, Default)]
/// #[serde(default)]
/// pub struct Settings {
///     theme: String,
///     scale: f32,
/// }
/// ```
///
/// # Usage with `hot_fn`
///
/// When using `hot_state`, all functions that access state
/// and are passed to the application function must use
/// `#[hot_ice::hot_fn(hot_state)]`:
///
/// ```rust,ignore
/// impl State {
///     #[hot_ice::hot_fn(hot_state)]
///     pub fn boot() -> (State, Task<Message>) {
///         (State::default(), Task::none())
///     }
///
///     #[hot_ice::hot_fn(hot_state)]
///     pub fn update(&mut self, message: Message) -> Task<Message> {
///         // Handle messages...
///         Task::none()
///     }
///
///     #[hot_ice::hot_fn(hot_state)]
///     pub fn view(&self) -> Element<'_, Message> {
///         // Build your UI...
///     }
/// }
/// ```
///
/// # Adding New Fields
///
/// Thanks to `#[serde(default)]`, you can add new fields to your state
/// and they will be initialized with their default values:
///
/// ```rust,ignore
/// #[hot_ice::hot_state]
/// #[derive(Debug, Clone)]
/// pub struct State {
///     counter: i32,
///     // New field - will be 0 for existing sessions
///     new_counter: i32,
/// }
/// ```
///
/// # Removing Fields
///
/// Removed fields are silently ignored during deserialization, so you can
/// safely remove fields without breaking existing sessions.
#[proc_macro_attribute]
pub fn hot_state(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    crate::hot_state::hot_state(_attr, item)
}

/// Transforms a function for hot reloading support.
///
/// This macro wraps your function to enable dynamic loading and hot reloading.
/// The function type is automatically detected based on its signature.
///
/// # Supported Functions
///
/// | Function | Signature | Description |
/// |----------|-----------|-------------|
/// | `boot` | `() -> (State, Task<Message>)` | Application initialization |
/// | `update` | `(&mut self, Message) -> Task<Message>` | Message handling |
/// | `view` | `(&self) -> Element<Message>` | UI rendering |
/// | `subscription` | `(&self) -> Subscription<Message>` | Event subscriptions |
/// | `theme` | `(&self) -> Option<Theme>` | Theme selection |
/// | `style` | `(&self, &Theme) -> theme::Style` | Window styling |
/// | `scale_factor` | `(&self) -> f32` | Display scaling |
/// | `title` | `(&self) -> String` | Window title |
///
/// # Arguments
///
/// ## No Arguments (Default)
///
/// Hot reloads the function with message type conversion:
///
/// ```rust,ignore
/// #[hot_ice::hot_fn]
/// pub fn update(&mut self, message: Message) -> Task<Message> {
///     // Your code here
/// }
/// ```
///
/// ## `hot_state`
///
/// Use with `#[hot_ice::hot_state]` for state persistence:
///
/// ```rust,ignore
/// #[hot_ice::hot_fn(hot_state)]
/// pub fn update(&mut self, message: Message) -> Task<Message> {
///     // Your code here
/// }
/// ```
///
/// This changes the function signature to work with `HotState` wrapper,
/// enabling state serialization across reloads.
///
/// ## `not-hot` / `not_hot`
///
/// Disables hot reloading for `update` and `subscription` functions:
///
/// ```rust,ignore
/// #[hot_ice::hot_fn(not_hot)]
/// pub fn subscription(&self) -> Subscription<Message> {
///     // This function won't be dynamically loaded
/// }
/// ```
///
/// The function still gets the wrapper transformation but without
/// `#[unsafe(no_mangle)]`, so it won't be exported for dynamic loading.
///
/// ## `cold_message` / `cold-message`
///
/// For `view` only - keeps the original `Message` type instead of `HotMessage`:
///
/// ```rust,ignore
/// #[hot_ice::hot_fn(cold_message)]
/// pub fn view(&self) -> Element<'_, Message> {
///     // Returns Element<Message>, not Element<HotMessage>
/// }
/// ```
///
/// Use this when you only want view hot reloading without message
/// type conversion overhead.
///
/// ## `feature = "..."`
///
/// Conditionally enables hot reloading based on a feature flag:
///
/// ```rust,ignore
/// #[hot_ice::hot_fn(feature = "hot-reload")]
/// pub fn view(&self) -> Element<'_, Message> {
///     // Hot reloading only when "hot-reload" feature is enabled
/// }
/// ```
///
/// This generates:
/// ```rust,ignore
/// #[cfg(feature = "hot-reload")]
/// // Hot reloading version
///
/// #[cfg(not(feature = "hot-reload"))]
/// // Original function unchanged
/// ```
///
/// ## Combining Arguments
///
/// Arguments can be combined:
///
/// ```rust,ignore
/// #[hot_ice::hot_fn(hot_state, feature = "dev")]
/// pub fn update(&mut self, message: Message) -> Task<Message> {
///     // Hot state + feature-gated
/// }
/// ```
///
/// # Examples
///
/// ## View-Only Hot Reloading
///
/// The simplest setup - only hot reload the view:
///
/// ```rust,ignore
/// impl State {
///     pub fn boot() -> (State, Task<Message>) {
///         (State::default(), Task::none())
///     }
///
///     pub fn update(&mut self, message: Message) -> Task<Message> {
///         Task::none()
///     }
///
///     #[hot_ice::hot_fn(cold_message)]
///     pub fn view(&self) -> Element<'_, Message> {
///         text("Hello, World!").into()
///     }
/// }
/// ```
///
/// ## Full Message Hot Reloading
///
/// Hot reload all message-returning functions:
///
/// ```rust,ignore
/// impl State {
///     #[hot_ice::hot_fn]
///     pub fn boot() -> (State, Task<Message>) {
///         (State::default(), Task::none())
///     }
///
///     #[hot_ice::hot_fn]
///     pub fn update(&mut self, message: Message) -> Task<Message> {
///         Task::none()
///     }
///
///     #[hot_ice::hot_fn]
///     pub fn view(&self) -> Element<'_, Message> {
///         text("Hello, World!").into()
///     }
///
///     #[hot_ice::hot_fn]
///     pub fn subscription(&self) -> Subscription<Message> {
///         Subscription::none()
///     }
///
///     // Non-message functions don't need the macro
///     pub fn theme(&self) -> Option<Theme> {
///         Some(Theme::Dark)
///     }
/// }
/// ```
///
/// ## Full Hot State
///
/// Hot reload with state persistence:
///
/// ```rust,ignore
/// #[hot_ice::hot_state]
/// #[derive(Debug, Clone)]
/// pub struct State {
///     value: i32,
/// }
///
/// impl State {
///     #[hot_ice::hot_fn(hot_state)]
///     pub fn boot() -> (State, Task<Message>) {
///         (State { value: 0 }, Task::none())
///     }
///
///     #[hot_ice::hot_fn(hot_state)]
///     pub fn update(&mut self, message: Message) -> Task<Message> {
///         Task::none()
///     }
///
///     #[hot_ice::hot_fn(hot_state)]
///     pub fn view(&self) -> Element<'_, Message> {
///         text(format!("Value: {}", self.value)).into()
///     }
///
///     #[hot_ice::hot_fn(hot_state)]
///     pub fn subscription(&self) -> Subscription<Message> {
///         Subscription::none()
///     }
///
///     #[hot_ice::hot_fn(hot_state)]
///     pub fn theme(&self) -> Option<Theme> {
///         Some(Theme::Dark)
///     }
///
///     #[hot_ice::hot_fn(hot_state)]
///     pub fn style(&self, theme: &Theme) -> theme::Style {
///         theme::default(theme)
///     }
///
///     #[hot_ice::hot_fn(hot_state)]
///     pub fn scale_factor(&self) -> f32 {
///         1.0
///     }
///
///     #[hot_ice::hot_fn(hot_state)]
///     pub fn title(&self) -> String {
///         "My App".to_string()
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn hot_fn(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    crate::hot_fn::hot_fn(attr, item)
}
