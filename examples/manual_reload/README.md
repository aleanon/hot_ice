# Manual Reload Example

This example demonstrates manual configuration of hot reloading behavior in
hot_ice applications. Unlike the default hot reloading settings,
this example shows how to customize the reloader settings
to control compilation behavior.

## Key Configuration

The main difference in this example is the manual configuration of
reloader settings in `manual_reload/src/main.rs`:

```rust
let reloader_settings = hot_ice::ReloaderSettings {
    compile_in_reloader: false,
    ..Default::default()
};

hot_ice::application(State::boot, State::update, State::view)
    .reloader_settings(reloader_settings)
    // ... other configuration
    .run()
    .unwrap();
```

## ReloaderSettings Configuration

### `compile_in_reloader: false`

This setting controls whether the reloader automatically runs `cargo watch`
to compile the dynamic library:

- **When `false`** (as in this example): The reloader does **NOT**
run `cargo watch`, meaning:

  - You must manually compile the dynamic library yourself
  - You have full control over when and how the library is built
  - Use custom compilation flags or build scripts as needed
  - Better integration with custom build pipelines
  
- **When `true`** (default behavior): The reloader automatically
runs `cargo watch`, meaning:
  - Automatic compilation when source files change
  - Hot reloading works out-of-the-box
  - Less control over compilation process

## Application Structure

This example shares the same UI structure as the hot_state example,
with three main components:

### 1. Counter Module

- Increment/decrement functionality
- Reset capability
- Display current value
- Auto-increment toggle (prepared but not active)

### 2. Todo List Module

- Add new todos with text input
- Mark todos as complete/incomplete
- Delete todos
- Shows completion statistics

### 3. Settings Module

- Theme selection (22 available themes)
- Scale factor adjustment (0.5x to 2.0x)
- Real-time theme and scaling application

## Hot State Integration

All state management uses the `#[hot_ice::hot_state]` macro for automatic
state persistence across hot reloads:

```rust
#[hot_ice::hot_state]
#[derive(Debug, Clone)]
pub struct State {
    counter: counter::State,
    todo_list: todo_list::State,
    settings: settings::State,
}
```

All state components implement the required serialization traits:

- `#[derive(Serialize, Deserialize, Default)]`
- `#[serde(default)]` for backward compatibility

## Running the Example

If you have Just installed, run the command `just run`. Alternatively,
if you don't want the watch command and the program running in the
same terminal, run the following:

```bash
# Terminal 1: Run the main application
cargo run --release

# Terminal 2: Manually compile the dynamic library when you make changes
just watch
```

The main application compiles with optimizations (`--release` flag),
The library gets built with the dev profile.

## Development Workflow

1. **Start the application**: Run `cargo run --release` in one terminal
2. **Make changes**: Modify any UI code in the `ui/` directory
3. **Library compiles**: Cargo watch automatically compiles the library
4. **Hot reload**: The application will automatically detect the new
library and reload
5. **State persistence**: All state persists across reloads due to the
`#[hot_ice::hot_state]` macro

### Alternative Compilation

You can use whatever method you want to compile the library,
as long as it's compiled to a `dylib` or `cdylib`(cdylib compiles faster)

Use the default `compile_in_reloader: true` when:

- You want simple, automatic hot reloading out-of-the-box
- You don't need custom compilation control
- You prefer the convenience of automatic file watching
