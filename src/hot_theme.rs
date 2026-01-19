use std::{
    any::type_name,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use crate::{
    error::HotIceError, into_result::IntoResult, lib_reloader::LibReloader, reloader::FunctionState,
};

pub trait IntoHotTheme<State, Theme> {
    fn static_theme(&self, state: &State) -> Result<Option<Theme>, HotIceError>;

    fn hot_theme(
        &self,
        state: &State,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<Option<Theme>, HotIceError>;
}

impl<T, C, State, Theme> IntoHotTheme<State, Theme> for T
where
    T: Fn(&State) -> C,
    C: IntoResult<Option<Theme>>,
{
    fn static_theme(&self, state: &State) -> Result<Option<Theme>, HotIceError> {
        (self)(state).into_result()
    }

    fn hot_theme(
        &self,
        state: &State,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<Option<Theme>, HotIceError> {
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

pub struct HotTheme<F, State, Theme> {
    function_name: &'static str,
    function: F,
    _state: PhantomData<State>,
    _theme: PhantomData<Theme>,
}

impl<F, State, Theme> HotTheme<F, State, Theme>
where
    F: IntoHotTheme<State, Theme>,
{
    pub fn new(function: F) -> Self {
        let type_name = type_name::<F>();
        let iterator = type_name.split("::");
        let function_name = iterator.last().unwrap();

        Self {
            function,
            function_name,
            _state: PhantomData,
            _theme: PhantomData,
        }
    }

    pub fn theme(
        &self,
        state: &State,
        fn_state: &mut FunctionState,
        reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Option<Theme> {
        let Some(reloader) = reloader else {
            *fn_state = FunctionState::Static;
            return match self.function.static_theme(state) {
                Ok(theme) => theme,
                Err(err) => {
                    *fn_state = FunctionState::Error(err.to_string());
                    None
                }
            };
        };

        match self.function.hot_theme(state, reloader, self.function_name) {
            Ok(theme) => {
                *fn_state = FunctionState::Hot;
                theme
            }
            Err(err) => {
                log::error!("{}\nFallback to default theme", err);
                *fn_state = FunctionState::FallBackStatic(err.to_string());
                None
            }
        }
    }
}
