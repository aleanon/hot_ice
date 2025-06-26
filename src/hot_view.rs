use std::{
    any::type_name,
    collections::HashMap,
    marker::PhantomData,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Mutex},
};

use iced_core::Element;

use crate::{
    error::HotFunctionError, hot_fn::HotFn, lib_reloader::LibReloader, message::MessageSource,
    reloader::LIB_RELOADER,
};

type Reloaders = HashMap<&'static str, Arc<Mutex<LibReloader>>>;

pub trait HotViewTrait<'a, State, Message, Theme, Renderer> {
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

    fn static_view(&self, state: &'a State) -> Element<'a, Message, Theme, Renderer>;

    fn hot_view(
        &self,
        state: &'a State,
        reloaders: &Reloaders,
    ) -> Result<Element<'a, Message, Theme, Renderer>, HotFunctionError>;
}

impl<'a, T, C, State, Message, Theme, Renderer> HotViewTrait<'a, State, Message, Theme, Renderer>
    for T
where
    State: 'a,
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
    ) -> Result<Element<'a, Message, Theme, Renderer>, HotFunctionError> {
        let reloader = reloaders
            .get(Self::library_name())
            .ok_or(HotFunctionError::LibraryNotFound)?;

        let lib = reloader
            .try_lock()
            .map_err(|_| HotFunctionError::LockAcquisitionError)?;

        let function = unsafe {
            lib.get_symbol::<fn(&'a State) -> C>(Self::function_name().as_bytes())
                .map_err(|_| HotFunctionError::FunctionNotFound(Self::function_name()))?
        };

        match catch_unwind(AssertUnwindSafe(move || function(state))) {
            Ok(element) => return Ok(element.into()),
            Err(err) => {
                std::mem::forget(err);
                return Err(HotFunctionError::FunctionPaniced(Self::function_name()));
            }
        }
    }
}

pub struct HotView<F, State, Message, Theme, Renderer> {
    lib_name: &'static str,
    function_name: &'static str,
    function: F,
    _state: PhantomData<State>,
    _message: PhantomData<Message>,
    _theme: PhantomData<Theme>,
    _renderer: PhantomData<Renderer>,
}

impl<'a, F, State, Message, Theme, Renderer> HotView<F, State, Message, Theme, Renderer>
where
    F: HotViewTrait<'a, State, Message, Theme, Renderer>,
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

    pub fn view(&self, state: &'a State) -> Element<'a, MessageSource<Message>, Theme, Renderer> {
        let Some(reloaders) = LIB_RELOADER.get() else {
            return self.function.static_view(state).map(MessageSource::Static);
        };

        match self.function.hot_view(state, reloaders) {
            Ok(element) => element.map(MessageSource::Dynamic),
            Err(err) => {
                eprintln!("{}", err);
                self.function.static_view(state).map(MessageSource::Static)
            }
        }
    }
}

impl<F, State, Message, Theme, Renderer> HotFn for HotView<F, State, Message, Theme, Renderer>
where
    F: for<'a> HotViewTrait<'a, State, Message, Theme, Renderer>,
{
    fn library_name(&self) -> &'static str {
        self.lib_name
    }
}
