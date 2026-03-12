# Hot Ice

**Hot-reloadable applications for [Iced](https://github.com/iced-rs/iced)**
Edit your GUI code and see changes instantly without restarting your application.

![Hot Ice Demo](./assets/demo.gif)

## Features

- **Hot Reloading** - Update your code without restarting the application
- **State Preservation** - Application state persists across reloads
- **Automatic Compilation** - Built-in file watcher triggers incremental builds
- **Panic Recovery** - Gracefully handles panics in hot-reloaded code

## Quick Start

### Project Structure

Hot Ice requires a workspace with separate crates for your
binary and hot-reloadable UI:

```js
my_app/
├── Cargo.toml              # Workspace manifest
├── bin/                    # Binary crate
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
└── ui/                     # Hot-reloadable library crate
    ├── Cargo.toml
    └── src/
        └── lib.rs
```

### Workspace Cargo.toml

```toml
[workspace]
members = ["bin", "ui"]

[workspace.dependencies]
hot_ice = { git = "https://github.com/anthropics/hot_ice" }
ui = { path = "ui" }
```

### UI Crate (ui/Cargo.toml)

```toml
[package]
name = "ui"
version = "0.1.0"
edition = "2024"

[features]
reload = []

[dependencies]
hot_ice.workspace = true
```

### Binary Crate (bin/Cargo.toml)

```toml
[package]
name = "bin"
version = "0.1.0"
edition = "2024"

[features]
reload = []

[dependencies]
hot_ice.workspace = true
ui.workspace = true
```

### UI Code (ui/src/lib.rs)

```rust
use hot_ice::iced::widget::{button, column, text};
use hot_ice::iced::{Element, Task};

#[derive(Debug, Clone)]
pub enum Message {
    Increment,
    Decrement,
}

#[derive(Debug, Clone)]
pub struct State {
    value: i32,
}

impl State {
    #[hot_ice::hot_fn(feature = "reload")]
    pub fn new() -> (Self, Task<Message>) {
        (State { value: 0 }, Task::none())
    }

    #[hot_ice::hot_fn(feature = "reload")]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Increment => self.value += 1,
            Message::Decrement => self.value -= 1,
        }
        Task::none()
    }

    #[hot_ice::hot_fn(feature = "reload")]
    pub fn view(&self) -> Element<'_, Message> {
        column![
            button("+").on_press(Message::Increment),
            text(self.value).size(50),
            button("-").on_press(Message::Decrement),
        ]
        .spacing(10)
        .into()
    }
}
```

### Main Binary (bin/src/main.rs)

```rust
use ui::State;

#[cfg(feature = "reload")]
use hot_ice::application;
#[cfg(not(feature = "reload"))]
use hot_ice::iced::application;

fn main() {
    #[cfg(feature = "reload")]
    let reloader_settings = hot_ice::ReloaderSettings {
        feature: Some("reload".to_string()),
        ..Default::default()
    };

    let app = application(State::boot, State::update, State::view)
        .title(|_| String::from("My Hot App"));

    #[cfg(not(feature = "reload"))]
    app.run().unwrap();

    #[cfg(feature = "reload")]
    app.reloader_settings(reloader_settings).run().unwrap();
}
```

### Run

```bash
# With hot reloading
cargo run --release --features reload

# Without hot reloading (normal build)
cargo run
```

Now edit your `view` function and save - your changes appear instantly!

Hot Ice supports two levels of hot reloading, each with different trade-offs:

### 1. Hot reloading without hot state

If you only want to hot reload the logic and not the state,
mark the functions you want to be hot
with the hot_fn macro, but if you use the macro on one function that
returns a message, you must use
it on all functions that return a message.

```rust
impl State {
    #[hot_ice::hot_fn(feature = "reload")]
    pub fn new() -> (Self, Task<Message>) { /* ... */ }

    #[hot_ice::hot_fn(feature = "reload")]
    pub fn update(&mut self, message: Message) -> Task<Message> { /* ... */ }

    #[hot_ice::hot_fn(feature = "reload")]
    pub fn view(&self) -> Element<'_, Message> { /* ... */ }

    #[hot_ice::hot_fn(feature = "reload")]
    pub fn subscription(&self) -> Subscription<Message> { /* ... */ }

    // Non-message functions don't need the macro(but woun't be hot without it)
    pub fn theme(&self) -> Option<Theme> { /* ... */ }
}
```

**Best for:** Iterating on application logic without state serialization overhead

### 2. Full Hot State

Hot reload everything including state structure changes.
when using the `hot_state` argument
with the hot_fn macro, every method that is called from the application builder
must use #[hot_fn(hot_state)].

```rust
#[hot_ice::hot_state(feature = "reload")]  // Enables state serialization
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    value: i32,
    // Add new fields - they'll be initialized to default
}

impl State {
    #[hot_ice::hot_fn(hot_state, feature = "reload")]
    pub fn new() -> (Self, Task<Message>) { /* ... */ }

    #[hot_ice::hot_fn(hot_state, feature = "reload")]
    pub fn update(&mut self, message: Message) -> Task<Message> { /* ... */ }

    #[hot_ice::hot_fn(hot_state, feature = "reload")]
    pub fn view(&self) -> Element<'_, Message> { /* ... */ }

    // All functions need the macro with hot_state
    #[hot_ice::hot_fn(hot_state, feature = "reload")]
    pub fn theme(&self) -> Option<Theme> { /* ... */ }
}
```

**Best for:** Rapid prototyping with evolving state structures

### Comparison

| Feature | Message | Hot State |
|---------|---------|-----------|
| Hot-reload view | Yes | Yes |
| Hot-reload update | Yes | Yes |
| Hot-reload subscription | Yes | Yes |
| State type changes | Recompile | Hot reload |
| Serialization required | No | Yes |
| Setup complexity | Low | Medium |

## Application Builder

The `application` function returns a builder for configuring your app:

```rust
hot_ice::application(State::boot, State::update, State::view)
    // Callbacks
    .subscription(State::subscription)
    .theme(State::theme)
    .style(State::style)
    .scale_factor(State::scale_factor)
    .title(State::title)
    
    // Window settings
    .window_size((1024, 768))
    .centered()
    .resizable(true)
    .decorations(true)
    
    // Rendering
    .antialiasing(true)
    .default_font(Font::MONOSPACE)
    .font(include_bytes!("../fonts/custom.ttf").as_slice())
    
    // Hot reloading
    .reloader_settings(ReloaderSettings {
        compile_in_reloader: true,
        ..Default::default()
    })
    
    .run()
    .unwrap();
```

## Macro Reference

### `#[hot_fn]`

Transforms functions for hot reloading. Supports these arguments:

| Argument | Description |
|----------|-------------|
| *(none)* | Default hot reloading with message conversion |
| `hot_state` | Use with `#[hot_state]` for state persistence |
| `feature = "..."` | Conditional compilation — only emit hot-reload wrappers when the feature is enabled |

### `#[hot_state]`

Enables state serialization for persistence across reloads:

- Automatically derives `Serialize`, `Deserialize`, `Default`
- Adds `#[serde(default)]` for backward compatibility
- Generates serialization functions for the hot reload system

**Requirements:** All nested types must implement
`Serialize`, `Deserialize`, and `Default`.

## Reloader Settings

Configure hot reloading behavior:

```rust
use hot_ice::ReloaderSettings;
use std::time::Duration;

ReloaderSettings {
    // Build directory for the dynamic library
    target_dir: "target/reload".to_string(),
    
    // Location of compiled library
    lib_dir: "target/reload/debug".to_string(),
    
    // Auto-run cargo watch (set false for manual control)
    compile_in_reloader: true,
    
    // File change detection interval
    file_watch_debounce: Duration::from_millis(25),
    
    // Custom watch directory (None = auto-detect)
    watch_dir: None,
    
    // Feature to enable when compiling the cdylib
    feature: Some("reload".to_string()),
}
```

## Status Bar

Hot Ice displays a status bar showing the state of each function:

| Color | Meaning |
|-------|---------|
| White | Static (not hot-reloadable) |
| Green | Hot (loaded from dynamic library) |
| Orange | Fallback (failed to load, using static) |
| Red | Error (function returned an error) |

## Examples

The `examples/` directory contains complete working examples:

| Example | Description |
|---------|-------------|
| [`hot_message`](./examples/hot_message/) | hot reload message type changes |
| [`hot_state`](./examples/hot_state/) | Full state persistence |
| [`manual_reload`](./examples/manual_reload/) | Manual compilation control |

Run an example:

```bash
cd examples/hot_state
cargo run --features reload
```

## How It Works

1. **Startup**: Hot Ice compiles your UI crate as a dynamic library (`.so`/`.dll`/`.dylib`)
2. **File Watching**: `cargo watch` monitors your source files for changes
3. **Recompilation**: On save, an incremental rebuild is triggered
4. **Hot Reload**: The new library is loaded while your app keeps running
5. **State Transfer**: If using `hot_state`, state is serialized and restored

The status bar updates to show which functions are successfully hot-reloaded.

## Platform Support

| Platform | Status |
|----------|--------|
| Linux | Fully supported |
| macOS | Supported (with automatic code signing) |
| Windows | Supported |

## Troubleshooting

### Changes not appearing

- Ensure files are saved
- Check console for compilation errors

### "Function not found" warnings

- Make sure the correct macro is applied to all required functions
- Try a full rebuild: `cargo clean && cargo run --release`

### State not persisting (hot_state mode)

- Ensure all nested types implement `Serialize`, `Deserialize`, `Default`
- Add `#[serde(default)]` to structs
- Check for non-serializable types (use `#[serde(skip)]` if needed)

### Cargo watch not stopping

Hot Ice automatically cleans up `cargo watch` when the application exits.
If processes remain orphaned, they can be killed manually.

## License

[MIT License](./LICENSE)

## Credits

Built on [Iced](https://github.com/iced-rs/iced) - A cross-platform GUI library
for Rust focused on simplicity and type-safety.
