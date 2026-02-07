use std::{
    any::type_name,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use iced_futures::Subscription;

use crate::{
    error::HotIceError, into_result::IntoResult, lib_reloader::LibReloader, message::MessageSource,
    reloader::FunctionState,
};

pub trait IntoHotSubscription<State, Message> {
    fn static_subscription(&self, state: &State) -> Result<Subscription<Message>, HotIceError>;

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
    C: IntoResult<Subscription<Message>>,
    Message: Send + 'static,
{
    fn static_subscription(&self, state: &State) -> Result<Subscription<Message>, HotIceError> {
        (self)(state).into_result()
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

        function(state).into_result()
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
        reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Result<(Subscription<MessageSource<Message>>, FunctionState), HotIceError> {
        let Some(reloader) = reloader else {
            let sub = self.function.static_subscription(state)?;
            return Ok((sub.map(MessageSource::Static), FunctionState::Static));
        };

        match self
            .function
            .hot_subscription(state, reloader, self.function_name)
        {
            Ok(sub) => Ok((sub.map(MessageSource::Dynamic), FunctionState::Hot)),
            Err(HotIceError::FunctionNotFound(_)) => {
                let sub = self.function.static_subscription(state)?;
                Ok((sub.map(MessageSource::Static), FunctionState::Static))
            }
            Err(err) => Err(err),
        }
    }
}
