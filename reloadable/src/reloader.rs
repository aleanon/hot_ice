use std::{fmt::Debug, marker::PhantomData, sync::{mpsc, Arc, Mutex, RwLock}, time::Duration};

use crossfire::mpmc::{RxBlocking, RxFuture, SharedSenderBRecvF, SharedSenderFRecvB, TxBlocking, TxFuture};
use iced::{advanced, futures::Stream, stream, theme::{self, Base}, widget::{button, column, container, stack, text, themer, Themer}, window, Element, Length, Program, Task, Theme};
use once_cell::sync::OnceCell;

use crate::lib_reloader::LibReloader;




pub static SUBSCRIPTION_CHANNEL: OnceCell<(TxBlocking<ReloadEvent, SharedSenderBRecvF>, RxFuture<ReloadEvent, SharedSenderBRecvF>)> = OnceCell::new();
pub static UPDATE_CHANNEL: OnceCell<(TxFuture<ReadyToReload, SharedSenderFRecvB>, RxBlocking<ReadyToReload, SharedSenderFRecvB>)> = OnceCell::new();
pub static LIB_RELOADER: OnceCell<Arc<Mutex<LibReloader>>> = OnceCell::new();

#[allow(type_alias_bounds)]
type Widget<'a, P: Program> = iced::Element<'a, P::Message, P::Theme, P::Renderer>;
#[allow(type_alias_bounds)]
type View<'a, P: Program> = fn(&'a P::State) -> Widget<'a, P>; 


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
    is_reloading: bool,
    update_ch_tx: TxFuture<ReadyToReload, SharedSenderFRecvB>,
}

impl<P> Reloader<P> 
where 
    P: Program + 'static,
    P::Message: Clone,
{

    pub fn new(program: &P) -> (Self, Task<Message<P>>) {
        let (update_ch_tx, update_ch_rx) = UPDATE_CHANNEL.get_or_init(|| crossfire::mpmc::bounded_tx_future_rx_blocking(1)).clone();
        let (sub_ch_tx, _) = SUBSCRIPTION_CHANNEL.get_or_init(||crossfire::mpmc::bounded_tx_blocking_rx_future(1)).clone();
        let mut lib_reloader = LibReloader::new("target/debug/lib/debug", "ui", None, None).expect("Unable to create LibReloader");
        let change_subscriber = lib_reloader.subscribe_to_file_changes();
        LIB_RELOADER.get_or_init(||Arc::new(Mutex::new(lib_reloader)));

        std::thread::spawn(move || {                        
            loop {
                let Ok(_) = change_subscriber.recv() else {
                    panic!("Sub channel closed")
                };
                if let Err(err) = sub_ch_tx.send(ReloadEvent::AboutToReload) {
                    println!("{err}")
                }

                let Ok(ReadyToReload) = update_ch_rx.recv() else {
                    panic!("Update Channel closed")
                };

                loop {
                    if let Some(lock) = LIB_RELOADER.get() {
                        if let Ok(mut lib_reloader) = lock.lock() {
                            if let Err(err) = lib_reloader.update() {
                                println!("{err}")
                            } else {
                                break;
                            }
                        }
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }

                if let Err(_) = sub_ch_tx.send(ReloadEvent::ReloadComplete) {
                    panic!("Subscription Channel closed")
                }
            }

        });


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

                let lib = LIB_RELOADER.get().unwrap().try_lock().unwrap();
                unsafe {
                    match lib.get_symbol::<fn(&mut P::State, P::Message)->Task<P::Message>>(b"update") {
                        Ok(function) => function(&mut self.state, message).map(Message::AppMessage),
                        Err(err) => {
                            eprintln!("{err}");
                            Task::none()
                        }
                    }
                }
            }
            Message::AboutToReload => {
                self.is_reloading = true;
                //updates the view so references to the state are dropped before sending the ready signal
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
            let lib = LIB_RELOADER.get().unwrap().try_lock().unwrap();
            match unsafe{lib.get_symbol::<fn(&P::State)->iced::Element<P::Message, P::Theme, P::Renderer>>(b"view")} {
                Ok(function) => function(&self.state).map(Message::AppMessage),
                Err(err) => {
                    println!("{err}");
                    program.view(&self.state, window).map(Message::AppMessage)
                }
            }
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
                .map(|palette| Theme::custom("reloader", palette))
                .unwrap_or_default()
            };

            themer(derive_theme(), content).into()
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
