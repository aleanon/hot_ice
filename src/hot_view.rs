use std::{
    any::type_name,
    collections::HashMap,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use iced_core::Element;

use crate::{
    error::HotFunctionError,
    hot_fn::HotFn,
    lib_reloader::LibReloader,
    message::MessageSource,
    reloader::{FunctionState, LIB_RELOADER},
};

type Reloaders = HashMap<&'static str, Arc<Mutex<LibReloader>>>;

pub trait IntoHotView<'a, State, Message, Theme, Renderer> {
    fn static_view(&self, state: &'a State) -> Element<'a, Message, Theme, Renderer>;

    fn hot_view(
        &self,
        state: &'a State,
        reloaders: &Reloaders,
        lib_name: &str,
        function_name: &'static str,
    ) -> Result<Element<'a, Message, Theme, Renderer>, HotFunctionError>;
}

impl<'a, T, C, State, Message, Theme, Renderer> IntoHotView<'a, State, Message, Theme, Renderer>
    for T
where
    State: 'static,
    T: Fn(&'a State) -> C,
    C: Into<Element<'a, Message, Theme, Renderer>>,
{
    fn static_view(&self, state: &'a State) -> Element<'a, Message, Theme, Renderer> {
        (self)(state).into()
    }

    fn hot_view(
        &self,
        state: &'a State,
        reloaders: &Reloaders,
        lib_name: &str,
        function_name: &'static str,
    ) -> Result<Element<'a, Message, Theme, Renderer>, HotFunctionError> {
        let reloader = reloaders
            .get(lib_name)
            .ok_or(HotFunctionError::LibraryNotFound)?;

        let lib = reloader
            .try_lock()
            .map_err(|_| HotFunctionError::LockAcquisitionError)?;

        let function = unsafe {
            lib.get_symbol::<fn(&'a State) -> C>(function_name.as_bytes())
                .map_err(|_| HotFunctionError::FunctionNotFound(function_name))?
        };
        Ok(function(state).into())
    }
}

pub struct HotView<F, State, Message, Theme, Renderer> {
    pub lib_name: &'static str,
    function_name: &'static str,
    function: F,
    _state: PhantomData<State>,
    _message: PhantomData<Message>,
    _theme: PhantomData<Theme>,
    _renderer: PhantomData<Renderer>,
}

impl<'a, F, State, Message, Theme, Renderer> HotView<F, State, Message, Theme, Renderer>
where
    F: IntoHotView<'a, State, Message, Theme, Renderer>,
    Renderer: iced_core::Renderer + 'a,
    Theme: 'a,
    Message: 'a,
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
            _message: PhantomData,
            _state: PhantomData,
            _theme: PhantomData,
            _renderer: PhantomData,
        }
    }

    pub fn view(
        &self,
        state: &'a State,
        fn_state: &mut FunctionState,
    ) -> Element<'a, MessageSource<Message>, Theme, Renderer> {
        let Some(reloaders) = LIB_RELOADER.get() else {
            *fn_state = FunctionState::Static;
            return self.function.static_view(state).map(MessageSource::Static);
        };

        match self
            .function
            .hot_view(state, reloaders, self.lib_name, self.function_name)
        {
            Ok(element) => {
                *fn_state = FunctionState::Hot;
                element.map(MessageSource::Dynamic)
            }
            Err(err) => {
                *fn_state = FunctionState::FallBackStatic(err.to_string());
                self.function.static_view(state).map(MessageSource::Static)
            }
        }
    }
}

impl<F, State, Message, Theme, Renderer> HotFn for HotView<F, State, Message, Theme, Renderer>
where
    F: for<'a> IntoHotView<'a, State, Message, Theme, Renderer>,
{
    fn library_name(&self) -> &'static str {
        self.lib_name
    }
}
