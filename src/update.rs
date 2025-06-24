use std::{
    any::type_name,
    marker::PhantomData,
    panic::{catch_unwind, AssertUnwindSafe},
};

use iced_winit::runtime::Task;

use crate::{hot_fn::HotFn, reloader::LIB_RELOADER, DynMessage, HotMessage};

/// The update logic of some [`Application`].
///
/// This trait allows the [`application`] builder to take any closure that
/// returns any `Into<Task<Message>>`.
pub trait Update<State, Message> {
    /// Processes the message and updates the state of the [`Application`].
    fn update(&self, state: &mut State, message: Message) -> impl Into<Task<Message>>;
}

impl<State, Message> Update<State, Message> for () {
    fn update(&self, _state: &mut State, _message: Message) -> impl Into<Task<Message>> {}
}
impl<T, State, Message, C> Update<State, Message> for T
where
    T: Fn(&mut State, Message) -> C,
    C: Into<Task<Message>>,
{
    fn update(&self, state: &mut State, message: Message) -> impl Into<Task<Message>> {
        self(state, message)
    }
}

pub struct HotUpdate<F, State, Message> {
    lib_name: &'static str,
    function_name: &'static str,
    function: F,
    _state: PhantomData<State>,
    _message: PhantomData<Message>,
}

impl<F, State, Message> HotUpdate<F, State, Message>
where
    Message: DynMessage + Clone,
    F: Update<State, Message>,
    State: 'static,
{
    pub fn new(function: F) -> Self {
        let type_name = type_name::<F>();
        let mut iterator = type_name.split("::");
        let lib_name = iterator.next().unwrap();
        let function_name = iterator.last().unwrap();

        Self {
            function,
            function_name,
            lib_name,
            _state: PhantomData,
            _message: PhantomData,
        }
    }

    pub fn update<'a>(&self, state: &'a mut State, message: HotMessage) -> Task<HotMessage> {
        if let Some(lock) = LIB_RELOADER.get().and_then(|map| map.get(&self.lib_name)) {
            if let Ok(lib) = lock.try_lock() {
                let message = message.clone();
                let state = state as *mut State;
                match message.into_message::<Message>() {
                    Ok(message) => {
                        match unsafe {
                            lib.get_symbol::<fn(&'a mut State, Message) -> Task<Message>>(
                                &self.function_name.as_bytes(),
                            )
                        } {
                            Ok(function) => {
                                let message = message.clone();
                                match catch_unwind(AssertUnwindSafe(move || {
                                    println!("message:{:?}", message);
                                    function(unsafe { &mut *state }, message)
                                })) {
                                    Ok(task) => return task.map(DynMessage::into_hot_message),
                                    Err(err) => {
                                        std::mem::forget(err);
                                        println!("Hot reloaded \"{}\" paniced", self.function_name);
                                    }
                                }
                            }
                            Err(_) => {}
                        }
                    }
                    Err(hot_message) => {
                        match unsafe {
                            lib.get_symbol::<fn(&'a mut State, HotMessage) -> Task<HotMessage>>(
                                &self.function_name.as_bytes(),
                            )
                        } {
                            Ok(function) => {
                                let state = state as *mut State;
                                match catch_unwind(AssertUnwindSafe(move || {
                                    println!("message:{:?}", hot_message);
                                    function(unsafe { &mut *state }, hot_message)
                                })) {
                                    Ok(task) => return task.map(DynMessage::into_hot_message),
                                    Err(err) => {
                                        std::mem::forget(err);
                                        println!("Hot reloaded \"{}\" paniced", self.function_name);
                                    }
                                }
                            }
                            Err(_) => {}
                        }
                    }
                }
            }
        }
        let task: Task<Message> = self
            .function
            .update(state, message.into_message().unwrap())
            .into();
        task.map(DynMessage::into_hot_message)
    }
}

impl<F, State, Message> HotFn for HotUpdate<F, State, Message>
where
    F: Update<State, Message>,
{
    fn library_name(&self) -> &'static str {
        self.lib_name
    }
}

impl<F, State, Message, C> Update<State, Message> for HotUpdate<F, State, Message>
where
    F: Fn(&mut State, Message) -> C,
    C: Into<Task<Message>>,
{
    fn update(&self, state: &mut State, message: Message) -> impl Into<Task<Message>> {
        (self.function)(state, message)
    }
}
