use std::{
    any::type_name,
    collections::HashMap,
    marker::PhantomData,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Mutex},
};

use iced_winit::runtime::Task;

use crate::{
    error::HotFunctionError, hot_fn::HotFn, lib_reloader::LibReloader, message::MessageSource,
    reloader::LIB_RELOADER, DynMessage,
};

type Reloaders = HashMap<&'static str, Arc<Mutex<LibReloader>>>;

pub trait HotUpdateTrait<State, Message> {
    fn library_name() -> &'static str {
        let type_name = std::any::type_name::<Self>();
        let mut iter = type_name.split("::");
        iter.next().unwrap_or(type_name)
    }

    fn function_name() -> &'static str {
        let type_name = std::any::type_name::<Self>();
        let iter = type_name.split("::");
        iter.last().unwrap_or(type_name)
    }

    fn static_update(&self, state: &mut State, message: Message) -> Task<Message>;

    fn hot_update(
        &self,
        state: &mut State,
        message: Message,
        reloaders: &Reloaders,
    ) -> Result<Task<Message>, HotFunctionError>;
}

impl<T, C, State, Message> HotUpdateTrait<State, Message> for T
where
    T: Fn(&mut State, Message) -> C,
    C: Into<Task<Message>>,
    Message: Send + 'static,
{
    fn static_update(&self, state: &mut State, message: Message) -> Task<Message> {
        (self)(state, message).into()
    }

    fn hot_update(
        &self,
        state: &mut State,
        message: Message,
        reloaders: &Reloaders,
    ) -> Result<Task<Message>, HotFunctionError> {
        let reloader = reloaders
            .get(Self::library_name())
            .ok_or(HotFunctionError::LibraryNotFound)?;

        let lib = reloader
            .try_lock()
            .map_err(|_| HotFunctionError::LockAcquisitionError)?;

        let function = unsafe {
            lib.get_symbol::<fn(&mut State, Message) -> C>(Self::function_name().as_bytes())
                .map_err(|_| HotFunctionError::FunctionNotFound(Self::function_name()))?
        };

        match catch_unwind(AssertUnwindSafe(move || function(state, message))) {
            Ok(task) => return Ok(task.into()),
            Err(err) => {
                std::mem::forget(err);
                return Err(HotFunctionError::FunctionPaniced(Self::function_name()));
            }
        }
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
    F: HotUpdateTrait<State, Message>,
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

    pub fn update<'a>(
        &self,
        state: &'a mut State,
        message: MessageSource<Message>,
    ) -> Task<MessageSource<Message>> {
        match message {
            MessageSource::Static(message) => self
                .function
                .static_update(state, message)
                .map(MessageSource::Static),
            MessageSource::Dynamic(message) => {
                let Some(reloaders) = LIB_RELOADER.get() else {
                    return Task::none();
                };

                self.function
                    .hot_update(state, message, reloaders)
                    .inspect_err(|e| eprintln!("{}", e))
                    .unwrap_or(Task::none())
                    .map(MessageSource::Dynamic)
            }
        }
    }
}

impl<F, State, Message> HotFn for HotUpdate<F, State, Message>
where
    F: HotUpdateTrait<State, Message>,
{
    fn library_name(&self) -> &'static str {
        self.lib_name
    }
}

// impl<F, State, Message, C> Update<State, Message> for HotUpdate<F, State, Message>
// where
//     F: Fn(&mut State, Message) -> C,
//     C: Into<Task<Message>>,
// {
//     fn update(&self, state: &mut State, message: Message) -> impl Into<Task<Message>> {
//         (self.function)(state, message)
//     }
// }
