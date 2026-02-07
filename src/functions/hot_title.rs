use std::{
    any::type_name,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use iced_core::window;

use crate::{
    error::HotIceError, into_result::IntoResult, lib_reloader::LibReloader, reloader::FunctionState,
};

pub trait IntoHotTitle<State> {
    fn static_title(&self, state: &State, window: window::Id) -> Result<String, HotIceError>;

    fn hot_title(
        &self,
        state: &State,
        window: window::Id,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<String, HotIceError>;
}

impl IntoHotTitle<()> for &'static str {
    fn static_title(&self, _state: &(), _window: window::Id) -> Result<String, HotIceError> {
        Ok(self.to_string())
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

impl<T, C, State> IntoHotTitle<State> for T
where
    T: Fn(&State) -> C,
    C: IntoResult<String>,
{
    fn static_title(&self, state: &State, _window: window::Id) -> Result<String, HotIceError> {
        (self)(state).into_result()
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
            lib.get_symbol::<fn(&State) -> C>(function_name.as_bytes())
                .map_err(|_| HotIceError::FunctionNotFound(function_name))?
        };

        function(state).into_result()
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
        reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Result<(String, FunctionState), HotIceError> {
        let Some(reloader) = reloader else {
            let title = self.function.static_title(state, window)?;
            return Ok((title, FunctionState::Static));
        };

        match self
            .function
            .hot_title(state, window, reloader, self.function_name)
        {
            Ok(title) => Ok((title, FunctionState::Hot)),
            Err(HotIceError::FunctionNotFound(_)) => {
                let title = self.function.static_title(state, window)?;
                Ok((title, FunctionState::Static))
            }
            Err(err) => Err(err),
        }
    }
}
