# hot_ice_macros

Procedural macros for the Hot Ice framework - enabling hot-reloading capabilities for Iced applications.

## Overview

`hot_ice_macros` provides a set of attribute macros that transform your application's lifecycle methods to work with Hot Ice's dynamic message system and hot-reloading infrastructure. These macros handle the conversion between your typed messages and Hot Ice's type-erased `HotMessage`, allowing your application code to remain clean and type-safe while supporting runtime code reloading.

## Macros

### `#[boot]`

Transforms a boot/initialization function to work with Hot Ice's message system.

**Before:**
```rust
fn new() -> (Self, Task<Message>) {
    // Your initialization logic
}
```

**After transformation:**
```rust
fn new() -> (Self, Task<hot_ice::HotMessage>) {
    use hot_ice::IntoBoot;
    
    let (app, task) = Self::new_inner().into_boot();
    (app, task.map(hot_ice::DynMessage::into_hot_message))
}

fn new_inner() -> (Self, Task<Message>) {
    // Your initialization logic (unchanged)
}
```

**Usage:**
```rust
#[boot]
fn new() -> (Self, Task<Message>) {
    (Self { /* ... */ }, Task::none())
}
```

### `#[update]`

Transforms an update function to handle dynamic message conversion with error handling.

**Before:**
```rust
fn update(&mut self, message: Message) -> Task<Message> {
    // Your update logic
}
```

**After transformation:**
```rust
#[unsafe(no_mangle)]
fn update(&mut self, message: hot_ice::HotMessage) 
    -> Result<Task<hot_ice::HotMessage>, hot_ice::HotFunctionError> 
{
    let message = message.into_message()
        .map_err(|message| hot_ice::HotFunctionError::MessageDowncastError(
            format!("{:?}", message)
        ))?;
    
    let task = self.update_inner(message)
        .map(hot_ice::DynMessage::into_hot_message);
    
    Ok(task)
}

fn update_inner(&mut self, message: Message) -> Task<Message> {
    // Your update logic (unchanged)
}
```

**Usage:**
```rust
#[update]
fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::Increment => self.counter += 1,
        Message::Decrement => self.counter -= 1,
    }
    Task::none()
}
```

**Note:** The `#[unsafe(no_mangle)]` attribute ensures the function name is preserved for dynamic loading.

### `#[view]`

Transforms a view function to return elements with `HotMessage` instead of your typed `Message`.

**Before:**
```rust
fn view(&self) -> Element<Message> {
    // Your view logic
}
```

**After transformation:**
```rust
#[unsafe(no_mangle)]
fn view(&self) -> Element<hot_ice::HotMessage> {
    self.view_inner()
        .map(hot_ice::DynMessage::into_hot_message)
}

fn view_inner(&self) -> Element<Message> {
    // Your view logic (unchanged)
}
```

**Usage:**
```rust
#[view]
fn view(&self) -> Element<Message> {
    column![
        button("Increment").on_press(Message::Increment),
        text(format!("Count: {}", self.counter)),
        button("Decrement").on_press(Message::Decrement),
    ].into()
}
```

### `#[subscription]`

Transforms a subscription function to return subscriptions with `HotMessage`.

**Before:**
```rust
fn subscription(&self) -> Subscription<Message> {
    // Your subscription logic
}
```

**After transformation:**
```rust
#[unsafe(no_mangle)]
fn subscription(&self) -> Subscription<hot_ice::HotMessage> {
    self.subscription_inner()
        .map(hot_ice::DynMessage::into_hot_message)
}

fn subscription_inner(&self) -> Subscription<Message> {
    // Your subscription logic (unchanged)
}
```

**Usage:**
```rust
#[subscription]
fn subscription(&self) -> Subscription<Message> {
    time::every(Duration::from_secs(1))
        .map(|_| Message::Tick)
}
```

**Attribute options:**
- `#[subscription]` - Default, adds `#[unsafe(no_mangle)]` for hot-reloading
- `#[subscription(not_hot)]` - Omits `#[unsafe(no_mangle)]`, useful for static subscriptions

### `#[auto_deser]`

Automatically adds serde derives and default attribute to structs for serialization support.

**Usage:**
```rust
#[auto_deser]
struct MyState {
    counter: i32,
    enabled: bool,
}
```

**Transformation:**
- Adds `#[derive(serde::Serialize, serde::Deserialize)]` if not present
- Adds `#[serde(default)]` if not present
- Skips derives that are already present

**After:**
```rust
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct MyState {
    counter: i32,
    enabled: bool,
}
```

This is particularly useful for state structures that need to persist across hot-reloads.

## Complete Example

```rust
use hot_ice_macros::{boot, update, view, subscription, auto_deser};
use iced::{Element, Task, Subscription, time};
use std::time::Duration;

#[auto_deser]
struct Counter {
    value: i32,
}

#[derive(Debug, Clone)]
enum Message {
    Increment,
    Decrement,
    Tick,
}

impl Counter {
    #[boot]
    fn new() -> (Self, Task<Message>) {
        (Self { value: 0 }, Task::none())
    }
    
    #[update]
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Increment => self.value += 1,
            Message::Decrement => self.value -= 1,
            Message::Tick => println!("Tick!"),
        }
        Task::none()
    }
    
    #[view]
    fn view(&self) -> Element<Message> {
        column![
            button("+").on_press(Message::Increment),
            text(format!("Value: {}", self.value)),
            button("-").on_press(Message::Decrement),
        ].into()
    }
    
    #[subscription]
    fn subscription(&self) -> Subscription<Message> {
        time::every(Duration::from_secs(1))
            .map(|_| Message::Tick)
    }
}
```

## Important Notes

### Hot Reloading Considerations

1. **Function Names:** If you change the name of a function marked with these macros, you must perform a full recompile. The `#[unsafe(no_mangle)]` attribute preserves function names for dynamic loading.

2. **Message Type Changes:** Changing your `Message` enum requires special handling. Hot Ice uses type erasure, so major message type changes may require a restart.

3. **State Serialization:** The `#[auto_deser]` macro helps preserve state across reloads, but ensure your state structure implements `Default` for proper initialization.

## How It Works

Hot Ice uses a dynamic library system for hot-reloading:

1. Your application code is compiled into a dynamic library (`.so`/`.dll`/`.dylib`)
2. These macros transform your functions to use type-erased `HotMessage` at ABI boundaries
3. The Hot Ice runtime watches for file changes and reloads the library
4. Your application continues running with the new code, preserving state when possible

The inner functions (e.g., `update_inner`, `view_inner`) remain type-safe and work with your original `Message` type, while the outer functions handle the conversion to/from `HotMessage`.

## Dependencies

- `syn` 2.0 - Parsing Rust code
- `quote` 1.0 - Code generation
- `proc-macro2` 1.0 - Procedural macro support

## License

Part of the Hot Ice project.
