

use std::{any::type_name, marker::PhantomData};

use iced::application::View;
use libloading::{Library, Symbol};






struct Reloadable<State, Message, Theme, Renderer, Boot, Update, View> {
    boot: Boot,
    update: Update,
    view: View,
    _state: PhantomData<State>,
    _message: PhantomData<Message>,
    _theme: PhantomData<Theme>,
    _renderer: PhantomData<Renderer>,
}



pub struct HotView<'a, View, State, Message, Theme, Renderer, Widget> 
where 
    View: Fn(&'a State) -> Widget,
    State: 'static,
    Widget: Into<iced::Element<'a, Message, Theme, Renderer>> {
    function_ptr: View,
    path: &'static str,
    _state: PhantomData<&'a State>,
    _message: PhantomData<Message>,
    _theme: PhantomData<Theme>,
    _renderer: PhantomData<Renderer>,
}

impl<'a, View, State, Message, Theme, Renderer, Widget> HotView<'a, View, State, Message, Theme, Renderer, Widget> 
where 
    View: Fn(&'a State) -> Widget,
    State: 'static,
    Widget: Into<iced::Element<'a, Message, Theme, Renderer>> {
    pub fn new(function: View) -> HotView<'a, View, State, Message, Theme, Renderer, Widget> {
        Self {
            function_ptr: function,
            path: type_name::<View>(),
            _message: PhantomData,
            _renderer: PhantomData,
            _state: PhantomData,
            _theme: PhantomData,
        }
    }

    // pub fn update_function_pointer(&mut self, library: Library) {
    //     let symbol= unsafe{library.get::<View>(&self.path.as_bytes()).unwrap()};

    //     self.function_ptr = *symbol;
    // }
}

pub struct DynamicFunctions<'a, View, State, Message, Theme, Renderer, Widget> 
where  
    View: Fn(&'a State) -> Widget,
    State: 'static,
    Widget: Into<iced::Element<'a, Message, Theme, Renderer>> {
    library: Option<Library>,
    view: HotView<'a, View, State, Message, Theme, Renderer, Widget>,
}

impl<'a, View, State, Message, Theme, Renderer, Widget> DynamicFunctions<'a, View, State, Message, Theme, Renderer, Widget> 
where 
    View: Fn(&'a State) -> Widget,
    State: 'static,
    Widget: Into<iced::Element<'a, Message, Theme, Renderer>> {
    pub fn new(hot_view: HotView<'a, View, State, Message, Theme, Renderer, Widget>) -> Self {
        Self {
            library: None,
            view: hot_view
        }
    }
}

// pub trait HotView<'a, State, Message, Theme, Renderer> {
//     /// Produces the widget of the [`Application`].
//     fn view(
//         &self,
//         state: &'a State,
//     ) -> impl Into<Element<'a, Message, Theme, Renderer>>;
// }

// impl<'a, T, State, Message, Theme, Renderer, Widget>
//     HotView<'a, State, Message, Theme, Renderer> for T
// where
//     T: Fn(&'a State) -> Widget,
//     State: 'static,
//     Widget: Into<Element<'a, Message, Theme, Renderer>>,
// {
//     fn view(
//         &self,
//         state: &'a State,
//     ) -> impl Into<Element<'a, Message, Theme, Renderer>> {
//         self(state)
//     }
// }