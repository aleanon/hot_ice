use std::{
    any::type_name,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use iced_core::window;

use crate::{
    error::HotIceError, into_result::IntoResult, lib_reloader::LibReloader, reloader::FunctionState,
};

pub trait IntoHotScaleFactor<State> {
    fn static_scale_factor(&self, state: &State, window: window::Id) -> Result<f32, HotIceError>;

    fn hot_scale_factor(
        &self,
        state: &State,
        window: window::Id,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<f32, HotIceError>;
}

impl<C, T, State> IntoHotScaleFactor<State> for T
where
    T: Fn(&State) -> C,
    C: IntoResult<f32>,
{
    fn static_scale_factor(&self, state: &State, _window: window::Id) -> Result<f32, HotIceError> {
        (self)(state).into_result()
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
            lib.get_symbol::<fn(&State) -> C>(function_name.as_bytes())
                .map_err(|_| HotIceError::FunctionNotFound(function_name))?
        };

        function(state).into_result()
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
        reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Result<(f32, FunctionState), HotIceError> {
        let Some(reloader) = reloader else {
            let scale_factor = self.function.static_scale_factor(state, window)?;
            return Ok((scale_factor, FunctionState::Static));
        };

        match self
            .function
            .hot_scale_factor(state, window, reloader, self.function_name)
        {
            Ok(scale_factor) => Ok((scale_factor, FunctionState::Hot)),
            Err(HotIceError::FunctionNotFound(_)) => {
                let scale_factor = self.function.static_scale_factor(state, window)?;
                Ok((scale_factor, FunctionState::Static))
            }
            Err(err) => Err(err),
        }
    }
}
