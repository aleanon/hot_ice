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
    Theme: theme::Base,
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
            return match self.function.static_style(state, theme) {
                Ok(style) => style,
                Err(err) => {
                    *fn_state = FunctionState::Error(err.to_string());
                    theme::Base::base(theme)
                }
            };
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
                log::error!("{}\nFallback to base style", err);
                *fn_state = FunctionState::FallBackStatic(err.to_string());
                theme::Base::base(theme)
            }
        }
    }
}
