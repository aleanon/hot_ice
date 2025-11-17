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

pub trait IntoHotUpdate<State, Message> {
    fn static_update(&self, state: &mut State, message: Message) -> Task<Message>;

    fn hot_update(
        &self,
        state: &mut State,
        message: Message,
        reloaders: &Reloaders,
        lib_name: &str,
        function_name: &'static str,
    ) -> Result<Task<Message>, HotFunctionError>;
}

impl<T, C, State, Message> IntoHotUpdate<State, Message> for T
where
    T: Fn(&mut State, Message) -> C,
    C: Into<Task<Message>> + 'static,
    Message: Send + 'static,
    State: Send + 'static,
{
    fn static_update(&self, state: &mut State, message: Message) -> Task<Message> {
        (self)(state, message).into()
    }

    fn hot_update(
        &self,
        state: &mut State,
        message: Message,
        reloaders: &Reloaders,
        lib_name: &str,
        function_name: &'static str,
    ) -> Result<Task<Message>, HotFunctionError> {
        let reloader = reloaders
            .get(lib_name)
            .ok_or(HotFunctionError::LibraryNotFound)?;

        let lib = reloader
            .try_lock()
            .map_err(|_| HotFunctionError::LockAcquisitionError)?;

        let function = unsafe {
            lib.get_symbol::<fn(&mut State, Message) -> C>(function_name.as_bytes())
                .map_err(|_| HotFunctionError::FunctionNotFound(function_name))?
        };

        match catch_unwind(AssertUnwindSafe(|| function(state, message))) {
            Ok(sub) => Ok(sub.into()),
            Err(err) => {
                std::mem::forget(err);
                Err(HotFunctionError::FunctionPaniced(function_name))
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
    F: IntoHotUpdate<State, Message>,
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
                    return self
                        .function
                        .static_update(state, message)
                        .map(MessageSource::Static);
                };

                match self.function.hot_update(
                    state,
                    message.clone(),
                    reloaders,
                    self.lib_name,
                    self.function_name,
                ) {
                    Ok(task) => task.map(MessageSource::Dynamic),
                    Err(e) => {
                        eprintln!("{}", e);
                        self.function
                            .static_update(state, message)
                            .map(MessageSource::Static)
                    }
                }
            }
        }
    }
}

impl<F, State, Message> HotFn for HotUpdate<F, State, Message>
where
    F: IntoHotUpdate<State, Message>,
{
    fn library_name(&self) -> &'static str {
        self.lib_name
    }
}
