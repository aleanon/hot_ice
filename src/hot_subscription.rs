use std::{
    any::type_name,
    collections::HashMap,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

use iced_futures::Subscription;

use crate::{
    error::HotFunctionError,
    lib_reloader::LibReloader,
    message::MessageSource,
    reloader::{FunctionState, LIB_RELOADER},
};

type Reloaders = HashMap<&'static str, Arc<Mutex<LibReloader>>>;

pub trait IntoHotSubscription<State, Message> {
    fn static_subscription(&self, state: &State) -> Subscription<Message>;

    fn hot_subscription(
        &self,
        state: &State,
        reloaders: &Reloaders,
        lib_name: &str,
        function_name: &'static str,
    ) -> Result<Subscription<Message>, HotFunctionError>;
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
        reloaders: &Reloaders,
        lib_name: &str,
        function_name: &'static str,
    ) -> Result<Subscription<Message>, HotFunctionError> {
        let reloader = reloaders
            .get(lib_name)
            .ok_or(HotFunctionError::LibraryNotFound)?;

        let lib = reloader
            .try_lock()
            .map_err(|_| HotFunctionError::LockAcquisitionError)?;

        let function = unsafe {
            lib.get_symbol::<fn(&State) -> C>(function_name.as_bytes())
                .map_err(|_| HotFunctionError::FunctionNotFound(function_name))?
        };

        match catch_unwind(AssertUnwindSafe(|| function(state))) {
            Ok(sub) => Ok(sub.into()),
            Err(err) => {
                std::mem::forget(err);
                Err(HotFunctionError::FunctionPaniced(function_name))
            }
        }
    }
}

pub struct HotSubscription<F, State, Message> {
    lib_name: &'static str,
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

    pub fn subscription(
        &self,
        state: &State,
        fn_state: &mut FunctionState,
    ) -> Subscription<MessageSource<Message>> {
        let Some(reloaders) = LIB_RELOADER.get() else {
            *fn_state = FunctionState::Static;
            return self
                .function
                .static_subscription(state)
                .map(MessageSource::Static);
        };

        match self
            .function
            .hot_subscription(state, reloaders, self.lib_name, self.function_name)
        {
            Ok(task) => {
                *fn_state = FunctionState::Hot;
                task.map(MessageSource::Dynamic)
            }
            Err(err) => {
                *fn_state = FunctionState::FallBackStatic(err.to_string());
                self.function
                    .static_subscription(state)
                    .map(MessageSource::Static)
            }
        }
    }
}
