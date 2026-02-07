use std::{
    any::type_name,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use iced_core::Element;

use crate::{
    error::HotIceError, into_result::IntoResult, lib_reloader::LibReloader, message::MessageSource,
    reloader::FunctionState,
};

pub trait IntoHotView<'a, State, Message, Theme, Renderer> {
    fn static_view(
        &self,
        state: &'a State,
    ) -> Result<Element<'a, Message, Theme, Renderer>, HotIceError>;

    fn hot_view(
        &self,
        state: &'a State,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<Element<'a, Message, Theme, Renderer>, HotIceError>;
}

impl<'a, T, C, State, Message, Theme, Renderer> IntoHotView<'a, State, Message, Theme, Renderer>
    for T
where
    State: 'static,
    T: Fn(&'a State) -> C,
    C: IntoResult<Element<'a, Message, Theme, Renderer>>,
{
    fn static_view(
        &self,
        state: &'a State,
    ) -> Result<Element<'a, Message, Theme, Renderer>, HotIceError> {
        (self)(state).into_result()
    }

    fn hot_view(
        &self,
        state: &'a State,
        reloader: &Arc<Mutex<LibReloader>>,
        function_name: &'static str,
    ) -> Result<Element<'a, Message, Theme, Renderer>, HotIceError> {
        let lib = reloader
            .try_lock()
            .map_err(|_| HotIceError::LockAcquisitionError)?;

        let function = unsafe {
            lib.get_symbol::<fn(&'a State) -> C>(function_name.as_bytes())
                .map_err(|_| HotIceError::FunctionNotFound(function_name))?
        };
        function(state).into_result()
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
    Renderer: iced_core::Renderer + iced_core::text::Renderer + 'a,
    Theme: iced_widget::text::Catalog + iced_widget::container::Catalog + 'a,
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
        reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Result<
        (
            Element<'a, MessageSource<Message>, Theme, Renderer>,
            FunctionState,
        ),
        HotIceError,
    > {
        let Some(reloader) = reloader else {
            let element = self.function.static_view(state)?;
            return Ok((element.map(MessageSource::Static), FunctionState::Static));
        };

        match self.function.hot_view(state, reloader, self.function_name) {
            Ok(element) => Ok((element.map(MessageSource::Dynamic), FunctionState::Hot)),
            Err(HotIceError::FunctionNotFound(_)) => {
                let element = self.function.static_view(state)?;
                Ok((element.map(MessageSource::Static), FunctionState::Static))
            }
            Err(err) => Err(err),
        }
    }
}
