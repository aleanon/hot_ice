# Hot Message Example

This example demonstrates the **minimum requirements** for using the `#[hot_ice::hot_fn]`
macro on any type that returns messages. Unlike the hot_state example,
this example shows what you need to add when using hot_fn without hot_state for persistence.

## Key Concept: Message-Based Hot Reloading

When using `#[hot_ice::hot_fn]` on functions that return messages
(like `update` functions), **all functions that return messages must use the macro**.
This is the fundamental requirement for hot reloading to work correctly.

## Minimum Requirements

### 1. Apply `#[hot_ice::hot_fn]` to All Message-Returning Functions

In `ui/src/lib.rs`, notice which functions have the macro:

```rust
impl State {
    #[hot_ice::hot_fn]  // ✅ Required - returns Task<Message>
    pub fn boot() -> (State, Task<Message>) { /* ... */ }

    #[hot_ice::hot_fn]  // ✅ Required - returns Task<Message>
    pub fn update(&mut self, message: Message) -> Task<Message> { /* ... */ }

    #[hot_ice::hot_fn]  // ✅ Required - returns Element<Message> (can produce messages)
    pub fn view(&self) -> Element<'_, Message> { /* ... */ }

    #[hot_ice::hot_fn]  // ✅ Required - returns Subscription<Message>
    pub fn subscription(&self) -> Subscription<Message> { /* ... */ }

    // ❌ NOT required - returns Option<Theme> (not a message type)
    pub fn theme(&self) -> Option<Theme> { /* ... */ }

    // ❌ NOT required - returns theme::Style (not a message type)
    pub fn style(&self, theme: &Theme) -> theme::Style { /* ... */ }

    // ❌ NOT required - returns f32 (not a message type)
    pub fn scale_factor(&self) -> f32 { /* ... */ }

    // ❌ NOT required - returns String (not a message type)
    pub fn title(&self) -> String { /* ... */ }
}
```

### 2. State is Plain (No Serialization Required)

Since we're not using `#[hot_ice::hot_state]`, the state doesn't need
serialization traits:

```rust
#[derive(Debug, Clone)]  // ❌ No Serialize, Deserialize, Default needed
pub struct State {
    counter: counter::State,
    todo_list: todo_list::State,
    settings: settings::State,
}
```

### 3. Component States Also Plain

Component states are also plain structs without serialization requirements:

```rust
#[derive(Debug, Clone)]  // ❌ No serde needed
pub struct State {
    value: i32,
    auto_increment: bool,
}
```

## What Functions Need the Macro?

**Functions that return message types MUST use `#[hot_ice::hot_fn]`:**

- `boot() -> (State, Task<Message>)` ✅
- `update(&mut self, Message) -> Task<Message>` ✅
- `view(&self) -> Element<Message>` ✅
- `subscription(&self) -> Subscription<Message>` ✅

**Functions that don't return message types DON'T need the macro:**

- `theme(&self) -> Option<Theme>` ❌
- `style(&self, &Theme) -> Style` ❌
- `scale_factor(&self) -> f32` ❌
- `title(&self) -> String` ❌
- Any other helper functions returning non-message types ❌

## Key Difference from hot_state Example

| Feature | hot_state | hot_message |
|---------|-----------|-------------|
| Make changes to state | ✅ Yes | x No |
| State persistence | ✅ Automatic | ✅ |
| Serialization required | ✅ Yes | ❌ No |
| `#[hot_ice::hot_state]` needed | ✅ Yes | ❌ No |
| All functions that take &self need macro | ✅ Yes | x No |
| All message functions need macro | ✅ Yes | ✅ Yes |
| Simpler setup | ❌ | ✅ |

## Application Structure

This example contains the same three modules as other examples:

### 1. Counter Module (`ui/src/counter.rs`)

- Simple counter with increment/decrement
- Returns `Task<Message>` from update
- No serialization required

### 2. Todo List Module (`ui/src/todo_list.rs`)

- Add, toggle, delete todos
- State persists during runtime but **not across hot reloads**
- No serialization required

### 3. Settings Module (`ui/src/settings.rs`)

- Theme selection and scale factor
- Real-time updates
- No serialization required

## Running the Example

```bash
cargo run --release
```

## Development Workflow

1. **Run the application**: `cargo run --release`
2. **Make changes**: Modify any function that returns messages
3. **Hot reload**: Application automatically reloads with changes
4. **State behavior**:
   - State is preserved across hot reloads
   - State type can not be changed without complete recompile
   - UI changes are preserved

## When to Use This Pattern

Use `#[hot_ice::hot_fn]` without hot_state when:

- **Rapid prototyping**: You want the simplest hot reload setup
- **Unsupported**: If you are unable to implement traits
- **Performance focus**: Avoid serialization overhead

Use `#[hot_ice::hot_state]` when:

- **State persistence**: You need to maintain state across hot reloads
- **Complex applications**: State management is critical
- **User experience**: Losing state would disrupt workflow

## Important Notes

1. **All message functions must use the macro** - Missing any will break hot reloading
2. **State type changes must be recompiled** - Changes to the state type
require a complete recompile of the application.
3. **No serialization overhead** - Faster compilation without serde requirements
4. **Simpler setup** - Less boilerplate code compared to hot_state
