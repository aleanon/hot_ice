use iced::Program;

#[allow(type_alias_bounds)]
type Element<'a, P> = iced::Element<'a, <P as Program>::Message, <P as Program>::Message, <P as Program>::Renderer>;

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

pub struct ViewFn<'a, P: Program> {
    pub fn_ptr: fn(&'a P::State) -> Element<'a, P>,
    pub path: &'static str,
}

impl<P> Into<ViewFn<'a, P>> for Fn(&P::State) -> T {
    
}