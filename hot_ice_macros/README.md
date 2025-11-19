# hot_ice_macros

Procedural macros for the hot_ice hot-reloading framework.

## `#[hot_update]`

An attribute macro that transforms an update function to handle `DynMessage` conversion automatically.

### Usage

Instead of manually writing the boilerplate for converting between `DynMessage` and your concrete message type:

```rust
// Before - manual conversion
fn update(&self, message: hot_ice::DynMessage) -> Result<hot_ice::runtime::Task<hot_ice::DynMessage>, hot_ice::HotFunctionError> {
    let message = message.into_message()
        .map_err(|_| hot_ice::HotFunctionError::MessageDowncastError)?;
    
    Ok(Self::my_update_logic(self, message)
        .map(hot_ice::DynMessage::into_hot_message))
}

fn my_update_logic(&self, message: Message) -> hot_ice::runtime::Task<Message> {
    // Your actual update logic
}
```

You can simply use the macro:

```rust
// After - with macro
use hot_ice::hot_update;

#[hot_update]
fn my_update_logic(&self, message: Message) -> Task<Message> {
    // Your actual update logic
}
```

The macro will:
1. **Always** generate a function named `update` that handles `DynMessage` conversion
2. Keep your original function with its original name for direct use
3. Handle errors with proper `HotFunctionError::MessageDowncastError`
4. Automatically map the return `Task<Message>` to `Task<DynMessage>`

### Example

```rust
use hot_ice::hot_update;
use hot_ice::runtime::Task;

struct MyApp;

#[hot_update]
fn handle_message(&self, message: MyMessage) -> Task<MyMessage> {
    match message {
        MyMessage::Increment => Task::none(),
        MyMessage::Decrement => Task::none(),
    }
}

// Expands to:
//
// fn update(&self, message: DynMessage) -> Result<Task<DynMessage>, HotFunctionError> {
//     let message = message.into_message()
//         .map_err(|_| HotFunctionError::MessageDowncastError)?;
//     Ok(Self::handle_message(self, message)
//         .map(DynMessage::into_hot_message))
// }
//
// fn handle_message(&self, message: MyMessage) -> Task<MyMessage> {
//     match message {
//         MyMessage::Increment => Task::none(),
//         MyMessage::Decrement => Task::none(),
//     }
// }
```

### Important Notes

- **The wrapper is always named `update`**: No matter what you name your function (`handle_message`, `process`, `my_update`, etc.), the generated wrapper will always be called `update`.
- **The original function keeps its name**: You can still call `handle_message()` directly in tests or other code.
- **Only one `#[hot_update]` per impl block**: Since the wrapper is always named `update`, you can only have one function with this attribute per type.

### Requirements

- The function must have a `&self` receiver
- The second parameter should be your message type
- Must return `Task<YourMessage>`
- Visibility modifiers (`pub`, `pub(crate)`, etc.) are preserved on the wrapper function

### What if I already have a function named `update`?

If you annotate a function that's already named `update`:

```rust
#[hot_update]
fn update(&self, message: Message) -> Task<Message> {
    // Your logic
}
```

You'll get a **compilation error** because you'll have two functions named `update`. In this case, simply rename your function to something else:

```rust
#[hot_update]
fn update_impl(&self, message: Message) -> Task<Message> {
    // Your logic
}
// This generates `update()` wrapper and keeps `update_impl()`
```
