use std::{
    fmt::Debug,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};

use cargo_metadata::{MetadataCommand, camino::Utf8PathBuf};
use crossfire::{AsyncRx, MAsyncRx, MTx, mpmc};
use iced_core::{
    Background, Color, Element, Length, Padding, Settings, Theme,
    theme::{self, Base, Mode},
    window,
};
use iced_futures::{Subscription, futures::Stream, stream};
use iced_widget::{
    Container, Text, column, container::Style as ContainerStyle, row, sensor, space, text::Style,
    themer,
};
use iced_winit::{program::Program, runtime::Task};
use log::info;
use thiserror::Error;

use crate::{
    DESERIALIZE_STATE_FUNCTION_NAME, HotFunctionError, SERIALIZE_STATE_FUNCTION_NAME,
    hot_program::HotProgram, lib_reloader::LibReloader, message::MessageSource,
};

const DEFAULT_TARGET_DIR: &str = "target/reload";
const DEFAULT_LIB_DIR: &str = "target/reload/debug";

#[derive(Clone)]
pub struct ReloaderSettings {
    /// The target directory for the build command, default: target/reload
    pub target_dir: String,
    /// The directory where the compiled dynamic library is located, default: target/reload/debug
    pub lib_dir: String,
    /// Default is true, if this is set to false, you need to initiate the cargo watch command youself
    /// and make the lib accessible in the supplied `lib_dir`
    pub compile_in_reloader: bool,
    /// The time between each check for a new dynamic library file, default is 25ms
    pub file_watch_debounce: Duration,
    /// The directory to watch for changes before recompiling, None means it will watch
    /// the UI crate root, default: None
    pub watch_dir: Option<PathBuf>,
}

impl Default for ReloaderSettings {
    fn default() -> Self {
        Self {
            target_dir: DEFAULT_TARGET_DIR.to_string(),
            lib_dir: DEFAULT_LIB_DIR.to_string(),
            compile_in_reloader: true,
            file_watch_debounce: Duration::from_millis(25),
            watch_dir: None,
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
    settings: Settings,
    window_settings: window::Settings,
    lib_name: &'static str,
}

impl<P> Reload<P>
where
    P: HotProgram + 'static,
    P::Message: Clone,
{
    pub fn new(
        program: P,
        reloader_settings: ReloaderSettings,
        settings: Settings,
        window_settings: window::Settings,
        lib_name: &'static str,
    ) -> Self {
        Self {
            program,
            reloader_settings,
            settings,
            window_settings,
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
        self.settings.clone()
    }

    fn window(&self) -> Option<window::Settings> {
        Some(self.window_settings.clone())
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
    CompilationComplete,
    AboutToReload,
    ReloadComplete,
    SendReadySignal,
    Error(ReloaderError),
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
            Self::CompilationComplete => Self::CompilationComplete,
            Self::Error(error) => Self::Error(error.clone()),
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
            Self::CompilationComplete => write!(f, "CompilationComplete"),
            Self::Error(error) => write!(f, "{}", error),
            Self::None => write!(f, "None"),
        }
    }
}

enum DynamicStateAction {
    Serialize,
    Deserialize,
}

impl DynamicStateAction {
    fn function_name(&self) -> &'static str {
        match self {
            DynamicStateAction::Serialize => SERIALIZE_STATE_FUNCTION_NAME,
            DynamicStateAction::Deserialize => DESERIALIZE_STATE_FUNCTION_NAME,
        }
    }
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
    Compiling,
    Ready,
    Reloading(u16),
    Error(ReloaderError),
}

#[derive(Debug, Clone, Error, PartialEq)]
pub enum ReloaderError {
    #[error("Failed to build command {0}")]
    FailedToBuildCommand(String),
    #[error("Compilation error: {0}")]
    CompilationError(String),
}

type UpdateChannel = (MTx<ReadyToReload>, MAsyncRx<ReadyToReload>);

pub struct Reloader<P: HotProgram + 'static> {
    state: P::State,
    serialized_state_ptr: *mut u8,
    serialized_state_len: usize,
    reloader_state: ReloaderState,
    lib_reloader: Option<Arc<Mutex<LibReloader>>>,
    reloader_settings: ReloaderSettings,
    lib_name: &'static str,
    sensor_key: u16,
    update_fn_state: FunctionState,
    subscription_fn_state: Mutex<FunctionState>,
    theme_fn_state: Mutex<FunctionState>,
    style_fn_state: Mutex<FunctionState>,
    scale_factor_fn_state: Mutex<FunctionState>,
    title_fn_state: Mutex<FunctionState>,
    update_channel: UpdateChannel,
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
            serialized_state_ptr: std::ptr::null_mut(),
            serialized_state_len: 0,
            reloader_state: ReloaderState::Compiling,
            lib_reloader: None,
            reloader_settings: reloader_settings.clone(),
            lib_name,
            sensor_key: 0,
            update_fn_state: FunctionState::Static,
            subscription_fn_state: Mutex::new(FunctionState::Static),
            theme_fn_state: Mutex::new(FunctionState::Static),
            style_fn_state: Mutex::new(FunctionState::Static),
            scale_factor_fn_state: Mutex::new(FunctionState::Static),
            title_fn_state: Mutex::new(FunctionState::Static),
            update_channel: mpmc::bounded_tx_blocking_rx_async(1),
        };

        let task = if reloader_settings.compile_in_reloader {
            Task::stream(Self::build_library(
                reloader.lib_name,
                reloader_settings.target_dir.clone(),
            ))
        } else {
            let mut lib_reloader = LibReloader::new(
                &reloader.reloader_settings.lib_dir,
                reloader.lib_name,
                Some(reloader.reloader_settings.file_watch_debounce),
                None,
            )
            .expect("Unable to create LibReloader");

            let change_subscriber = lib_reloader.subscribe_to_file_changes();
            let lib_reloader = Arc::new(Mutex::new(lib_reloader));
            reloader.lib_reloader = Some(lib_reloader.clone());

            reloader.reloader_state = ReloaderState::Ready;
            Task::stream(Self::listen_for_lib_changes(
                lib_reloader,
                reloader.update_channel.1.clone(),
                change_subscriber,
            ))
        };

        (reloader, task.chain(program_task.map(Message::AppMessage)))
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
            Message::CompilationComplete => {
                let mut lib_reloader = LibReloader::new(
                    &self.reloader_settings.lib_dir,
                    self.lib_name,
                    Some(self.reloader_settings.file_watch_debounce),
                    None,
                )
                .expect("Unable to create LibReloader");

                let change_subscriber = lib_reloader.subscribe_to_file_changes();
                let lib_reloader = Arc::new(Mutex::new(lib_reloader));
                self.lib_reloader = Some(lib_reloader.clone());

                self.reloader_state = ReloaderState::Ready;
                let listen_for_lib_changes = Task::stream(Self::listen_for_lib_changes(
                    lib_reloader,
                    self.update_channel.1.clone(),
                    change_subscriber,
                ));

                let watch_dir = self
                    .reloader_settings
                    .watch_dir
                    .clone()
                    .and_then(|p| Utf8PathBuf::from_path_buf(p).ok());

                let watch_dir = match watch_dir {
                    Some(dir) => dir,
                    None => {
                        let metadata = MetadataCommand::new()
                            .exec()
                            .expect("Failed to get cargo metadata");

                        let package = metadata
                            .packages
                            .iter()
                            .find(|p| p.name == self.lib_name)
                            .expect("Found no crate matching the lib name");

                        let mut manifest_path = package.manifest_path.clone();
                        manifest_path.pop();
                        manifest_path
                    }
                };

                log::info!("Directory to watch: {:?}", watch_dir);

                let watch = Task::stream(Self::watch_library(
                    watch_dir,
                    self.lib_name,
                    self.reloader_settings.target_dir.clone(),
                ));
                Task::batch([listen_for_lib_changes, watch])
            }
            Message::Error(error) => {
                self.reloader_state = ReloaderState::Error(error);
                Task::none()
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
                self.dynamic_state_action(DynamicStateAction::Serialize)
                    .ok();
                self.update_channel
                    .0
                    .send(ReadyToReload)
                    .expect("Update Channel closed");
                Task::none()
            }
            Message::ReloadComplete => {
                match &self.reloader_state {
                    ReloaderState::Reloading(num) => {
                        if *num == 1 {
                            self.dynamic_state_action(DynamicStateAction::Deserialize)
                                .ok();
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
            Message::None => panic!(),
        }
    }

    pub fn view(
        &'a self,
        program: &P,
        window: window::Id,
    ) -> Element<'a, Message<P>, P::Theme, P::Renderer> {
        let with_default_theme = |content| {
            let theme = P::Theme::default(Mode::Dark);

            let derive_theme = move || {
                theme
                    .palette()
                    .map(|palette| Theme::custom("reloader".to_string(), palette))
            };

            themer(derive_theme(), content).into()
        };

        let mut view_fn_state = FunctionState::Static;
        let program_view = match &self.reloader_state {
            ReloaderState::Ready => program
                .view(
                    &self.state,
                    window,
                    &mut view_fn_state,
                    self.lib_reloader.as_ref(),
                )
                .map(Message::AppMessage),
            ReloaderState::Reloading(_) => {
                let content = Container::new(
                    sensor(Text::new("Reloading...").size(20))
                        .key(self.sensor_key)
                        .on_show(|_| Message::SendReadySignal),
                )
                .center_x(Length::Fill)
                .center_y(Length::Fill);

                with_default_theme(content.into())
            }
            ReloaderState::Error(error) => {
                let content = Container::new(Text::new(error.to_string()).size(20))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill);

                with_default_theme(content.into())
            }
            ReloaderState::Compiling => {
                let content = Container::new(Text::new("Compiling...").size(20))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill);

                with_default_theme(content.into())
            }
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

        let sub_fn_state = self
            .subscription_fn_state
            .try_lock()
            .map(|m| m.clone())
            .unwrap_or(FunctionState::Static);
        let subscription_fn =
            Container::new(function_state(&sub_fn_state, "Subscription")).padding(3);

        let theme_fn_state = self
            .theme_fn_state
            .try_lock()
            .map(|m| m.clone())
            .unwrap_or(FunctionState::Static);
        let theme_fn = Container::new(function_state(&theme_fn_state, "Theme")).padding(3);

        let function_states = row![
            space().width(Length::Fill),
            view_fn,
            update_fn,
            subscription_fn,
            theme_fn,
            space().width(Length::Fill)
        ]
        .spacing(100)
        .padding(Padding {
            left: 20.,
            right: 20.,
            ..Default::default()
        });

        let top_bar = with_default_theme(
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
        if let Ok(mut title_fn_state) = self.title_fn_state.lock() {
            if self.reloader_state == ReloaderState::Ready {
                return program.title(
                    &self.state,
                    window,
                    &mut title_fn_state,
                    self.lib_reloader.as_ref(),
                );
            }
        };
        String::from("Hot Ice")
    }

    pub fn theme(&self, program: &P, window: window::Id) -> Option<P::Theme> {
        if let Ok(mut theme_fn_state) = self.theme_fn_state.lock() {
            if self.reloader_state == ReloaderState::Ready {
                return program.theme(
                    &self.state,
                    window,
                    &mut theme_fn_state,
                    self.lib_reloader.as_ref(),
                );
            }
        }
        None
    }

    pub fn style(&self, program: &P, theme: &P::Theme) -> theme::Style {
        if let Ok(mut style_fn_state) = self.style_fn_state.lock() {
            if self.reloader_state == ReloaderState::Ready {
                return program.style(
                    &self.state,
                    theme,
                    &mut style_fn_state,
                    self.lib_reloader.as_ref(),
                );
            }
        };
        theme.base()
    }

    pub fn scale_factor(&self, program: &P, window: window::Id) -> f32 {
        if let Ok(mut scale_factor_fn_state) = self.scale_factor_fn_state.lock() {
            if self.reloader_state == ReloaderState::Ready {
                return program.scale_factor(
                    &self.state,
                    window,
                    &mut scale_factor_fn_state,
                    self.lib_reloader.as_ref(),
                );
            }
        };
        1.0
    }

    fn build_library(
        lib_crate_name: &'static str,
        target_dir: String,
    ) -> impl Stream<Item = Message<P>> {
        stream::channel(200, async move |mut output| {
            let metadata = MetadataCommand::new()
                .exec()
                .expect("Failed to get cargo metadata");

            let workspace_root = metadata.workspace_root.as_std_path();

            let result = Command::new("cargo")
                .current_dir(workspace_root)
                .args(build_args(lib_crate_name))
                .environment_variables(&target_dir)
                .stderr(Stdio::piped())
                .spawn();

            let mut child = match result {
                Ok(child) => child,
                Err(err) => {
                    if let Err(err) = output.try_send(Message::Error(
                        ReloaderError::FailedToBuildCommand(err.to_string()),
                    )) {
                        log::error!("Failed to send Message: {}", err);
                    }
                    return;
                }
            };

            let stderr = child.stderr.take().unwrap();
            let stderr_reader = BufReader::new(stderr);

            for line in stderr_reader.lines() {
                match line {
                    Ok(line) => {
                        log::info!("{}", line);
                    }
                    Err(err) => {
                        log::error!("Failed to read line from stderr: {}", err);
                    }
                };
            }
            match child.wait() {
                Ok(status) => {
                    let message = if status.success() {
                        Message::CompilationComplete
                    } else {
                        Message::Error(ReloaderError::CompilationError(status.to_string()))
                    };
                    if let Err(err) = output.try_send(message) {
                        log::error!("Failed to send message: {err}");
                    }
                }
                Err(err) => {
                    log::error!("Failed to wait for child process: {}", err);
                }
            }
        })
    }

    fn watch_library(
        watch_dir: Utf8PathBuf,
        lib_crate_name: &'static str,
        target_dir: String,
    ) -> impl Stream<Item = Message<P>> {
        stream::channel(200, async move |mut output| {
            let metadata = MetadataCommand::new()
                .exec()
                .expect("Failed to get cargo metadata");

            let workspace_root = metadata.workspace_root;

            let Ok(watch_dir) = watch_dir.strip_prefix(&workspace_root) else {
                log::error!("Failed to strip prefix");
                return;
            };

            log::info!("workspace_root: {}", workspace_root);
            log::info!("watch dir relative path: {}", watch_dir);

            let result = Command::new("cargo")
                .current_dir(workspace_root)
                .arg("watch")
                .arg("-w")
                .arg(watch_dir)
                .arg("-d")
                .arg("0.01")
                .arg("-x")
                .arg(build_args(lib_crate_name).join(" "))
                .environment_variables(&target_dir)
                .stderr(Stdio::piped())
                .spawn();

            let mut child = match result {
                Ok(child) => child,
                Err(err) => {
                    if let Err(err) = output.try_send(Message::Error(
                        ReloaderError::FailedToBuildCommand(err.to_string()),
                    )) {
                        log::error!("Failed to send Message: {}", err);
                    }
                    return;
                }
            };

            log::info!("cargo watch started successfully");

            if let Some(stderr) = child.stderr.take() {
                std::thread::spawn(move || {
                    let stderr_reader = BufReader::new(stderr);
                    for line in stderr_reader.lines() {
                        match line {
                            Ok(line) => {
                                log::info!("[cargo watch] {}", line);
                            }
                            Err(err) => {
                                log::error!("Failed to read line from stderr: {}", err);
                                break;
                            }
                        };
                    }
                    log::info!("cargo watch stderr reader stopped");
                });
            }
        })
    }

    fn listen_for_lib_changes(
        lib_reloader: Arc<Mutex<LibReloader>>,
        update_ch_rx: MAsyncRx<ReadyToReload>,
        change_subscriber: AsyncRx<()>,
    ) -> impl Stream<Item = Message<P>> {
        stream::channel(10, async move |mut output| {
            loop {
                log::info!("Waiting for lib changes");
                change_subscriber.recv().await.expect("Sub channel closed");

                if let Err(err) = output.try_send(Message::AboutToReload) {
                    log::error!("Failed to send reloading message: {err}")
                }

                update_ch_rx.recv().await.expect("Update Channel closed");

                log::info!("Reloading library");

                loop {
                    if let Ok(mut reloader) = lib_reloader.lock() {
                        if let Err(err) = reloader.update() {
                            log::error!("{err}")
                        } else {
                            break;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }

                log::info!("Reload complete");

                if let Err(err) = output.try_send(Message::ReloadComplete) {
                    log::error!("Failed to send reload complete message: {err}")
                }
            }
        })
    }

    fn dynamic_state_action(&mut self, action: DynamicStateAction) -> Result<(), HotFunctionError> {
        let reloader = self
            .lib_reloader
            .as_ref()
            .expect("reloader not initialized");

        let Ok(reloader) = reloader.lock() else {
            return Err(HotFunctionError::LockAcquisitionError);
        };

        match action {
            DynamicStateAction::Serialize => {
                if !self.serialized_state_ptr.is_null() && self.serialized_state_len > 0 {
                    let Ok(free_fn) = (unsafe {
                        reloader.get_symbol::<fn(*mut u8, usize)>(b"free_serialized_data")
                    }) else {
                        log::warn!("Failed to get free_serialized_data function");
                        self.serialized_state_ptr = std::ptr::null_mut();
                        self.serialized_state_len = 0;
                        return Ok(());
                    };

                    free_fn(self.serialized_state_ptr, self.serialized_state_len);
                    self.serialized_state_ptr = std::ptr::null_mut();
                    self.serialized_state_len = 0;
                }

                let Ok(serialize_fn) = (unsafe {
                    reloader.get_symbol::<fn(&P::State, *mut *mut u8, *mut usize) -> Result<(), HotFunctionError>>(
                        action.function_name().as_bytes(),
                    )
                }) else {
                    log::info!("Failed to get state action function, assuming no dynamic state");
                    return Err(HotFunctionError::FunctionNotFound(action.function_name()));
                };

                log::info!("state action: {}", action.function_name());
                serialize_fn(
                    &self.state,
                    &mut self.serialized_state_ptr,
                    &mut self.serialized_state_len,
                )
                .inspect_err(|e| log::error!("{e}"))?;

                info!("Size of serialized state: {}", self.serialized_state_len);
            }
            DynamicStateAction::Deserialize => {
                let Ok(deserialize_fn) = (unsafe {
                    reloader.get_symbol::<fn(&mut P::State, *const u8, usize) -> Result<(), HotFunctionError>>(
                        action.function_name().as_bytes(),
                    )
                }) else {
                    log::info!("Failed to get state action function, assuming no dynamic state");
                    return Err(HotFunctionError::FunctionNotFound(action.function_name()));
                };

                log::info!("size of serialized state: {}", self.serialized_state_len);
                log::info!("state action: {}", action.function_name());
                deserialize_fn(
                    &mut self.state,
                    self.serialized_state_ptr,
                    self.serialized_state_len,
                )
                .inspect_err(|e| log::error!("Failed to deserialize state: {}", e))?;

                // Free the memory after successful deserialization
                if !self.serialized_state_ptr.is_null() && self.serialized_state_len > 0 {
                    let Ok(free_fn) = (unsafe {
                        reloader.get_symbol::<fn(*mut u8, usize)>(b"free_serialized_data")
                    }) else {
                        log::warn!("Failed to get free_serialized_data function");
                        // Continue anyway
                        self.serialized_state_ptr = std::ptr::null_mut();
                        self.serialized_state_len = 0;
                        return Ok(());
                    };

                    free_fn(self.serialized_state_ptr, self.serialized_state_len);
                    self.serialized_state_ptr = std::ptr::null_mut();
                    self.serialized_state_len = 0;
                }
            }
        }
        Ok(())
    }
}

fn build_args(library_name: &str) -> [&str; 11] {
    [
        "rustc",
        "--package",
        library_name,
        "--lib",
        "--crate-type",
        "dylib",
        "--profile",
        "dev",
        "--",
        "-C",
        "link-arg=-Wl,--whole-archive",
    ]
}

trait EnvVariables {
    fn environment_variables(&mut self, target_dir: &str) -> &mut Self;
}

impl EnvVariables for Command {
    fn environment_variables(&mut self, target_dir: &str) -> &mut Self {
        self.env("CARGO_PROFILE_DEV_OPT_LEVEL", "0")
            .env("CARGO_PROFILE_DEV_CODEGEN_UNITS", "1")
            .env("CARGO_PROFILE_DEV_DEBUG", "false")
            .env("CARGO_PROFILE_DEV_LTO", "false")
            .env("CARGO_TARGET_DIR", target_dir)
    }
}
