use std::{
    any::type_name,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

use iced_core::theme;

use crate::{error::HotIceError, lib_reloader::LibReloader, reloader::FunctionState};

pub trait IntoHotStyle<State, Theme> {
    fn static_style(&self, state: &State, theme: &Theme) -> theme::Style;

    fn hot_style(
        &self,
        state: &State,
        theme: &Theme,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<theme::Style, HotIceError>;
}

impl<T, State, Theme> IntoHotStyle<State, Theme> for T
where
    T: Fn(&State, &Theme) -> theme::Style,
{
    fn static_style(&self, state: &State, theme: &Theme) -> theme::Style {
        (self)(state, theme)
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
            lib.get_symbol::<fn(&State, &Theme) -> theme::Style>(function_name.as_bytes())
                .map_err(|_| HotIceError::FunctionNotFound(function_name))?
        };

        match catch_unwind(AssertUnwindSafe(|| function(state, theme))) {
            Ok(style) => Ok(style),
            Err(err) => {
                std::mem::forget(err);
                Err(HotIceError::FunctionPaniced(function_name))
            }
        }
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
        fn_state: &mut FunctionState,
        reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> theme::Style {
        let Some(reloader) = reloader else {
            *fn_state = FunctionState::Static;
            return self.function.static_style(state, theme);
        };

        match self
            .function
            .hot_style(state, theme, reloader, self.function_name)
        {
            Ok(style) => {
                *fn_state = FunctionState::Hot;
                style
            }
            Err(err) => {
                log::error!("style(): {}", err);
                *fn_state = FunctionState::FallBackStatic(err.to_string());
                self.function.static_style(state, theme)
            }
        }
    }
}
