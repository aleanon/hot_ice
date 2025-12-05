# Hot Ice 🔥❄️

**Hot-reloadable Iced applications** - Write GUI apps that update instantly as you code, without losing application state.

Hot Ice is a framework built on top of [Iced](https://github.com/iced-rs/iced) that enables true hot-reloading for Rust GUI applications. Edit your code, save the file, and watch your running application update in real-time while preserving its current state.

## Features

- 🔥 **True Hot Reloading** - Update your application code without restarting
- 💾 **State Preservation** - Application state persists across reloads
- 🎯 **Type-Safe** - Full type safety with zero runtime overhead in production
- 🚀 **Fast Iteration** - See changes in milliseconds, not seconds
- 🔧 **Automatic Compilation** - Built-in file watcher and incremental builds
- 🎨 **Full Iced Compatibility** - Works with all Iced widgets and features
- 🐛 **Debug Support** - Optional dev tools and time-travel debugging

## How It Works

Hot Ice uses a dynamic library approach:

1. Your application code compiles into a dynamic library (`.so`/`.dll`/`.dylib`)
2. Hot Ice watches your source files for changes
3. On save, it triggers an incremental rebuild
4. The new library is loaded while your app keeps running
5. State is preserved using serialization at ABI boundaries

This approach gives you the development speed of interpreted languages with the performance of compiled Rust.

## Quick Start

### Installation

Add Hot Ice to your `Cargo.toml`:

```toml
[dependencies]
hot_ice = { path = "../hot_ice" }  # or from crates.io when published
serde = { version = "1.0", features = ["derive"] }

```


### Basic Example

```rust
use hot_ice::{hot_application, boot, update, view};
use iced::{Element, Task};
use iced_widget::{button, column, text};
use serde::{Deserialize, Serialize};

// Your application state - must implement Serialize + Deserialize for state preservation
#[derive(Default, Serialize, Deserialize)]
struct Counter {
    value: i32,
}

// Your message type
#[derive(Debug, Clone)]
enum Message {
    Increment,
    Decrement,
}

impl Counter {
    // Initialize your application
    #[boot]
    fn new() -> (Self, Task<Message>) {
        (Self::default(), Task::none())
    }

    // Handle messages and update state
    #[update]
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Increment => self.value += 1,
            Message::Decrement => self.value -= 1,
        }
        Task::none()
    }

    // Render your UI
    #[view]
    fn view(&self) -> Element<Message> {
        column![
            button("+").on_press(Message::Increment),
            text(format!("Count: {}", self.value)),
            button("-").on_press(Message::Decrement),
        ]
        .into()
    }
}

fn main() -> iced_winit::Result {
    hot_application(Counter::new, Counter::update, Counter::view)
        .title("Hot Counter")
        .window_size((400, 300))
        .run()
}
```

Now run your app with:

```bash
cargo run
```

While it's running, edit the `view` function to change the UI - your changes appear instantly!

## The Hot Ice Macros

Hot Ice provides attribute macros that transform your code to work with the hot-reloading system. See the [hot_ice_macros README](./hot_ice_macros/README.md) for detailed documentation.

### `#[boot]`

Marks your initialization function. Converts `(State, Task<Message>)` to work with Hot Ice's message system.

```rust
#[boot]
fn new() -> (Self, Task<Message>) {
    (Self { /* ... */ }, Task::none())
}
```

### `#[update]`

Marks your update function. Handles message type conversion and error handling for hot-reloading.

```rust
#[update]
fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::DoSomething => { /* ... */ }
    }
    Task::none()
}
```

### `#[view]`

Marks your view function. Converts your typed `Element<Message>` to work with Hot Ice's system.

```rust
#[view]
fn view(&self) -> Element<Message> {
    column![
        text("Hello, Hot Ice!"),
    ].into()
}
```

### `#[subscription]`

Marks your subscription function for event streams (timers, websockets, etc.).

```rust
#[subscription]
fn subscription(&self) -> Subscription<Message> {
    time::every(Duration::from_secs(1))
        .map(|_| Message::Tick)
}
```

## Application Builder API

The `hot_application` function returns a builder with a fluent API:

```rust
hot_application(Counter::new, Counter::update, Counter::view)
    // Window configuration
    .title("My App")
    .window_size((800, 600))
    .centered()
    .resizable(true)
    
    // Application features
    .subscription(|state| my_subscription(state))
    .theme(|state| if state.dark_mode { Theme::Dark } else { Theme::Light })
    
    // Hot-reload settings
    .reloader_settings(ReloaderSettings {
        target_dir: "target/reload".to_string(),
        file_watch_debounce: Duration::from_millis(50),
        ..Default::default()
    })
    
    // Run it!
    .run()
```

### Builder Methods

#### Window Configuration
- `.title(title)` - Set window title (string or closure)
- `.window_size(size)` - Set window dimensions
- `.centered()` - Center window on screen
- `.position(position)` - Set window position
- `.resizable(bool)` - Allow/disallow window resizing
- `.decorations(bool)` - Show/hide window decorations
- `.transparent(bool)` - Enable transparent window
- `.level(level)` - Set window level (normal, floating, etc.)
- `.exit_on_close_request(bool)` - Control exit behavior

#### Application Features
- `.subscription(fn)` - Add subscriptions for async events
- `.theme(fn)` - Dynamic theming based on state
- `.style(fn)` - Custom styling
- `.scale_factor(fn)` - Custom DPI scaling
- `.executor::<E>()` - Custom async executor

#### Rendering Configuration
- `.antialiasing(bool)` - Enable antialiasing
- `.default_font(font)` - Set default font
- `.font(bytes)` - Load additional fonts

#### Hot-Reload Configuration
- `.reloader_settings(settings)` - Configure reload behavior

## State Preservation

For your state to persist across hot-reloads, it must implement `Serialize` and `Deserialize`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct MyState {
    counter: i32,
    text: String,
    #[serde(skip)]  // Don't serialize this field
    cached_data: Vec<u8>,
}

impl Default for MyState {
    fn default() -> Self {
        Self {
            counter: 0,
            text: String::new(),
            cached_data: Vec::new(),
        }
    }
}
```

Use the `#[auto_deser]` macro for convenience:

```rust
use hot_ice_macros::auto_deser;

#[auto_deser]
struct MyState {
    counter: i32,
    text: String,
}
```

This automatically adds the necessary derives and `#[serde(default)]` attribute.

## Configuration

### Reloader Settings

Customize the hot-reload behavior:

```rust
use hot_ice::reloader::ReloaderSettings;
use std::time::Duration;

let settings = ReloaderSettings {
    // Where to build the dynamic library
    target_dir: "target/reload".to_string(),
    
    // Where to find the compiled library
    lib_dir: "target/reload/debug".to_string(),
    
    // Whether to run cargo watch automatically
    compile_in_reloader: true,
    
    // Debounce time for file changes
    file_watch_debounce: Duration::from_millis(25),
    
    // Custom watch directory (None = auto-detect)
    watch_dir: None,
};

hot_application(/* ... */)
    .reloader_settings(settings)
    .run()
```

### Project Structure

For hot-reloading to work, your project structure should be:

```
my_app/
├── Cargo.toml       # Main crate
├── src/
│   ├── lib.rs       # Your app code (with #[lib] crate-type = ["cdylib", "rlib"])
│   └── main.rs      # Calls hot_application
└── target/
    └── reload/      # Hot-reload builds go here
```

## Features

### Debug Features

Enable dev tools and time-travel debugging:

```toml
[dependencies]
hot_ice = { path = "../hot_ice", features = ["debug", "time-travel"] }
```

This adds:
- Visual debugger overlay
- State inspection
- Time-travel debugging (rewind/replay)

## Advanced Usage

### Custom Subscriptions

```rust
use iced_futures::Subscription;

#[subscription]
fn subscription(&self) -> Subscription<Message> {
    // Combine multiple subscriptions
    Subscription::batch([
        time::every(Duration::from_secs(1))
            .map(|_| Message::Tick),
        
        keyboard::on_key_press(|key, mods| {
            // Handle keyboard events
            Some(Message::KeyPressed(key))
        }),
    ])
}
```

### Dynamic Theming

```rust
hot_application(App::new, App::update, App::view)
    .theme(|state| {
        if state.dark_mode {
            Theme::CatppuccinMocha
        } else {
            Theme::CatppuccinLatte
        }
    })
    .run()
```

### Custom Title

```rust
// Static title
.title("My App")

// Dynamic title based on state
.title(|state| format!("Counter: {}", state.value))
```

## Important Notes

### Function Name Changes

If you change the name of a function marked with `#[boot]`, `#[update]`, `#[view]`, or `#[subscription]`, you **must** perform a full recompile. The macros use `#[unsafe(no_mangle)]` to preserve function names for dynamic loading.

### Message Type Stability

Changing your `Message` enum significantly may require a restart. Minor additions are usually fine, but changing variants or their data can cause issues.

### Performance

Hot-reloading has zero overhead in release builds. The dynamic library system is only active in debug/development mode. For production, compile normally:

```bash
cargo build --release
```

Your application will be a standard static binary with no hot-reloading machinery.

## Troubleshooting

### "Function not found" errors

- Make sure your crate is configured as `crate-type = ["cdylib", "rlib"]`
- Verify the macros are applied: `#[boot]`, `#[update]`, `#[view]`
- Try a full rebuild: `cargo clean && cargo build`

### Changes not appearing

- Check that files are saving properly
- Verify the `target/reload` directory is being created
- Increase `file_watch_debounce` if on a slow filesystem
- Look for compilation errors in the console

### State not persisting

- Ensure your state implements `Serialize` and `Deserialize`
- Add `#[serde(default)]` to your state struct
- Check for `#[serde(skip)]` on fields that shouldn't persist

## Examples

See the examples directory for more complete applications:

- `counter` - Simple counter with hot-reloading
- `todo` - Todo list with state persistence
- `theming` - Dynamic theme switching
- `subscriptions` - Working with time and events

## Architecture

Hot Ice consists of:

- **hot_ice** - Main framework and runtime
- **hot_ice_macros** - Procedural macros for code transformation
- **Reloader** - File watching and dynamic library management
- **LibReloader** - Low-level library loading and symbol resolution

The framework uses type erasure at ABI boundaries to enable hot-reloading while maintaining type safety in your application code.

## Platform Support

- ✅ Linux
- ✅ macOS (with code signing for dynamic libraries)
- ✅ Windows
- ❌ WebAssembly (not applicable for hot-reloading)

## Contributing

Contributions welcome! This is an experimental framework exploring hot-reloading in Rust.

## License

[Your license here]

## Credits

Built on top of [Iced](https://github.com/iced-rs/iced) - A cross-platform GUI library for Rust.
