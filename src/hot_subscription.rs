use std::{
    any::type_name,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

use iced_futures::Subscription;

use crate::{
    error::HotIceError, lib_reloader::LibReloader, message::MessageSource, reloader::FunctionState,
};

pub trait IntoHotSubscription<State, Message> {
    fn static_subscription(&self, state: &State) -> Subscription<Message>;

    fn hot_subscription(
        &self,
        state: &State,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<Subscription<Message>, HotIceError>;
}

impl<T, C, State, Message> IntoHotSubscription<State, Message> for T
where
    T: Fn(&State) -> C,
    C: Into<Subscription<Message>>,
    Message: Send + 'static,
{
    fn static_subscription(&self, state: &State) -> Subscription<Message> {
        (self)(state).into()
    }

    fn hot_subscription(
        &self,
        state: &State,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<Subscription<Message>, HotIceError> {
        let lib = reloader
            .try_lock()
            .map_err(|_| HotIceError::LockAcquisitionError)?;

        let function = unsafe {
            lib.get_symbol::<fn(&State) -> C>(function_name.as_bytes())
                .map_err(|_| HotIceError::FunctionNotFound(function_name))?
        };

        match catch_unwind(AssertUnwindSafe(|| function(state))) {
            Ok(sub) => Ok(sub.into()),
            Err(err) => {
                std::mem::forget(err);
                Err(HotIceError::FunctionPaniced(function_name))
            }
        }
    }
}

pub struct HotSubscription<F, State, Message> {
    function_name: &'static str,
    function: F,
    _state: PhantomData<State>,
    _message: PhantomData<Message>,
}

impl<F, State, Message> HotSubscription<F, State, Message>
where
    F: IntoHotSubscription<State, Message>,
    Message: 'static,
{
    pub fn new(function: F) -> Self {
        let type_name = type_name::<F>();
        let iterator = type_name.split("::");
        let function_name = iterator.last().unwrap();

        Self {
            function,
            function_name,
            _state: PhantomData,
            _message: PhantomData,
        }
    }

    pub fn subscription(
        &self,
        state: &State,
        fn_state: &mut FunctionState,
        reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Subscription<MessageSource<Message>> {
        let Some(reloader) = reloader else {
            *fn_state = FunctionState::Static;
            return self
                .function
                .static_subscription(state)
                .map(MessageSource::Static);
        };

        match self
            .function
            .hot_subscription(state, reloader, self.function_name)
        {
            Ok(task) => {
                *fn_state = FunctionState::Hot;
                task.map(MessageSource::Dynamic)
            }
            Err(err) => {
                log::error!("subscription(): {}", err);
                *fn_state = FunctionState::FallBackStatic(err.to_string());
                self.function
                    .static_subscription(state)
                    .map(MessageSource::Static)
            }
        }
    }
}
