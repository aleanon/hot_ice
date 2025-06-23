use std::{
    any::{type_name, TypeId},
    marker::PhantomData,
    panic::{catch_unwind, AssertUnwindSafe},
};

pub trait HotFn {
    fn library_name(&self) -> &'static str;
}

use iced_core::Element;
use iced_winit::runtime::Task;

use crate::{
    hot_ice::{Update, View},
    reloader::LIB_RELOADER,
    DynMessage, HotMessage,
};

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

pub struct HotUpdate<F, State, Message> {
    lib_name: &'static str,
    function_name: &'static str,
    function: F,
    _state: PhantomData<State>,
    _message: PhantomData<Message>,
}

impl<F, State, Message> HotUpdate<F, State, Message>
where
    Message: DynMessage + Clone,
    F: Update<State, Message>,
    State: 'static,
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
            _state: PhantomData,
            _message: PhantomData,
        }
    }

    pub fn update<'a>(&self, state: &'a mut State, message: HotMessage) -> Task<HotMessage> {
        if let Some(lock) = LIB_RELOADER.get().and_then(|map| map.get(&self.lib_name)) {
            if let Ok(lib) = lock.try_lock() {
                match unsafe {
                    lib.get_symbol::<fn(&'a mut State, Message) -> Task<Message>>(
                        &self.function_name.as_bytes(),
                    )
                } {
                    Ok(function) => {
                        let state = state as *mut State;
                        let message = message.clone();
                        match catch_unwind(AssertUnwindSafe(move || {
                            function(unsafe { &mut *state }, message.into_message())
                        })) {
                            Ok(task) => return task.map(DynMessage::into_hot_message),
                            Err(err) => {
                                std::mem::forget(err);
                                println!("Hot reloaded \"{}\" paniced", self.function_name);
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
        }
        let task: Task<Message> = self.function.update(state, message.into_message()).into();
        task.map(DynMessage::into_hot_message)
    }
}

impl<F, State, Message> HotFn for HotUpdate<F, State, Message>
where
    F: Update<State, Message>,
{
    fn library_name(&self) -> &'static str {
        self.lib_name
    }
}

impl<F, State, Message, C> Update<State, Message> for HotUpdate<F, State, Message>
where
    F: Fn(&mut State, Message) -> C,
    C: Into<Task<Message>>,
{
    fn update(&self, state: &mut State, message: Message) -> impl Into<Task<Message>> {
        (self.function)(state, message)
    }
}
