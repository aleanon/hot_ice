use iced_winit::runtime::Task;

use crate::{DynMessage, HotMessage};

/// The logic to initialize the `State` of some [`Application`].
///
/// This trait is implemented for both `Fn() -> State` and
/// `Fn() -> (State, Task<Message>)`.
///
/// In practice, this means that [`application`] can both take
/// simple functions like `State::default` and more advanced ones
/// that return a [`Task`].
pub trait Boot<State> {
    /// Initializes the [`Application`] state.
    fn boot(&self) -> (State, Task<HotMessage>);
}

impl<T, C, State> Boot<State> for T
where
    T: Fn() -> C,
    C: IntoBoot<State>,
{
    fn boot(&self) -> (State, Task<HotMessage>) {
        self().into_boot()
    }
}

/// The initial state of some [`Application`].
pub trait IntoBoot<State> {
    /// Turns some type into the initial state of some [`Application`].
    fn into_boot(self) -> (State, Task<HotMessage>);
}

impl<State> IntoBoot<State> for State {
    fn into_boot(self) -> (State, Task<HotMessage>) {
        (self, Task::none())
    }
}

impl<State, Message> IntoBoot<State> for (State, Task<Message>)
where
    Message: DynMessage,
{
    fn into_boot(self) -> (State, Task<HotMessage>) {
        let (state, task) = self;
        (state, task.map(DynMessage::into_hot_message))
    }
}
