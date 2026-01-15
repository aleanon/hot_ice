use std::{
    any::type_name,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

use iced_core::window;

use crate::{error::HotIceError, lib_reloader::LibReloader, reloader::FunctionState};

pub trait IntoHotTitle<State> {
    fn static_title(&self, state: &State, window: window::Id) -> String;

    fn hot_title(
        &self,
        state: &State,
        window: window::Id,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<String, HotIceError>;
}

impl IntoHotTitle<()> for &'static str {
    fn static_title(&self, _state: &(), _window: window::Id) -> String {
        self.to_string()
    }

    fn hot_title(
        &self,
        _state: &(),
        _window: window::Id,
        _reloader: &Arc<Mutex<LibReloader>>,
        _function_name: &'static str,
    ) -> Result<String, HotIceError> {
        Ok(self.to_string())
    }
}

impl<T, State> IntoHotTitle<State> for T
where
    T: Fn(&State) -> String,
{
    fn static_title(&self, state: &State, _window: window::Id) -> String {
        (self)(state)
    }

    fn hot_title(
        &self,
        state: &State,
        _window: window::Id,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<String, HotIceError> {
        let lib = reloader
            .try_lock()
            .map_err(|_| HotIceError::LockAcquisitionError)?;

        let function = unsafe {
            lib.get_symbol::<fn(&State) -> String>(function_name.as_bytes())
                .map_err(|_| HotIceError::FunctionNotFound(function_name))?
        };

        match catch_unwind(AssertUnwindSafe(|| function(state))) {
            Ok(title) => Ok(title),
            Err(err) => {
                std::mem::forget(err);
                Err(HotIceError::FunctionPaniced(function_name))
            }
        }
    }
}

pub struct HotTitle<F, State> {
    function_name: &'static str,
    function: F,
    _state: PhantomData<State>,
}

impl<F, State> HotTitle<F, State>
where
    F: IntoHotTitle<State>,
{
    pub fn new(function: F) -> Self {
        let type_name = type_name::<F>();
        let iterator = type_name.split("::");
        let function_name = iterator.last().unwrap();

        Self {
            function,
            function_name,
            _state: PhantomData,
        }
    }

    pub fn title(
        &self,
        state: &State,
        window: window::Id,
        fn_state: &mut FunctionState,
        reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> String {
        let Some(reloader) = reloader else {
            *fn_state = FunctionState::Static;
            return self.function.static_title(state, window);
        };

        match self
            .function
            .hot_title(state, window, reloader, self.function_name)
        {
            Ok(title) => {
                *fn_state = FunctionState::Hot;
                title
            }
            Err(err) => {
                log::error!("hot_title(): {}", err);
                *fn_state = FunctionState::FallBackStatic(err.to_string());
                self.function.static_title(state, window)
            }
        }
    }
}
