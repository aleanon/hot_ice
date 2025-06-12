use std::{
    any::type_name,
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

    pub fn view<'a>(&self, state: &'a State) -> Element<'a, Message, Theme, Renderer> {
        if let Some(lock) = LIB_RELOADER.get().and_then(|map| map.get(&self.lib_name)) {
            if let Ok(lib) = lock.try_lock() {
                match unsafe {
                    lib.get_symbol::<fn(&'a State) -> Element<'a, Message, Theme, Renderer>>(
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
        self.function.view(state).into()
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
{
    fn view(&self, state: &'a State) -> impl Into<Element<'a, Message, Theme, Renderer>> {
        (self.function)(state)
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
    Message: Clone,
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
            _message: PhantomData,
            _state: PhantomData,
        }
    }

    pub fn update<'a>(&self, state: &'a mut State, message: Message) -> Task<Message> {
        if let Some(lock) = LIB_RELOADER.get().and_then(|map| map.get(&self.lib_name)) {
            if let Ok(lib) = lock.try_lock() {
                match unsafe {
                    lib.get_symbol::<fn(&'a mut State, Message) -> Task<Message>>(
                        &self.function_name.as_bytes(),
                    )
                } {
                    Ok(function) => {
                        let state_ptr = state as *mut State;
                        let message_clone = message.clone();
                        match catch_unwind(AssertUnwindSafe(move || {
                            function(unsafe { &mut *state_ptr }, message_clone)
                        })) {
                            Ok(task) => return task,
                            Err(err) => {
                                std::mem::forget(err);
                                println!("Hot reloaded \"{}\" paniced", self.function_name);
                            }
                        }
                    }
                    Err(_) => {
                        println!("Unable to load function: \"{}\"", self.function_name);
                    }
                }
            }
        }
        self.function.update(state, message).into()
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
