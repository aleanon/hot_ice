use std::{
    any::type_name,
    fmt::Debug,
    marker::PhantomData,
    panic::{catch_unwind, AssertUnwindSafe},
};

use iced_core::Element;

use crate::{hot_fn::HotFn, reloader::LIB_RELOADER, DynMessage, HotMessage};

/// The view logic of some [`Application`].
///
/// This trait allows the [`application`] builder to take any closure that
/// returns any `Into<Element<'_, Message>>`.
pub trait View<'a, State, Message, Theme, Renderer> {
    /// Produces the widget of the [`Application`].
    fn view(&self, state: &'a State) -> Element<'a, Message, Theme, Renderer>;
}

impl<'a, T, State, Message, Theme, Renderer, Widget> View<'a, State, Message, Theme, Renderer> for T
where
    T: Fn(&'a State) -> Widget,
    State: 'static,
    Widget: Into<Element<'a, Message, Theme, Renderer>>,
    Message: Send + Debug + Clone + 'static,
    Theme: 'a,
    Renderer: iced_core::Renderer + 'a,
{
    fn view(&self, state: &'a State) -> Element<'a, Message, Theme, Renderer> {
        self(state).into()
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

impl<F, State, Message, Theme, Renderer> HotView<F, State, Message, Theme, Renderer>
where
    F: for<'a> View<'a, State, Message, Theme, Renderer>,
    State: 'static,
    Message: DynMessage,
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

    pub fn view<'a>(&self, state: &'a State) -> Element<'a, Message, Theme, Renderer>
// where
    //     Renderer: iced_core::Renderer + 'a,
    //     Theme: 'a,
    {
        if let Some(lock) = LIB_RELOADER.get().and_then(|map| map.get(&self.lib_name)) {
            if let Ok(lib) = lock.try_lock() {
                match unsafe {
                    lib.get_symbol::<fn(&State) -> Element<Message, Theme, Renderer>>(
                        &self.function_name.as_bytes(),
                    )
                } {
                    Ok(view) => match catch_unwind(AssertUnwindSafe(|| view(state))) {
                        Ok(element) => return element,
                        Err(_) => {
                            println!("Hot reloaded \"{}\" paniced", self.function_name);
                        }
                    },
                    Err(_) => {
                        println!("Unable to load function \"{}\"", self.function_name);
                    }
                }
            }
        }
        self.function.view(state)
    }
}

impl<F, State, Message, Theme, Renderer> HotFn for HotView<F, State, Message, Theme, Renderer>
where
    F: for<'a> View<'a, State, Message, Theme, Renderer>,
{
    fn library_name(&self) -> &'static str {
        self.lib_name
    }
}

impl<'a, F, State, Message, Theme, Renderer, Widget> View<'a, State, Message, Theme, Renderer>
    for HotView<F, State, Message, Theme, Renderer>
where
    F: Fn(&'a State) -> Widget,
    State: 'static,
    Widget: Into<Element<'a, Message, Theme, Renderer>>,
    Message: Send + std::fmt::Debug + Clone + 'static,
    Renderer: iced_core::Renderer + 'a,
    Theme: 'a,
{
    fn view(&self, state: &'a State) -> Element<'a, Message, Theme, Renderer> {
        (self.function)(state).into()
    }
}

// pub trait HotViewTrait<'a, State, Message, Theme, Renderer> {
//     fn hotview(&self, state: &'a State) -> Element<'a, HotMessage, Theme, Renderer>;
// }

// impl<'a, F, State, Message, Theme, Renderer, Widget>
//     HotViewTrait<'a, State, Message, Theme, Renderer> for F
// where
//     F: Fn(&'a State) -> Widget,
//     State: 'static,
//     Widget: Into<Element<'a, Message, Theme, Renderer>>,
//     Message: Into<HotMessage> + TryFrom<HotMessage> + 'a,
//     Renderer: iced_core::Renderer + 'a,
//     Theme: 'a,
// {
//     fn hotview(&self, state: &'a State, library: ) -> Element<'a, HotMessage, Theme, Renderer> {
//         let element: Element<'a, Message, Theme, Renderer> = (self)(state).into();
//         element.map(|m| m.into())
//     }
// }
