use std::fmt::Debug;

use crossfire::mpmc::{RxBlocking, RxFuture, SharedSenderBRecvF, SharedSenderFRecvB, TxBlocking, TxFuture};
use iced::{advanced::{self, widget}, futures::Stream, shell::graphics::compositor, stream, theme::{self, Base}, widget::{button, column, container, stack, text, themer}, window, Element, Length, Program, Task, Theme};
use once_cell::sync::OnceCell;




static SUBSCRIPTION_CHANNEL: OnceCell<(TxBlocking<ReloadEvent,SharedSenderBRecvF>, RxFuture<ReloadEvent, SharedSenderBRecvF>)> = OnceCell::new();
static UPDATE_CHANNEL: OnceCell<(TxFuture<ReadyToReload, SharedSenderFRecvB>, RxBlocking<ReadyToReload, SharedSenderFRecvB>)> = OnceCell::new();


pub enum Message<P> where P: Program {
    None,
    Reloading,
    ReloadFinished,
    SendReadySignal,
    AppMessage(P::Message)
}


impl<P> Clone for Message<P> 
where 
    P: Program,
    P::Message: Clone
     {
    fn clone(&self) -> Self {
        match &self {
            Self::AppMessage(message) => Self::AppMessage(message.clone()),
            Self::SendReadySignal => Self::SendReadySignal,
            Self::Reloading => Self::Reloading,
            Self::ReloadFinished => Self::ReloadFinished,
            Self::None => Self::None,
        }
    }
}

impl<P: Program> Debug for Message<P> where P::Message: Debug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AppMessage(message) => write!(f,"Message::AppMessage({:?})", message),
            Self::SendReadySignal => write!(f,"Self::SendReadySignal"),
            Self::Reloading => write!(f,"Self::Reloading"),
            Self::ReloadFinished => write!(f, "Self::ReloadFinished"),
            Self::None => write!(f,"Self::None"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ReloadEvent {
    AboutToReload,
    ReloadComplete
}

struct ReadyToReload;

pub trait Catalogs: text::Catalog + button::Catalog + container::Catalog {} 

pub struct Reloader<P: Program> {
    state: P::State,
    is_reloading: bool,
    update_ch_tx: TxFuture<ReadyToReload, SharedSenderFRecvB>,
}

impl<P> Reloader<P> where 
    P: Program + 'static,
    P::Message: Clone,
    {

    pub fn wrap(program: P) -> Wrapper<P> {
        Wrapper{program}
    }

    pub fn new(program: &P) -> (Self, Task<Message<P>>) {

        let (update_ch_tx, _) = UPDATE_CHANNEL.get().unwrap().clone();
        let (state, task)  = program.boot();
        let reloader = Self {
            state,
            is_reloading: false,
            update_ch_tx,
        };

        (reloader, task.map(Message::AppMessage))
    }


    pub fn update(&mut self, program: &P, message: Message<P>) -> Task<Message<P>> {
        match message {
            Message::AppMessage(message) => {
                if self.is_reloading {return Task::none()}
                program.update(&mut self.state, message).map(Message::AppMessage)
                // self.state.update(&mut self.state, message).map(Message::AppMessage)
            }
            Message::Reloading => {
                self.is_reloading = true;
                Task::done(Message::SendReadySignal)
            }
            Message::SendReadySignal => {
                let sender = self.update_ch_tx.clone();
                Task::future(async move {sender.send(ReadyToReload).await}).discard()
            }
            Message::ReloadFinished => {
                self.is_reloading = false;
                Task::none()
            }
            Message::None => {
                Task::none()
            }
        }
    }

    pub fn view(&self, program: &P, window: window::Id) -> Element<Message<P>, P::Theme, P::Renderer> {
        if !self.is_reloading {
            program.view(&self.state, window).map(Message::AppMessage)
        } else {
            let theme:<P as Program>::Theme = program.theme(&self.state, window);

            let derive_theme = move || {
            theme
                .palette()
                .map(|palette| Theme::custom("reloader", palette))
                .unwrap_or_default()
            };

            themer(derive_theme(), container(column![
                text("Reloading...").size(20),
                button("Refresh").on_press(Message::None)
            ])
            .center_x(Length::Fill)
            .center_y(Length::Fill))
        }
    }

    pub fn subscription(&self, program: &P) -> iced::Subscription<Message<P>> {
        let subscription = program.subscription(&self.state).map(Message::AppMessage);
        let listen_for_lib_changes = iced::Subscription::run(Self::listen_for_lib_change);
        iced::Subscription::batch([subscription, listen_for_lib_changes])
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
                                if let Err(err) = output.try_send(Message::Reloading) {
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

pub struct Wrapper<P: Program + 'static> {
    program: P
}

impl<P: Program> Program for Wrapper<P> 
    where 
        P: Program + 'static,
        P::Theme: Catalogs,
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
    ) -> iced::Subscription<Self::Message> {
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