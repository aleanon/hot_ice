use std::{
    collections::HashMap,
    error::Error,
    fmt::Debug,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};

use crossfire::{MAsyncRx, MAsyncTx, MRx, MTx, mpmc};
use iced_core::{
    Background, Color, Element, Length, Padding, Settings, Theme,
    theme::{self, Base, Mode},
    window,
};
use iced_futures::{Subscription, futures::Stream, stream};
use iced_widget::{
    Container, Text, column, container::Style as ContainerStyle, row, sensor, text::Style, themer,
};
use iced_winit::{program::Program, runtime::Task};

use crate::{hot_program::HotProgram, lib_reloader::LibReloader, message::MessageSource};

// pub static SUBSCRIPTION_CHANNEL: OnceCell<(MTx<ReloadEvent>, MAsyncRx<ReloadEvent>)> =
//     OnceCell::new();

// pub static UPDATE_CHANNEL: OnceCell<(MAsyncTx<ReadyToReload>, MRx<ReadyToReload>)> =
//     OnceCell::new();

// pub static LIB_RELOADER: OnceCell<HashMap<&'static str, Arc<Mutex<LibReloader>>>> = OnceCell::new();

const DEFAULT_LIB_PATH: &str = "target/reload/debug";

#[derive(Clone)]
pub struct ReloaderSettings {
    pub lib_path: String,
    /// Default is true, if this is set to false, you need to initiate the cargo watch command youself
    /// and make the lib accessible in the supplied `lib_path`
    pub compile_in_reloader: bool,
}

impl Default for ReloaderSettings {
    fn default() -> Self {
        Self {
            lib_path: String::from(DEFAULT_LIB_PATH),
            compile_in_reloader: true,
        }
    }
}

pub struct Reload<P>
where
    P: HotProgram + 'static,
    P::Message: Clone,
{
    program: P,
    reloader_settings: ReloaderSettings,
    lib_name: &'static str,
}

impl<P> Reload<P>
where
    P: HotProgram + 'static,
    P::Message: Clone,
{
    pub fn new(program: P, reloader_settings: ReloaderSettings, lib_name: &'static str) -> Self {
        Self {
            program,
            reloader_settings,
            lib_name,
        }
    }
}

impl<P> Program for Reload<P>
where
    P: HotProgram + 'static,
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
        Reloader::new(&self.program, &self.reloader_settings, &self.lib_name)
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

    fn settings(&self) -> Settings {
        Settings::default()
    }

    fn window(&self) -> Option<window::Settings> {
        Some(window::Settings::default())
    }

    fn title(&self, state: &Self::State, window: window::Id) -> String {
        state.title(&self.program, window)
    }

    fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
        state.subscription(&self.program)
    }

    fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
        state.theme(&self.program, window)
    }

    fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
        state.style(&self.program, theme)
    }

    fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
        state.scale_factor(&self.program, window)
    }
}

pub enum Message<P>
where
    P: HotProgram,
{
    None,
    AboutToReload,
    ReloadComplete,
    SendReadySignal,
    AppMessage(MessageSource<P::Message>),
}

impl<P> Clone for Message<P>
where
    P: HotProgram,
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

impl<P: HotProgram> Debug for Message<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AppMessage(message) => message.fmt(f),
            Self::SendReadySignal => write!(f, "SendReadySignal"),
            Self::AboutToReload => write!(f, "AboutToReload"),
            Self::ReloadComplete => write!(f, "ReloadComplete"),
            Self::None => write!(f, "None"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReloadEvent {
    AboutToReload,
    ReloadComplete,
}

pub struct ReadyToReload;

#[derive(Clone)]
pub enum FunctionState {
    Static,
    Hot,
    FallBackStatic(String),
    Error(String),
}

#[derive(Debug, PartialEq)]
enum ReloaderState {
    // Initial(u16, u16),
    Ready,
    Reloading(u16),
}

type UpdateChannel = (MAsyncTx<ReadyToReload>, MRx<ReadyToReload>);
type SubscriptionChannel = (MTx<ReloadEvent>, MAsyncRx<ReloadEvent>);

pub struct Reloader<P: HotProgram + 'static> {
    state: P::State,
    reloader_state: ReloaderState,
    lib_reloader: Option<Arc<Mutex<LibReloader>>>,
    reloader_settings: ReloaderSettings,
    lib_name: &'static str,
    sensor_key: u16,
    update_fn_state: FunctionState,
    subscription_fn_state: Mutex<FunctionState>,
    update_channel: UpdateChannel,
    subscription_channel: SubscriptionChannel,
}

impl<'a, P> Reloader<P>
where
    P: HotProgram + 'static,
    P::Message: Clone,
{
    pub fn new(
        program: &P,
        reloader_settings: &ReloaderSettings,
        lib_name: &'static str,
    ) -> (Self, Task<Message<P>>) {
        let (state, program_task) = program.boot();
        let mut reloader = Self {
            state,
            reloader_state: ReloaderState::Ready,
            lib_reloader: None,
            reloader_settings: reloader_settings.clone(),
            lib_name,
            sensor_key: 0,
            update_fn_state: FunctionState::Static,
            subscription_fn_state: Mutex::new(FunctionState::Static),
            update_channel: mpmc::bounded_tx_async_rx_blocking(1),
            subscription_channel: mpmc::bounded_tx_blocking_rx_async(1),
        };

        reloader.lib_reloader = Some(Self::initiate_reloader(
            &reloader.reloader_settings.lib_path,
            reloader.lib_name,
            reloader.update_channel.1.clone(),
            reloader.subscription_channel.0.clone(),
        ));

        // let compilation_task = Task::stream(Self::listen_for_compilation());
        let lib_change_task = Task::stream(Self::listen_for_lib_change(
            reloader.subscription_channel.1.clone(),
        ));

        (
            reloader,
            lib_change_task.chain(program_task.map(Message::AppMessage)),
        )
    }

    pub fn update(&mut self, program: &P, message: Message<P>) -> Task<Message<P>> {
        match message {
            Message::AppMessage(message) => {
                if self.reloader_state != ReloaderState::Ready {
                    return Task::none();
                }

                program
                    .update(
                        &mut self.state,
                        message,
                        &mut self.update_fn_state,
                        self.lib_reloader.as_ref(),
                    )
                    .map(Message::AppMessage)
            }
            Message::AboutToReload => {
                match self.reloader_state {
                    ReloaderState::Reloading(num) => {
                        self.reloader_state = ReloaderState::Reloading(num + 1);
                    }
                    _ => self.reloader_state = ReloaderState::Reloading(1),
                }
                self.sensor_key += 1;
                Task::none()
            }
            Message::SendReadySignal => {
                let sender = self.update_channel.0.clone();
                Task::future(async move { sender.send(ReadyToReload).await }).discard()
            }
            Message::ReloadComplete => {
                match &self.reloader_state {
                    ReloaderState::Reloading(num) => {
                        if *num == 1 {
                            self.reloader_state = ReloaderState::Ready;
                        } else {
                            self.reloader_state = ReloaderState::Reloading(num - 1);
                        }
                    }
                    s => {
                        log::error!(
                            "Invalid state, Should have ReloaderState::Reloading, found {:?}",
                            s
                        )
                    }
                }
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
        let with_theme = |content| {
            let theme = program
                .theme(&self.state, window)
                .unwrap_or(P::Theme::default(Mode::default()));

            let derive_theme = move || {
                theme
                    .palette()
                    .map(|palette| Theme::custom("reloader".to_string(), palette))
            };

            themer(derive_theme(), content).into()
        };

        let mut view_fn_state = FunctionState::Static;
        let program_view = if self.reloader_state == ReloaderState::Ready {
            program
                .view(
                    &self.state,
                    window,
                    &mut view_fn_state,
                    self.lib_reloader.as_ref(),
                )
                .map(Message::AppMessage)
        } else {
            let content = Container::new(
                sensor(Text::new("Reloading...").size(20))
                    .key(self.sensor_key)
                    .on_show(|_| Message::SendReadySignal),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill);

            with_theme(content.into())
        };

        let function_state_widgets = |(r, g, b, a), fn_name, error| {
            let function_name = Text::new(fn_name)
                .style(move |_| Style {
                    color: Some(Color::from_rgba8(r, g, b, a)),
                })
                .size(12);

            let error_block = Text::new(error)
                .style(|_| Style {
                    color: Some(Color::from_rgba8(225, 29, 72, 1.0)),
                })
                .size(12);

            column![function_name, error_block].spacing(2)
        };

        let function_state = |fn_state, fn_name| match fn_state {
            &FunctionState::Static => {
                function_state_widgets((255, 255, 255, 1.0), fn_name, String::new())
            }
            &FunctionState::Hot => {
                function_state_widgets((74, 222, 128, 1.0), fn_name, String::new())
            }
            &FunctionState::FallBackStatic(ref err) => {
                function_state_widgets((255, 152, 0, 1.0), fn_name, err.clone())
            }
            &FunctionState::Error(ref err) => {
                function_state_widgets((225, 29, 72, 1.0), fn_name, err.clone())
            }
        };

        let view_fn = Container::new(function_state(&view_fn_state, "View")).padding(3);
        let update_fn = Container::new(function_state(&self.update_fn_state, "Update")).padding(3);
        let subscription_fn = Container::new(function_state(
            &self
                .subscription_fn_state
                .try_lock()
                .map(|m| m.clone())
                .unwrap_or(FunctionState::Static),
            "Subscription",
        ))
        .padding(3);

        let function_states = row![view_fn, update_fn, subscription_fn]
            .spacing(100)
            .padding(Padding {
                left: 20.,
                right: 20.,
                ..Default::default()
            });

        let top_bar = with_theme(
            Container::new(function_states)
                .style(|_| ContainerStyle {
                    background: Some(Background::Color(Color::BLACK)),
                    ..Default::default()
                })
                .width(Length::Fill),
        );

        column![top_bar, program_view].into()
    }

    pub fn subscription(&self, program: &P) -> Subscription<Message<P>> {
        match self.subscription_fn_state.try_lock() {
            Ok(mut fn_state) => {
                if self.reloader_state == ReloaderState::Ready {
                    program
                        .subscription(&self.state, &mut fn_state, self.lib_reloader.as_ref())
                        .map(Message::AppMessage)
                } else {
                    Subscription::none()
                }
            }
            Err(_) => Subscription::none(),
        }
    }

    pub fn title(&self, program: &P, window: window::Id) -> String {
        program.title(&self.state, window)
    }

    pub fn theme(&self, program: &P, window: window::Id) -> Option<P::Theme> {
        program.theme(&self.state, window)
    }

    pub fn style(&self, program: &P, theme: &P::Theme) -> theme::Style {
        program.style(&self.state, theme)
    }

    pub fn scale_factor(&self, program: &P, window: window::Id) -> f32 {
        program.scale_factor(&self.state, window)
    }

    fn listen_for_lib_change(rx: MAsyncRx<ReloadEvent>) -> impl Stream<Item = Message<P>> {
        // let rx = SUBSCRIPTION_CHANNEL.get().unwrap().1.clone();
        stream::channel(10, async move |mut output| {
            loop {
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
            }
        })
    }

    // fn compile_library(
    //     lib_dir: &str,
    //     library_name: &str,
    //     target_dir: &str,
    // ) -> Result<(), Box<dyn Error>> {
    //     let watch_path: &str = library_name;

    //     let child = Command::new("cargo")
    //             .arg("watch")
    //             .arg("-w")
    //             .arg(watch_path)
    //             .arg("-d")
    //             .arg("0.01")
    //             .arg("-x")
    //             .arg(format!(
    //                 "rustc --package {} --crate-type cdylib --profile dev -- -C link-arg=-Wl,--whole-archive",
    //                 library_name
    //             ))
    //             .env("CARGO_PROFILE_DEV_OPT_LEVEL", "0")
    //             .env("CARGO_PROFILE_DEV_CODEGEN_UNITS", "1")
    //             .env("CARGO_PROFILE_DEV_DEBUG", "false")
    //             .env("CARGO_PROFILE_DEV_LTO", "false")
    //             .env("CARGO_TARGET_DIR", target_dir)
    //             .stdout(Stdio::piped())
    //             .stderr(Stdio::piped())
    //             .spawn()?;

    //     let stdout = child.stdout.take().unwrap();
    //     let stderr = child.stderr.take().unwrap();

    //     stream::channel(10, async move |mut output| {
    //         let stdout_reader = BufReader::new(stdout);
    //         for line in stdout_reader.lines() {
    //             let line = line?;
    //         }

    //         if status.success() {
    //             Ok(())
    //         } else {
    //             Err(std::io::Error::new(
    //                 std::io::ErrorKind::Other,
    //                 format!("cargo watch exited with status: {}", status),
    //             ))
    //         }
    //     });

    //     let stdout_reader = BufReader::new(stdout);
    //     for line in stdout_reader.lines() {
    //         let line = line?;
    //     }

    //     if status.success() {
    //         Ok(())
    //     } else {
    //         Err(std::io::Error::new(
    //             std::io::ErrorKind::Other,
    //             format!("cargo watch exited with status: {}", status),
    //         ))
    //     }
    // }

    fn initiate_reloader(
        lib_dir: &str,
        library_name: &str,
        update_ch_rx: MRx<ReadyToReload>,
        subscription_ch_tx: MTx<ReloadEvent>,
    ) -> Arc<Mutex<LibReloader>> {
        let mut lib_reloader =
            LibReloader::new(lib_dir, library_name, Some(Duration::from_millis(25)), None)
                .expect("Unable to create LibReloader");

        let change_subscriber = lib_reloader.subscribe_to_file_changes();
        let lib_reloader = Arc::new(Mutex::new(lib_reloader));
        let lib = lib_reloader.clone();

        std::thread::spawn(move || {
            loop {
                change_subscriber.recv().expect("Sub channel closed");

                if let Err(err) = subscription_ch_tx.send(ReloadEvent::AboutToReload) {
                    println!("{err}")
                }

                update_ch_rx.recv().expect("Update Channel closed");

                loop {
                    if let Ok(mut lib_reloader) = lib.lock() {
                        if let Err(err) = lib_reloader.update() {
                            println!("{err}")
                        } else {
                            break;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }

                subscription_ch_tx
                    .send(ReloadEvent::ReloadComplete)
                    .expect("Subscription channel closed");
            }
        });
        lib_reloader
    }
}
