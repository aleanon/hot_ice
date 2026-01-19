# Hot State Example

This example demonstrates the use of the `#[hot_ice::hot_state]` macro
in `lib.rs` for creating hot-reloadable state management in Iced applications.

## Macro Usage

The `#[hot_ice::hot_state]` macro is applied to the main `State` struct in `ui/src/lib.rs`:

```rust
#[hot_ice::hot_state]
#[derive(Debug, Clone)]
pub struct State {
    counter: counter::State,
    todo_list: todo_list::State,
    settings: settings::State,
}
```

This macro enables:

- Hot reloading of state changes during development
- Automatic state persistence and restoration
- Type-safe state management across nested components

## Required Settings for Contained Types

All types used within a `#[hot_ice::hot_state]` struct must implement certain
traits to support serialization and deserialization for hot reloading:

### 1. Serialization Traits

Every type must derive `Serialize` and `Deserialize` from `serde`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct State {
    value: i32,
    auto_increment: bool,
}
```

### 2. Default Values

Use `#[serde(default)]` on the struct to ensure missing fields can be populated
with default values during deserialization. Additionally,
implement `Default` or use `#[derive(Default)]`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub enum ThemeChoice {
    Light,
    #[default]
    Dark,
    // ... other variants
}
```

### 3. Complete Example

Here's a properly configured type for use within hot state:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TodoItem {
    text: String,
    completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct State {
    input: String,
    todos: Vec<TodoItem>,
}
```

### 4. function macros

When using hot state, you need to add an argument to the hot_fn macro:

It is also needed to use the macro on any function that is used in
application()

```rust

    // This function will not be hot, but it needs the macro to use the
    // correct return type(s) to allign with the hot functions
    #[hot_ice::hot_fn(hot-state)]
    fn new() -> Self {
        Self {
            input: String::new(),
            todos: Vec::new(),
        }
    }

    #[hot_ice::hot_fn(hot-state)]
    fn update(&mut self, message: Message) -> Task<Message> {
        // ...
    }
```

## Important Notes

1. **All nested types must be serializable** - Any type contained in a hot
state struct must implement `Serialize` and `Deserialize`
2. **Use `#[serde(default)]`** - This ensures backward compatibility when
adding new fields
3. **Implement `Default`** - Either derive it or implement it manually
for proper initialization
4. **Keep serializable** - Avoid non-serializable types like function pointers
or complex system resources

## Running the Example

run with optimizations, the dynamicly loaded library still compiles without optimizations

```bash
cargo run --release
```

The example shows:

- A counter with increment/decrement functionality
- A todo list with add/delete/toggle operations  
- Settings for theme and scale factor
- All state persists across hot reloads during development
