use std::{
    any::type_name,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

use iced_core::window;

use crate::{error::HotIceError, lib_reloader::LibReloader, reloader::FunctionState};

pub trait IntoHotScaleFactor<State> {
    fn static_scale_factor(&self, state: &State, window: window::Id) -> f32;

    fn hot_scale_factor(
        &self,
        state: &State,
        window: window::Id,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<f32, HotIceError>;
}

impl<T, State> IntoHotScaleFactor<State> for T
where
    T: Fn(&State) -> f32,
{
    fn static_scale_factor(&self, state: &State, _window: window::Id) -> f32 {
        (self)(state)
    }

    fn hot_scale_factor(
        &self,
        state: &State,
        _window: window::Id,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<f32, HotIceError> {
        let lib = reloader
            .try_lock()
            .map_err(|_| HotIceError::LockAcquisitionError)?;

        let function = unsafe {
            lib.get_symbol::<fn(&State) -> f32>(function_name.as_bytes())
                .map_err(|_| HotIceError::FunctionNotFound(function_name))?
        };

        match catch_unwind(AssertUnwindSafe(|| function(state))) {
            Ok(scale_factor) => Ok(scale_factor),
            Err(err) => {
                std::mem::forget(err);
                Err(HotIceError::FunctionPaniced(function_name))
            }
        }
    }
}

pub struct HotScaleFactor<F, State> {
    function_name: &'static str,
    function: F,
    _state: PhantomData<State>,
}

impl<F, State> HotScaleFactor<F, State>
where
    F: IntoHotScaleFactor<State>,
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

    pub fn scale_factor(
        &self,
        state: &State,
        window: window::Id,
        fn_state: &mut FunctionState,
        reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> f32 {
        let Some(reloader) = reloader else {
            *fn_state = FunctionState::Static;
            return self.function.static_scale_factor(state, window);
        };

        match self
            .function
            .hot_scale_factor(state, window, reloader, self.function_name)
        {
            Ok(scale_factor) => {
                *fn_state = FunctionState::Hot;
                scale_factor
            }
            Err(err) => {
                log::error!("scale_factor(): {}", err);
                *fn_state = FunctionState::FallBackStatic(err.to_string());
                self.function.static_scale_factor(state, window)
            }
        }
    }
}
