use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, Mutex},
};

use crossfire::mpmc::{
    RxBlocking, RxFuture, SharedSenderBRecvF, SharedSenderFRecvB, TxBlocking, TxFuture,
};
use iced_core::{
    theme::{self, Base},
    window, Element, Length, Theme,
};
use iced_futures::{futures::Stream, stream, Subscription};
use iced_widget::{container, pop, text, themer};
use iced_winit::{program::Program, runtime::Task};
use once_cell::sync::OnceCell;

use crate::lib_reloader::LibReloader;

pub static SUBSCRIPTION_CHANNEL: OnceCell<(
    TxBlocking<ReloadEvent, SharedSenderBRecvF>,
    RxFuture<ReloadEvent, SharedSenderBRecvF>,
)> = OnceCell::new();

pub static UPDATE_CHANNEL: OnceCell<(
    TxFuture<ReadyToReload, SharedSenderFRecvB>,
    RxBlocking<ReadyToReload, SharedSenderFRecvB>,
)> = OnceCell::new();

pub static LIB_RELOADER: OnceCell<HashMap<&'static str, Arc<Mutex<LibReloader>>>> = OnceCell::new();

pub struct Reload<P>
where
    P: Program + 'static,
    P::Message: Clone,
{
    program: P,
}

impl<P> Reload<P>
where
    P: Program + 'static,
    P::Message: Clone,
{
    pub fn new(program: P) -> Self {
        Self { program }
    }
}

impl<P: Program> Program for Reload<P>
where
    P: Program + 'static,
    P::Message: Clone,
{
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

    fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
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

    fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
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

pub enum Message<P>
where
    P: Program,
{
    None,
    AboutToReload,
    ReloadComplete,
    SendReadySignal,
    AppMessage(P::Message),
}

impl<P> Clone for Message<P>
where
    P: Program,
    P::Message: Clone,
{
    fn clone(&self) -> Self {
        match &self {
            Self::AppMessage(message) => Self::AppMessage(message.clone()),
            Self::SendReadySignal => Self::SendReadySignal,
            Self::AboutToReload => Self::AboutToReload,
            Self::ReloadComplete => Self::ReloadComplete,
            Self::None => Self::None,
        }
    }
}

impl<P: Program> Debug for Message<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AppMessage(message) => message.fmt(f),
            Self::SendReadySignal => write!(f, "Self::SendReadySignal"),
            Self::AboutToReload => write!(f, "Self::Reloading"),
            Self::ReloadComplete => write!(f, "Self::ReloadFinished"),
            Self::None => write!(f, "Self::None"),
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
    pop_key: u16,
}

impl<'a, P> Reloader<P>
where
    P: Program + 'static,
    P::Message: Clone,
{
    pub fn new(program: &P) -> (Self, Task<Message<P>>) {
        let (update_ch_tx, _) = UPDATE_CHANNEL
            .get_or_init(|| crossfire::mpmc::bounded_tx_future_rx_blocking(1))
            .clone();

        let (state, task) = program.boot();
        let reloader = Self {
            state,
            libraries_reloading: 0,
            update_ch_tx,
            pop_key: 0,
        };

        (reloader, task.map(Message::AppMessage))
    }

    pub fn update(&mut self, program: &P, message: Message<P>) -> Task<Message<P>> {
        match message {
            Message::AppMessage(message) => {
                if self.libraries_reloading > 0 {
                    return Task::none();
                }

                program
                    .update(&mut self.state, message)
                    .map(Message::AppMessage)
            }
            Message::AboutToReload => {
                self.libraries_reloading += 1;
                self.pop_key += 1;
                Task::none()
            }
            Message::SendReadySignal => {
                let sender = self.update_ch_tx.clone();
                Task::future(async move { sender.send(ReadyToReload).await }).discard()
            }
            Message::ReloadComplete => {
                self.libraries_reloading -= 1;
                Task::none()
            }
            Message::None => Task::none(),
        }
    }

    pub fn view(
        &'a self,
        program: &P,
        window: window::Id,
    ) -> Element<'a, Message<P>, P::Theme, P::Renderer> {
        if self.libraries_reloading == 0 {
            program.view(&self.state, window).map(Message::AppMessage)
        } else {
            let content = container(
                pop(text("Reloading...").size(20))
                    .key(self.pop_key)
                    .on_show(|_| Message::SendReadySignal),
            )
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
        stream::channel(10, async move |mut output| loop {
            match rx.recv().await {
                Ok(message) => match message {
                    ReloadEvent::AboutToReload => {
                        if let Err(err) = output.try_send(Message::AboutToReload) {
                            println!("Failed to send reloading message: {err}")
                        }
                    }
                    ReloadEvent::ReloadComplete => {
                        if let Err(err) = output.try_send(Message::ReloadComplete) {
                            println!("Failed to send reload complete message: {err}")
                        }
                    }
                },
                Err(err) => {
                    println!("{err}")
                }
            }
        })
    }
}
