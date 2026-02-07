use std::{
    any::type_name,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use iced_core::theme;

use crate::{
    error::HotIceError, into_result::IntoResult, lib_reloader::LibReloader, reloader::FunctionState,
};

pub trait IntoHotStyle<State, Theme> {
    fn static_style(&self, state: &State, theme: &Theme) -> Result<theme::Style, HotIceError>;

    fn hot_style(
        &self,
        state: &State,
        theme: &Theme,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<theme::Style, HotIceError>;
}

impl<T, C, State, Theme> IntoHotStyle<State, Theme> for T
where
    T: Fn(&State, &Theme) -> C,
    C: IntoResult<theme::Style>,
{
    fn static_style(&self, state: &State, theme: &Theme) -> Result<theme::Style, HotIceError> {
        (self)(state, theme).into_result()
    }

    fn hot_style(
        &self,
        state: &State,
        theme: &Theme,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<theme::Style, HotIceError> {
        let lib = reloader
            .try_lock()
            .map_err(|_| HotIceError::LockAcquisitionError)?;

        let function = unsafe {
            lib.get_symbol::<fn(&State, &Theme) -> C>(function_name.as_bytes())
                .map_err(|_| HotIceError::FunctionNotFound(function_name))?
        };

        function(state, theme).into_result()
    }
}

pub struct HotStyle<F, State, Theme> {
    function_name: &'static str,
    function: F,
    _state: PhantomData<State>,
    _theme: PhantomData<Theme>,
}

impl<F, State, Theme> HotStyle<F, State, Theme>
where
    F: IntoHotStyle<State, Theme>,
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

    pub fn style(
        &self,
        state: &State,
        theme: &Theme,
        reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Result<(theme::Style, FunctionState), HotIceError> {
        let Some(reloader) = reloader else {
            let style = self.function.static_style(state, theme)?;
            return Ok((style, FunctionState::Static));
        };

        match self
            .function
            .hot_style(state, theme, reloader, self.function_name)
        {
            Ok(style) => Ok((style, FunctionState::Hot)),
            Err(HotIceError::FunctionNotFound(_)) => {
                let style = self.function.static_style(state, theme)?;
                Ok((style, FunctionState::Static))
            }
            Err(err) => Err(err),
        }
    }
}
