use std::any::type_name;

use iced::Program;

use crate::reloader::LIB_RELOADER;

#[allow(type_alias_bounds)]
type Element<'a, P> = iced::Element<'a, <P as Program>::Message, <P as Program>::Message, <P as Program>::Renderer>;

#[allow(type_alias_bounds)]
type View<'a, P: Program, E: Into<Element<'a, P>>> = fn(&'a P::State) -> E; 

// pub trait HotView<'a, State, Message, Theme, Renderer> {
//     /// Produces the widget of the [`Application`].
//     fn view(
//         &self,
//         state: &'a State,
//         symbol: &'static str,
//     ) -> impl Into<iced::Element<'a, Message, Theme, Renderer>>;
// }

// impl<'a, State, Message, Theme, Renderer, P>
//     HotView<'a, State, Message, Theme, Renderer> for ViewFn<'a, P>
// where
//     P: Program + 'static,
//     State: 'static,
// {
//     fn view(
//         &self,
//         state: &'a P::State,
//     ) -> impl Into<Element<'a, P>> {
//         let lib = LIB_RELOADER.get().unwrap().try_lock().unwrap();
//         unsafe {
//             let function = lib.get_symbol::<View<'a, P>>(b"view\0").expect("symbol view not found");
//             function(state)
//         }
//     }
// }

pub struct ViewFn<'a, P, E> 
where
    P: Program,
    E: Into<Element<'a, P>> {
    pub fn_ptr: fn(&'a P::State) -> E,
    pub path: &'static str,
}

impl<'a, P, E> ViewFn<'a, P, E> 
where 
    P: Program + 'a,
    E: Into<Element<'a, P>>,
{
    pub fn new(function: View<'a, P, E>) -> Self {
        let path  = type_name::<View<'a, P, E>>();
        Self {
            fn_ptr: function,
            path,
        }
    }
    pub fn view(&self, state: &'a P::State) -> E {
        let lib = LIB_RELOADER.get().unwrap().try_lock().unwrap();
        unsafe {
            let function = lib.get_symbol::<View<'a, P, E>>(b"view\0").expect("symbol view not found");
            function(state)
        }
    }
    
}

impl<'a, P, E> Into<ViewFn<'a, P, E>> for fn(&'a P::State) -> E 
where 
    P: Program + 'static,
    E: Into<Element<'a, P>> {
    fn into(self) -> ViewFn<'a, P, E> {
        ViewFn { fn_ptr: self, path: type_name::<Self>() }
    }
}