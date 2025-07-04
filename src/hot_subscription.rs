use std::{
    any::type_name,
    collections::HashMap,
    marker::PhantomData,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Mutex},
};

use iced_futures::Subscription;

use crate::{
    error::HotFunctionError,
    lib_reloader::LibReloader,
    message::MessageSource,
    reloader::LIB_RELOADER,
    unsafe_ref_mut::{UnsafeMover, UnsafeRef, UnsafeRefMut},
};

type Reloaders = HashMap<&'static str, Arc<Mutex<LibReloader>>>;

pub trait IntoHotSubscription<State, Message> {
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

    fn static_subscription(&self, state: &State) -> Subscription<Message>;

    fn hot_subscription(
        &self,
        state: &State,
        reloaders: &Reloaders,
    ) -> Result<Subscription<Message>, HotFunctionError>;
}

impl<T, State, Message> IntoHotSubscription<State, Message> for T
where
    T: Fn(&State) -> Subscription<Message>,
    Message: Send + 'static,
{
    fn static_subscription(&self, state: &State) -> Subscription<Message> {
        (self)(state)
    }

    fn hot_subscription(
        &self,
        state: &State,
        reloaders: &Reloaders,
    ) -> Result<Subscription<Message>, HotFunctionError> {
        let reloader = reloaders
            .get(Self::library_name())
            .ok_or(HotFunctionError::LibraryNotFound)?;

        let lib = reloader
            .try_lock()
            .map_err(|_| HotFunctionError::LockAcquisitionError)?;

        let function = unsafe {
            lib.get_symbol::<fn(&State) -> Subscription<Message>>(Self::function_name().as_bytes())
                .map_err(|_| HotFunctionError::FunctionNotFound(Self::function_name()))?
        };

        match catch_unwind(AssertUnwindSafe(|| function(state))) {
            Ok(sub) => Ok(sub),
            Err(err) => {
                std::mem::forget(err);
                Err(HotFunctionError::FunctionPaniced(Self::function_name()))
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

    pub fn subscription(&self, state: &State) -> Subscription<MessageSource<Message>> {
        let Some(reloaders) = LIB_RELOADER.get() else {
            return self
                .function
                .static_subscription(state)
                .map(MessageSource::Static);
        };

        match self.function.hot_subscription(state, reloaders) {
            Ok(task) => task.map(MessageSource::Dynamic),
            Err(e) => {
                eprintln!("{}", e);
                self.function
                    .static_subscription(state)
                    .map(MessageSource::Static)
            }
        }
    }
}
