use std::{
    any::type_name,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use iced_winit::runtime::Task;

use crate::{
    error::HotIceError, lib_reloader::LibReloader, message::DynMessage, message::MessageSource,
    reloader::FunctionState,
};

trait IntoResult<Message> {
    fn into_result(self) -> Result<Task<Message>, HotIceError>;
}

impl<Message> IntoResult<Message> for Task<Message> {
    fn into_result(self) -> Result<Task<Message>, HotIceError> {
        Ok(self)
    }
}

impl<Message> IntoResult<Message> for Result<Task<Message>, HotIceError> {
    fn into_result(self) -> Result<Task<Message>, HotIceError> {
        self
    }
}

pub trait IntoHotUpdate<State, Message> {
    fn static_update(
        &self,
        state: &mut State,
        message: Message,
    ) -> Result<Task<Message>, HotIceError>;

    fn hot_update(
        &self,
        state: &mut State,
        message: Message,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<Task<Message>, HotIceError>;
}

impl<T, C, State, Message> IntoHotUpdate<State, Message> for T
where
    T: Fn(&mut State, Message) -> C,
    C: IntoResult<Message>,
    Message: Send + 'static,
    State: Send + 'static,
{
    fn static_update(
        &self,
        state: &mut State,
        message: Message,
    ) -> Result<Task<Message>, HotIceError> {
        (self)(state, message).into_result()
    }

    fn hot_update(
        &self,
        state: &mut State,
        message: Message,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<Task<Message>, HotIceError> {
        let lib = reloader
            .try_lock()
            .map_err(|_| HotIceError::LockAcquisitionError)?;

        let function = unsafe {
            lib.get_symbol::<fn(&mut State, Message) -> C>(function_name.as_bytes())
                .map_err(|_| HotIceError::FunctionNotFound(function_name))?
        };

        function(state, message).into_result()
    }
}

pub struct HotUpdate<F, State, Message> {
    pub lib_name: &'static str,
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

    fn run_static(
        &self,
        state: &mut State,
        message: Message,
        fn_state: &mut FunctionState,
        new_fn_state: FunctionState,
    ) -> Task<MessageSource<Message>> {
        let result = self
            .function
            .static_update(state, message)
            .map(|t| t.map(MessageSource::Static));

        match result {
            Ok(task) => {
                *fn_state = new_fn_state;
                task
            }
            Err(err) => {
                *fn_state = FunctionState::Error(err.to_string());
                Task::none()
            }
        }
    }

    pub fn update<'a>(
        &self,
        state: &'a mut State,
        message: MessageSource<Message>,
        fn_state: &mut FunctionState,
        reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Task<MessageSource<Message>> {
        match message {
            MessageSource::Static(message) => {
                self.run_static(state, message, fn_state, FunctionState::Static)
            }
            MessageSource::Dynamic(message) => {
                let Some(reloader) = reloader else {
                    return self.run_static(state, message, fn_state, FunctionState::Static);
                };

                match self
                    .function
                    .hot_update(state, message.clone(), reloader, self.function_name)
                {
                    Ok(task) => {
                        *fn_state = FunctionState::Hot;
                        task.map(MessageSource::Dynamic)
                    }
                    Err(err) => {
                        match err {
                            HotIceError::FunctionNotFound(_) => {
                                return match self.function.static_update(state, message) {
                                    Ok(task) => {
                                        *fn_state = FunctionState::Static;
                                        task.map(MessageSource::Static)
                                    }
                                    Err(err) => {
                                        *fn_state = FunctionState::Error(err.to_string());
                                        Task::none()
                                    }
                                };
                            }
                            _ => {}
                        }
                        log::error!("update():{}", err);
                        *fn_state = FunctionState::Error(err.to_string());
                        Task::none()
                    }
                }
            }
        }
    }
}
