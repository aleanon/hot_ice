use std::{collections::HashMap, fmt::Debug, fs::File, path::PathBuf, sync::{Arc, Mutex}, time::Duration};

// use ferrishot_iced_core as iced_core;
// use ferrishot_iced_futures as iced_futures;
// use ferrishot_iced_widget as iced_widget;
// use ferrishot_iced_winit as iced_winit;

use crossfire::mpmc::{RxBlocking, RxFuture, SharedSenderBRecvF, SharedSenderFRecvB, TxBlocking, TxFuture};
use iced_core::{theme::{self, Base}, window, Element, Length, Theme};
use iced_futures::{futures::Stream, stream, Subscription};
use iced_widget::{button, column, container, text, themer};
use iced_winit::{program::Program, runtime::Task};
// use iced::{futures::Stream, stream, theme::{self, Base}, widget::{button, column, container, text, themer}, window, Element, Length, Program, Task, Theme};
use once_cell::sync::OnceCell;

use crate::lib_reloader::LibReloader;


pub static SUBSCRIPTION_CHANNEL: OnceCell<(TxBlocking<ReloadEvent, SharedSenderBRecvF>, RxFuture<ReloadEvent, SharedSenderBRecvF>)> = OnceCell::new();
pub static UPDATE_CHANNEL: OnceCell<(TxFuture<ReadyToReload, SharedSenderFRecvB>, RxBlocking<ReadyToReload, SharedSenderFRecvB>)> = OnceCell::new();
pub static LIB_RELOADER: OnceCell<HashMap<&'static str, Arc<Mutex<LibReloader>>>> = OnceCell::new();


pub struct Reload<P> 
where 
    P: Program + 'static,
    P::Message: Clone {
    program: P,
}

impl<P> Reload<P> 
where 
    P: Program + 'static,
    P::Message: Clone {
    pub fn new(program: P) -> Self {
        Self{program}
    }
}

impl<P: Program> Program for Reload<P>
    where 
        P: Program + 'static,
        P::Message: Clone {
    type State = Reloader<P>;
    type Message = Message<P>;
    type Theme = P::Theme;
    type Renderer = P::Renderer;
    type Executor = P::Executor;

    fn name() -> &'static str {
        P::name()
    }

    fn boot(&self) -> (Self::State, Task<Self::Message>) {
        Reloader::new(&self.program)
    }

    fn update(
        &self,
        state: &mut Self::State,
        message: Self::Message,
    ) -> Task<Self::Message> {
        state.update(&self.program, message)
    }

    fn view<'a>(
        &self,
        state: &'a Self::State,
        window: window::Id,
    ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
        state.view(&self.program, window)
    }

    fn title(&self, state: &Self::State, window: window::Id) -> String {
        state.title(&self.program, window)
    } 

    fn subscription(
        &self,
        state: &Self::State,
    ) -> Subscription<Self::Message> {
        state.subscription(&self.program)
    }

    fn theme(&self, state: &Self::State, window: window::Id) -> Self::Theme {
        state.theme(&self.program, window)
    }

    fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
        state.style(&self.program, theme)
    }

    fn scale_factor(&self, state: &Self::State, window: window::Id) -> f64 {
        state.scale_factor(&self.program, window)
    }
}


pub enum Message<P> where P: Program {
    None,
    AboutToReload,
    ReloadFinished,
    SendReadySignal,
    AppMessage(P::Message)
}


impl<P> Clone for Message<P> 
where 
    P: Program, 
    P::Message: Clone {
    fn clone(&self) -> Self {
        match &self {
            Self::AppMessage(message) => Self::AppMessage(message.clone()),
            Self::SendReadySignal => Self::SendReadySignal,
            Self::AboutToReload => Self::AboutToReload,
            Self::ReloadFinished => Self::ReloadFinished,
            Self::None => Self::None,
        }
    }
}

impl<P: Program> Debug for Message<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AppMessage(message) => message.fmt(f),
            Self::SendReadySignal => write!(f,"Self::SendReadySignal"),
            Self::AboutToReload => write!(f,"Self::Reloading"),
            Self::ReloadFinished => write!(f, "Self::ReloadFinished"),
            Self::None => write!(f,"Self::None"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReloadEvent {
    AboutToReload,
    ReloadComplete,
}

pub struct ReadyToReload;


pub struct Reloader<P: Program + 'static> {
    state: P::State,
    libraries_reloading: u16,
    update_ch_tx: TxFuture<ReadyToReload, SharedSenderFRecvB>,
}

impl<P> Reloader<P> 
where 
    P: Program + 'static,
    P::Message: Clone,
{

    pub fn new(program: &P) -> (Self, Task<Message<P>>) {
        let (update_ch_tx, _) = UPDATE_CHANNEL.get_or_init(|| crossfire::mpmc::bounded_tx_future_rx_blocking(1)).clone();

        let (state, task)  = program.boot();
        let reloader = Self {
            state,
            libraries_reloading: 0,
            update_ch_tx,
        };

        (reloader, task.map(Message::AppMessage))
    }


    pub fn update(&mut self, program: &P, message: Message<P>) -> Task<Message<P>> {
        match message {
            Message::AppMessage(message) => {
                if self.libraries_reloading > 0 {return Task::none()}

                program.update(&mut self.state, message).map(Message::AppMessage)
            }
            Message::AboutToReload => {
                self.libraries_reloading += 1;
                //updates the view so references to the state are dropped before sending the ready signal
                Task::done(Message::SendReadySignal)
            }
            Message::SendReadySignal => {
                let sender = self.update_ch_tx.clone();
                Task::future(async move {sender.send(ReadyToReload).await}).discard()
            }
            Message::ReloadFinished => {
                self.libraries_reloading -= 1;
                Task::none()
            }
            Message::None => {
                Task::none()
            }
        }
    }

    pub fn view(&self, program: &P, window: window::Id) -> Element<Message<P>, P::Theme, P::Renderer> {
        if self.libraries_reloading == 0 {
            program.view(&self.state, window).map(Message::AppMessage)
        } else {
            let content = container(column![
                    text("Reloading...").size(20),
                    button("Refresh").on_press(Message::None)
                ])
                .center_x(Length::Fill)
                .center_y(Length::Fill);

            let theme = program.theme(&self.state, window);

            let derive_theme = move || {
            theme
                .palette()
                .map(|palette| Theme::custom("reloader".to_string(), palette))
                .unwrap_or_default()
            };

            themer(derive_theme(), content).into()
        }
    }

    pub fn subscription(&self, program: &P) -> Subscription<Message<P>> {
        let subscription = program.subscription(&self.state).map(Message::AppMessage);
        let listen_for_lib_changes = Subscription::run(Self::listen_for_lib_change);
        Subscription::batch([subscription, listen_for_lib_changes])
    }

    pub fn title(&self, program: &P, window: window::Id) -> String {
        program.title(&self.state, window)
    }

    pub fn theme(&self, program: &P, window: window::Id) -> P::Theme {
        program.theme(&self.state, window)
    }

    pub fn style(&self, program: &P, theme: &P::Theme) -> theme::Style {
        program.style(&self.state, theme)
    }

    pub fn scale_factor(&self, program: &P, window: window::Id) -> f64 {
        program.scale_factor(&self.state, window)
    }

    fn listen_for_lib_change() -> impl Stream<Item = Message<P>> {
        let rx = SUBSCRIPTION_CHANNEL.get().unwrap().1.clone();
        stream::channel(10, async move |mut output| {
            loop {
                match rx.recv().await {
                    Ok(message) => {
                        match message {
                            ReloadEvent::AboutToReload => {
                                if let Err(err) = output.try_send(Message::AboutToReload) {
                                    println!("Failed to send reloading message: {err}")
                                }
                            }
                            ReloadEvent::ReloadComplete => {
                                if let Err(err) = output.try_send(Message::ReloadFinished){
                                    println!("Failed to send reload complete message: {err}")
                                }
                            }
                        }
                    }
                    Err(err) => {
                        println!("{err}")
                    }
                }
            }
        })
    }
}

pub fn find_workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut current = manifest_dir.as_path();
    
    // Go up directories until we find a directory with a Cargo.toml that 
    // contains [workspace]
    while let Some(parent) = current.parent() {
        let workspace_toml = parent.join("Cargo.toml");
        if workspace_toml.exists() {
            // Check if this Cargo.toml has a [workspace] section
            if let Ok(content) = std::fs::read_to_string(&workspace_toml) {
                if content.contains("[workspace]") {
                    return parent.to_path_buf();
                }
            }
        }
        current = parent;
    }
    
    // If we didn't find a workspace root, return the package root
    manifest_dir
}