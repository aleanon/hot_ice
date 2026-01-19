# Hot View Example

This example demonstrates **view-only hot reloading** using the `#[hot_ice::hot_fn(cold_message)]`
macro. This is the simplest hot reload setup - only the `view` function is hot-reloadable,
while all other functions remain static.

## Key Concept: View-Only Hot Reloading

When you only need to iterate on UI layout and styling, you don't need full message
hot reloading. By using `cold_message`, the view function keeps its original `Message`
type instead of converting to `HotMessage`, simplifying the setup.

## Minimum Requirements

### 1. Apply `#[hot_ice::hot_fn(cold_message)]` to the View Function Only

In `ui/src/lib.rs`, only the view function has the macro:

```rust
impl State {
    // No macro - returns (State, Task<Message>)
    pub fn boot() -> (State, Task<Message>) { /* ... */ }

    // No macro - returns Task<Message>
    pub fn update(&mut self, message: Message) -> Task<Message> { /* ... */ }

    #[hot_ice::hot_fn(cold_message)]  // Only this function is hot-reloadable
    pub fn view(&self) -> Element<'_, Message> { /* ... */ }

    // No macro - returns Subscription<Message>
    pub fn subscription(&self) -> Subscription<Message> { /* ... */ }

    // No macro - returns Option<Theme>
    pub fn theme(&self) -> Option<Theme> { /* ... */ }

    // No macro - returns theme::Style
    pub fn style(&self, theme: &Theme) -> theme::Style { /* ... */ }

    // No macro - returns f32
    pub fn scale_factor(&self) -> f32 { /* ... */ }

    // No macro - returns String
    pub fn title(&self) -> String { /* ... */ }
}
```

### 2. What `cold_message` Does

The `cold_message` argument tells the macro to:

- **Keep the original return type**: Returns `Element<Message>` instead of `Element<HotMessage>`
- **Skip message mapping**: Doesn't call `.map(DynMessage::into_hot_message)`
- **Still enable hot reloading**: The function still gets `#[unsafe(no_mangle)]` for dynamic loading

### 3. State is Plain (No Serialization Required)

Since we're not using `#[hot_ice::hot_state]`, the state doesn't need serialization traits:

```rust
#[derive(Debug, Clone)]  // No Serialize, Deserialize, Default needed
pub struct State {
    counter: counter::State,
    todo_list: todo_list::State,
    settings: settings::State,
}
```

## Comparison with Other Examples

| Feature | hot_view | hot_message | hot_state |
|---------|----------|-------------|-----------|
| Hot-reloadable view | Yes | Yes | Yes |
| Hot-reloadable update | No | Yes | Yes |
| Hot-reloadable boot | No | Yes | Yes |
| Hot-reloadable subscription | No | Yes | Yes |
| State persistence across reloads | Yes | Yes | Yes |
| State type changes | Recompile | Recompile | Hot reload |
| Serialization required | No | No | Yes |
| Macro on view only | Yes | No | No |
| Simplest setup | Yes | No | No |

## When to Use This Pattern

Use `#[hot_ice::hot_fn(cold_message)]` on view only when:

- **UI iteration**: You're primarily tweaking layouts, colors, and styling
- **Stable logic**: Your update/subscription logic is finalized
- **Minimal overhead**: You want the absolute simplest hot reload setup
- **Quick prototyping**: Getting visual feedback without full hot reload infrastructure

## Application Structure

This example contains three modules:

### 1. Counter Module (`ui/src/counter.rs`)

- Simple counter with increment/decrement
- Standard Iced pattern without macros

### 2. Todo List Module (`ui/src/todo_list.rs`)

- Add, toggle, delete todos
- Standard Iced pattern without macros

### 3. Settings Module (`ui/src/settings.rs`)

- Theme selection and scale factor
- Standard Iced pattern without macros

## Running the Example

```bash
cargo run --release
```

## Development Workflow

1. **Run the application**: `cargo run --release`
2. **Make view changes**: Modify the `view` function in `ui/src/lib.rs`
3. **Hot reload**: Application automatically reloads with UI changes
4. **State preserved**: All state remains intact during hot reloads
5. **Logic changes**: Require recompilation (only view is hot-reloadable)

## Example Changes You Can Hot Reload

With view-only hot reloading, you can instantly see changes to:

- Widget arrangement and layout
- Spacing and padding values
- Text content and sizes
- Color and styling
- Container structures
- Widget properties

## Important Notes

1. **Only view changes hot reload** - Changes to `update`, `boot`, or `subscription` require recompilation
2. **State type is fixed** - Changes to state structure require recompilation
3. **Simplest setup** - Only one function needs the macro
4. **Original message type** - Uses your `Message` enum directly, no conversion overhead
5. **Component views unaffected** - Child component `view` functions don't need macros unless you want them hot-reloadable too
