use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::Debug,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use cargo_metadata::{MetadataCommand, camino::Utf8PathBuf};
use crossfire::{AsyncRx, MAsyncRx, MTx, mpmc};
use hot_ice_common::{
    DESERIALIZE_STATE_FUNCTION_NAME, FREE_SERIALIZED_DATA_FUNCTION_NAME,
    SERIALIZE_STATE_FUNCTION_NAME,
};
use iced::Border;
use iced_core::{
    Alignment, Animation, Background, Color, Element, Length, Padding, Settings, Theme,
    animation::Easing,
    theme::{self, Base, Mode},
    time::Instant,
    window,
};
use iced_futures::{
    BoxStream, Subscription,
    futures::{Stream, StreamExt},
    stream,
    subscription::{self as iced_subscription, EventStream, Hasher, Recipe},
};
use iced_widget::{
    Stack, Text, button, column, container, container::Style as ContainerStyle, row, sensor, space,
    text::Style as TextStyle, themer,
};
use iced_winit::{
    program::Program,
    runtime::{Action, Task, task, window as runtime_window},
};
use log::info;
use thiserror::Error;

use crate::{
    error::HotIceError,
    executor::{CdylibWorker, DrainHandle},
    hot_program::HotProgram,
    lib_reloader::{LibReloader, RetiredLibrary},
    message::MessageSource,
};

const DEFAULT_TARGET_DIR: &str = "target/reload";
const DEFAULT_LIB_DIR: &str = "target/reload/debug";

/// Global handle to the cargo watch child process for cleanup on exit
static CARGO_WATCH_CHILD: OnceLock<Mutex<Option<Child>>> = OnceLock::new();

/// Kill the cargo watch process if it's running.
/// This is called automatically via atexit, but can also be called manually.
pub fn kill_cargo_watch() {
    if let Some(mutex) = CARGO_WATCH_CHILD.get() {
        if let Ok(mut guard) = mutex.lock() {
            if let Some(ref mut child) = *guard {
                log::info!("Killing cargo watch process (pid: {:?})", child.id());
                #[cfg(unix)]
                {
                    // Kill the entire process group
                    unsafe {
                        libc::kill(-(child.id() as i32), libc::SIGTERM);
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = child.kill();
                }
                let _ = child.wait();
            }
            *guard = None;
        }
    }
}

/// Register cleanup handler for cargo watch process
fn register_cleanup_handler() {
    use std::sync::Once;
    static REGISTER_ONCE: Once = Once::new();

    REGISTER_ONCE.call_once(|| {
        #[cfg(unix)]
        {
            // Register atexit handler on Unix
            extern "C" fn cleanup() {
                kill_cargo_watch();
            }
            unsafe {
                libc::atexit(cleanup);
            }
        }

        #[cfg(not(unix))]
        {
            // On non-Unix platforms, we rely on the child.kill() in kill_cargo_watch
            // which will be called if the process exits gracefully
        }
    });
}

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
    /// Maximum time to wait for in-flight async streams to complete during a
    /// hot reload before dropping them. Default: 5 seconds.
    pub drain_timeout: Duration,
    /// Optional cargo feature to enable when compiling the cdylib.
    /// When set, `--features <feature>` is appended to the build command.
    pub feature: Option<String>,
}

impl Default for ReloaderSettings {
    fn default() -> Self {
        Self {
            target_dir: DEFAULT_TARGET_DIR.to_string(),
            lib_dir: DEFAULT_LIB_DIR.to_string(),
            compile_in_reloader: true,
            file_watch_debounce: Duration::from_millis(25),
            watch_dir: None,
            drain_timeout: Duration::from_secs(5),
            feature: None,
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
    fonts: Vec<Cow<'static, [u8]>>,
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
        fonts: Vec<Cow<'static, [u8]>>,
    ) -> Self {
        Self {
            program,
            reloader_settings,
            settings,
            window_settings,
            lib_name,
            fonts,
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
        Reloader::new(
            &self.program,
            &self.reloader_settings,
            &self.lib_name,
            self.fonts.clone(),
        )
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

/// Wraps a `RetiredLibrary` so it can be passed through `Message` (which must
/// be `Clone`). The inner `Option` is `.take()`n by the handler.
type SharedRetired = Arc<Mutex<Option<RetiredLibrary>>>;

pub enum Message<P>
where
    P: HotProgram,
{
    CompilationComplete,
    AboutToReload,
    ReloadComplete(Option<SharedRetired>),
    SendReadySignal,
    Error(ReloaderError),
    AppMessage(MessageSource<P::Message>),
    ErrorShown(HotFunction),
    AutoDismissError(HotFunction),
    DismissError(HotFunction),
    ToggleErrorExpand(HotFunction),
    AnimationTick(Instant),
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
            Self::ReloadComplete(r) => Self::ReloadComplete(r.clone()),
            Self::CompilationComplete => Self::CompilationComplete,
            Self::Error(error) => Self::Error(error.clone()),
            Self::ErrorShown(func) => Self::ErrorShown(*func),
            Self::AutoDismissError(func) => Self::AutoDismissError(*func),
            Self::DismissError(func) => Self::DismissError(*func),
            Self::ToggleErrorExpand(func) => Self::ToggleErrorExpand(*func),
            Self::AnimationTick(t) => Self::AnimationTick(*t),
        }
    }
}

impl<P: HotProgram> Debug for Message<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AppMessage(message) => message.fmt(f),
            Self::SendReadySignal => write!(f, "SendReadySignal"),
            Self::AboutToReload => write!(f, "AboutToReload"),
            Self::ReloadComplete(_) => write!(f, "ReloadComplete"),
            Self::CompilationComplete => write!(f, "CompilationComplete"),
            Self::Error(error) => write!(f, "{}", error),
            Self::ErrorShown(func) => write!(f, "ErrorShown({})", func),
            Self::AutoDismissError(func) => write!(f, "AutoDismissError({})", func),
            Self::DismissError(func) => write!(f, "DismissError({})", func),
            Self::ToggleErrorExpand(func) => write!(f, "ToggleErrorExpand({})", func),
            Self::AnimationTick(_) => write!(f, "AnimationTick"),
        }
    }
}

pub struct ReadyToReload;

#[derive(Clone)]
pub enum FunctionState {
    None,
    Static,
    Hot,
    FallBackStatic(String),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotFunction {
    Update,
    Subscription,
    Theme,
    Style,
    Title,
    ScaleFactor,
}

impl std::fmt::Display for HotFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

struct ErrorEntry {
    message: String,
    handle: Option<task::Handle>,
    sensor_key: u16,
    expanded: bool,
    animation: Animation<bool>,
    dismissing: bool,
}

/// Wrapper recipe that routes subscription streams to the cdylib worker.
///
/// When the tracker calls `stream()`, this recipe:
/// 1. Gets the inner recipe's stream
/// 2. Maps items to `Action::Output` and sends to the worker
/// 3. Returns `pending()` to keep the tracker slot alive
///
/// Messages flow back to the event loop via `Proxy::send_action()` in the worker.
struct WorkerRecipe<M: Send + 'static> {
    inner: Box<dyn Recipe<Output = M>>,
    worker: Arc<CdylibWorker<M>>,
}

impl<M: Send + 'static> Recipe for WorkerRecipe<M> {
    type Output = M;

    fn hash(&self, state: &mut Hasher) {
        // Delegate to inner recipe — preserves subscription identity
        self.inner.hash(state);
    }

    fn stream(self: Box<Self>, input: EventStream) -> BoxStream<Self::Output> {
        // Get the inner stream (this is what the app recipe produces)
        let app_stream = self.inner.stream(input);

        // Map Message → Action<Message> and send to worker
        let action_stream = Box::pin(app_stream.map(Action::Output));
        self.worker.run_stream(action_stream);

        // Return a stream that never produces items — the tracker keeps
        // this subscription alive by hash, but actual messages flow back
        // via the worker's Proxy::send_action()
        Box::pin(futures::stream::pending())
    }
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
    worker: Option<Arc<CdylibWorker<Message<P>>>>,
    pending_drain: Option<DrainHandle<Message<P>>>,
    reloader_settings: ReloaderSettings,
    lib_name: &'static str,
    reloading_sensor_key: u16,
    update_fn_state: FunctionState,
    subscription_fn_state: Mutex<FunctionState>,
    theme_fn_state: Mutex<FunctionState>,
    style_fn_state: Mutex<FunctionState>,
    scale_factor_fn_state: Mutex<FunctionState>,
    title_fn_state: Mutex<FunctionState>,
    update_channel: UpdateChannel,
    loaded_fonts: Vec<Cow<'static, [u8]>>,
    active_errors: Mutex<HashMap<HotFunction, ErrorEntry>>,
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
        fonts: Vec<Cow<'static, [u8]>>,
    ) -> (Self, Task<Message<P>>) {
        let (state, program_task) = program.boot();

        let mut reloader = Self {
            state,
            serialized_state_ptr: std::ptr::null_mut(),
            serialized_state_len: 0,
            reloader_state: ReloaderState::Compiling,
            lib_reloader: None,
            worker: None,
            pending_drain: None,
            reloader_settings: reloader_settings.clone(),
            lib_name,
            reloading_sensor_key: 0,
            update_fn_state: FunctionState::Static,
            subscription_fn_state: Mutex::new(FunctionState::Static),
            theme_fn_state: Mutex::new(FunctionState::Static),
            style_fn_state: Mutex::new(FunctionState::Static),
            scale_factor_fn_state: Mutex::new(FunctionState::Static),
            title_fn_state: Mutex::new(FunctionState::Static),
            update_channel: mpmc::bounded_tx_blocking_rx_async(1),
            loaded_fonts: fonts,
            active_errors: Mutex::new(HashMap::new()),
        };

        let task = if reloader_settings.compile_in_reloader {
            Task::stream(Self::build_library(
                reloader.lib_name,
                reloader_settings.target_dir.clone(),
                reloader_settings.feature.clone(),
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

            reloader.sync_fonts_to_library();
            reloader.start_worker_from_library();

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

                match program.update(&mut self.state, message, self.lib_reloader.as_ref()) {
                    Ok((task, fn_state)) => {
                        self.update_fn_state = fn_state;
                        self.sync_error_state(HotFunction::Update, &self.update_fn_state);
                        self.intercept_app_task(task.map(Message::AppMessage))
                    }
                    Err(err) => {
                        log::error!("update(): {}", err);
                        self.update_fn_state = FunctionState::Error(err.to_string());
                        self.sync_error_state(HotFunction::Update, &self.update_fn_state);
                        Task::none()
                    }
                }
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

                self.sync_fonts_to_library();
                self.start_worker_from_library();

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
                    self.reloader_settings.feature.clone(),
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
                self.reloading_sensor_key += 1;
                Task::none()
            }
            Message::SendReadySignal => {
                self.serialize_state()
                    .inspect_err(|e| log::error!("{}", e))
                    .ok();

                // Begin draining the old worker instead of hard shutdown.
                // The worker stops accepting new streams and polls active ones
                // to completion (or until timeout). The actual thread join
                // happens later in a background cleanup thread spawned from
                // ReloadComplete.
                if let Some(worker_arc) = self.worker.take() {
                    log::info!("Beginning drain of cdylib worker before reload");
                    // Try to get sole ownership of the worker. Since WorkerRecipe
                    // instances are transient (consumed by stream()), we should
                    // have sole ownership here. If not, drop the Arc and let the
                    // worker clean up when all refs are gone.
                    match Arc::try_unwrap(worker_arc) {
                        Ok(worker) => {
                            let drain_handle =
                                worker.begin_drain(self.reloader_settings.drain_timeout);
                            self.pending_drain = Some(drain_handle);
                        }
                        Err(_arc) => {
                            log::warn!(
                                "Worker had outstanding references during shutdown, \
                                 dropping without drain"
                            );
                            // The Arc is dropped here; worker continues until all
                            // refs are gone and channel is closed
                        }
                    }
                }

                self.update_channel
                    .0
                    .send(ReadyToReload)
                    .expect("Update Channel closed");
                Task::none()
            }
            Message::ReloadComplete(retired_wrapper) => {
                match &self.reloader_state {
                    ReloaderState::Reloading(num) => {
                        if *num == 1 {
                            self.deserialize_state()
                                .inspect_err(|e| log::error!("{}", e))
                                .ok();

                            self.sync_fonts_to_library();
                            self.start_worker_from_library();

                            self.reloader_state = ReloaderState::Ready;
                            if let Ok(mut errors) = self.active_errors.lock() {
                                for entry in errors.values() {
                                    if let Some(h) = &entry.handle {
                                        h.abort();
                                    }
                                }
                                errors.clear();
                            }

                            // Spawn background cleanup thread: join the old
                            // (draining) worker, then drop the retired library.
                            let drain_handle = self.pending_drain.take();
                            let retired =
                                retired_wrapper.as_ref().and_then(|w| w.lock().ok()?.take());

                            if drain_handle.is_some() || retired.is_some() {
                                std::thread::Builder::new()
                                    .name("hot-ice-drain-cleanup".into())
                                    .spawn(move || {
                                        if let Some(h) = drain_handle {
                                            h.join();
                                        }
                                        if let Some(retired) = retired {
                                            log::info!(
                                                "Dropping retired library: {:?}",
                                                retired.file_path
                                            );
                                            drop(retired);
                                        }
                                        log::info!("hot-ice drain: cleanup thread finished");
                                    })
                                    .expect("spawn drain cleanup thread");
                            }
                        } else {
                            self.reloader_state = ReloaderState::Reloading(num - 1);
                            // Drop the retired library from a skipped reload
                            // immediately — no worker was using it.
                            if let Some(wrapper) = &retired_wrapper {
                                if let Ok(mut guard) = wrapper.lock() {
                                    guard.take();
                                }
                            }
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
            Message::ErrorShown(func) => {
                let mut errors = self.active_errors.lock().unwrap();
                if let Some(entry) = errors.get_mut(&func) {
                    if entry.dismissing {
                        return Task::none();
                    }
                    if let Some(h) = entry.handle.take() {
                        h.abort();
                    }
                    let (task, handle) = Task::future(async move {
                        futures_timer::Delay::new(Duration::from_secs(10)).await;
                        Message::AutoDismissError(func)
                    })
                    .abortable();
                    entry.handle = Some(handle);
                    drop(errors);
                    task
                } else {
                    Task::none()
                }
            }
            Message::AutoDismissError(func) | Message::DismissError(func) => {
                let mut errors = self.active_errors.lock().unwrap();
                if let Some(entry) = errors.get_mut(&func) {
                    if let Some(h) = entry.handle.take() {
                        h.abort();
                    }
                    if !entry.dismissing {
                        entry.dismissing = true;
                        entry.animation.go_mut(false, Instant::now());
                    }
                }
                drop(errors);
                Task::none()
            }
            Message::ToggleErrorExpand(func) => {
                let mut errors = self.active_errors.lock().unwrap();
                if let Some(entry) = errors.get_mut(&func) {
                    entry.expanded = !entry.expanded;
                    if entry.expanded {
                        // Expanding — abort countdown so it stays visible
                        if let Some(h) = entry.handle.take() {
                            h.abort();
                        }
                    } else {
                        // Collapsing — restart countdown
                        if let Some(h) = entry.handle.take() {
                            h.abort();
                        }
                        let (task, handle) = Task::future(async move {
                            futures_timer::Delay::new(Duration::from_secs(10)).await;
                            Message::AutoDismissError(func)
                        })
                        .abortable();
                        entry.handle = Some(handle);
                        drop(errors);
                        return task;
                    }
                }
                Task::none()
            }
            Message::AnimationTick(now) => {
                let mut errors = self.active_errors.lock().unwrap();
                errors.retain(|_, entry| {
                    if entry.dismissing && !entry.animation.is_animating(now) {
                        if let Some(h) = entry.handle.take() {
                            h.abort();
                        }
                        false
                    } else {
                        true
                    }
                });
                Task::none()
            }
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

        let program_view = match &self.reloader_state {
            ReloaderState::Ready => {
                match program.view(&self.state, window, self.lib_reloader.as_ref()) {
                    Ok((element, _fn_state)) => element.map(Message::AppMessage),
                    Err(err) => {
                        log::error!("view(): {}", err);
                        with_default_theme(
                            container(
                                container(
                                    column![
                                        iced_widget::Text::new("View Error")
                                            .center()
                                            .style(iced_widget::text::danger),
                                        iced_widget::Text::new(err.to_string())
                                    ]
                                    .width(Length::Fill)
                                    .spacing(30)
                                    .align_x(Alignment::Center),
                                )
                                .style(|theme| {
                                    let palette = theme.extended_palette();
                                    let mut style = iced_widget::container::dark(theme);
                                    style.border = Border {
                                        color: palette.warning.weak.color,
                                        radius: 15.0.into(),
                                        width: 2.0,
                                    };
                                    style
                                })
                                .center(Length::Shrink)
                                .padding(20),
                            )
                            .center(Length::Fill)
                            .padding(100)
                            .into(),
                        )
                    }
                }
            }
            ReloaderState::Reloading(_) => {
                let reloading_message = container(
                    sensor(Text::new("Reloading...").size(20))
                        .key(self.reloading_sensor_key)
                        .on_show(|_| Message::SendReadySignal),
                )
                .center(Length::Fill);

                with_default_theme(Element::from(reloading_message))
            }
            ReloaderState::Error(error) => {
                let error_text =
                    container(Text::new(error.to_string()).size(20)).center(Length::Fill);

                with_default_theme(Element::from(error_text))
            }
            ReloaderState::Compiling => {
                let compilation_message =
                    container(Text::new("Compiling...").size(20)).center(Length::Fill);

                with_default_theme(Element::from(compilation_message))
            }
        };

        // Build error bar from active_errors HashMap.
        let now = Instant::now();
        let errors = self.active_errors.lock().unwrap();
        if errors.is_empty() {
            drop(errors);
            program_view
        } else {
            let mut error_col = column![].spacing(2);
            let mut max_t: f32 = 0.0;

            for (&func, entry) in errors.iter() {
                let t: f32 = entry.animation.interpolate(0.0f32, 1.0f32, now);
                if t < 0.001 && entry.dismissing {
                    continue;
                }
                if t > max_t {
                    max_t = t;
                }

                let text_alpha = t;
                let detail_alpha = 0.7 * t;

                let toggle_label = if entry.expanded {
                    "Show less"
                } else {
                    "Read more"
                };

                let summary = row![
                    Text::new(format!("Error: {}", func))
                        .style(move |_| TextStyle {
                            color: Some(Color::from_rgba8(225, 29, 72, text_alpha)),
                        })
                        .size(13),
                    space().width(Length::Fill),
                    button(Text::new(toggle_label).size(12).style(move |_| TextStyle {
                        color: Some(Color::from_rgba(1.0, 1.0, 1.0, text_alpha)),
                    }),)
                    .on_press(Message::ToggleErrorExpand(func))
                    .style(button::text),
                    button(Text::new("X").size(12).style(move |_| TextStyle {
                        color: Some(Color::from_rgba(1.0, 1.0, 1.0, text_alpha)),
                    }),)
                    .on_press(Message::DismissError(func))
                    .style(button::text),
                ]
                .spacing(8)
                .align_y(Alignment::Center);

                let error_row = if entry.expanded {
                    column![
                        summary,
                        Text::new(entry.message.clone())
                            .style(move |_| TextStyle {
                                color: Some(Color::from_rgba8(225, 29, 72, detail_alpha)),
                            })
                            .size(11),
                    ]
                    .spacing(4)
                } else {
                    column![summary]
                };

                let sensor_key = entry.sensor_key;
                error_col = error_col.push(
                    sensor(error_row)
                        .key(sensor_key)
                        .on_show(move |_| Message::ErrorShown(func)),
                );
            }
            drop(errors);

            let error_bar = with_default_theme(Element::from(
                container(error_col)
                    .style(move |_| ContainerStyle {
                        background: Some(Background::Color(Color::from_rgba(
                            0.0,
                            0.0,
                            0.0,
                            0.85 * max_t,
                        ))),
                        ..Default::default()
                    })
                    .width(Length::Fill)
                    .padding(Padding {
                        top: 6.,
                        bottom: 6.,
                        left: 16.,
                        right: 16.,
                    }),
            ));

            Stack::new()
                .push(program_view)
                .push(error_bar)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
    }

    pub fn subscription(&self, program: &P) -> Subscription<Message<P>> {
        let app_sub = if self.reloader_state == ReloaderState::Ready {
            match program.subscription(&self.state, self.lib_reloader.as_ref()) {
                Ok((sub, fn_state)) => {
                    if let Ok(mut state) = self.subscription_fn_state.try_lock() {
                        *state = fn_state.clone();
                    }
                    self.sync_error_state(HotFunction::Subscription, &fn_state);
                    sub.map(Message::AppMessage)
                }
                Err(err) => {
                    log::error!("subscription(): {}", err);
                    let fn_state = FunctionState::Error(err.to_string());
                    if let Ok(mut state) = self.subscription_fn_state.try_lock() {
                        *state = fn_state.clone();
                    }
                    self.sync_error_state(HotFunction::Subscription, &fn_state);
                    Subscription::none()
                }
            }
        } else {
            log::debug!("Called subscription when Reloader was not ready");
            Subscription::none()
        };

        // Wrap app recipes to run on the cdylib worker. This ensures that
        // tokio::spawn() and similar calls inside subscription streams find
        // the cdylib's executor TLS context, not the host binary's.
        let app_sub = if let Some(worker) = &self.worker {
            let recipes = iced_subscription::into_recipes(app_sub);
            if recipes.is_empty() {
                Subscription::none()
            } else {
                let wrapped = recipes.into_iter().map(|recipe| {
                    iced_subscription::from_recipe(WorkerRecipe {
                        inner: recipe,
                        worker: Arc::clone(worker),
                    })
                });
                Subscription::batch(wrapped)
            }
        } else {
            app_sub
        };

        let needs_frames = {
            let now = Instant::now();
            self.active_errors
                .lock()
                .map(|errors| errors.values().any(|e| e.animation.is_animating(now)))
                .unwrap_or(false)
        };

        if needs_frames {
            Subscription::batch([
                app_sub,
                runtime_window::frames().map(Message::AnimationTick),
            ])
        } else {
            app_sub
        }
    }

    pub fn title(&self, program: &P, window: window::Id) -> String {
        if self.reloader_state == ReloaderState::Ready {
            match program.title(&self.state, window, self.lib_reloader.as_ref()) {
                Ok((title, fn_state)) => {
                    if let Ok(mut state) = self.title_fn_state.lock() {
                        *state = fn_state;
                    }
                    format!("Hot-Reloading: {}", title)
                }
                Err(err) => {
                    log::error!("title(): {}", err);
                    let fn_state = FunctionState::Error(err.to_string());
                    if let Ok(mut state) = self.title_fn_state.lock() {
                        *state = fn_state.clone();
                    }
                    self.sync_error_state(HotFunction::Title, &fn_state);
                    "An ice-hot application".to_string()
                }
            }
        } else {
            log::debug!("Called title when Reloader was not ready");
            String::from("Reloader")
        }
    }

    pub fn theme(&self, program: &P, window: window::Id) -> Option<P::Theme> {
        if self.reloader_state == ReloaderState::Ready {
            match program.theme(&self.state, window, self.lib_reloader.as_ref()) {
                Ok((theme, fn_state)) => {
                    if let Ok(mut state) = self.theme_fn_state.lock() {
                        *state = fn_state;
                    }
                    theme
                }
                Err(err) => {
                    log::error!("theme(): {}", err);
                    let fn_state = FunctionState::Error(err.to_string());
                    if let Ok(mut state) = self.theme_fn_state.lock() {
                        *state = fn_state.clone();
                    }
                    self.sync_error_state(HotFunction::Theme, &fn_state);
                    None
                }
            }
        } else {
            log::debug!("Called theme when Reloader was not ready");
            None
        }
    }

    pub fn style(&self, program: &P, theme: &P::Theme) -> theme::Style {
        if self.reloader_state == ReloaderState::Ready {
            match program.style(&self.state, theme, self.lib_reloader.as_ref()) {
                Ok((style, fn_state)) => {
                    if let Ok(mut state) = self.style_fn_state.lock() {
                        *state = fn_state;
                    }
                    style
                }
                Err(err) => {
                    log::error!("style(): {}", err);
                    let fn_state = FunctionState::Error(err.to_string());
                    if let Ok(mut state) = self.style_fn_state.lock() {
                        *state = fn_state.clone();
                    }
                    self.sync_error_state(HotFunction::Style, &fn_state);
                    theme.base()
                }
            }
        } else {
            log::debug!("Called style when Reloader was not ready");
            theme.base()
        }
    }

    pub fn scale_factor(&self, program: &P, window: window::Id) -> f32 {
        if self.reloader_state == ReloaderState::Ready {
            match program.scale_factor(&self.state, window, self.lib_reloader.as_ref()) {
                Ok((factor, fn_state)) => {
                    if let Ok(mut state) = self.scale_factor_fn_state.lock() {
                        *state = fn_state;
                    }
                    factor
                }
                Err(err) => {
                    log::error!("scale_factor(): {}", err);
                    let fn_state = FunctionState::Error(err.to_string());
                    if let Ok(mut state) = self.scale_factor_fn_state.lock() {
                        *state = fn_state.clone();
                    }
                    self.sync_error_state(HotFunction::ScaleFactor, &fn_state);
                    1.0
                }
            }
        } else {
            log::debug!("Called scale_factor when Reloader was not ready");
            1.0
        }
    }

    fn sync_error_state(&self, func: HotFunction, fn_state: &FunctionState) {
        let Ok(mut errors) = self.active_errors.lock() else {
            return;
        };
        let now = Instant::now();
        match fn_state {
            FunctionState::Error(msg) => {
                use std::collections::hash_map::Entry;
                match errors.entry(func) {
                    Entry::Occupied(mut e) => {
                        if e.get().dismissing {
                            // Was being dismissed but error came back — re-animate in
                            if let Some(h) = &e.get().handle {
                                h.abort();
                            }
                            let old_key = e.get().sensor_key;
                            let mut anim = Animation::new(false).quick().easing(Easing::EaseOut);
                            anim.go_mut(true, now);
                            e.insert(ErrorEntry {
                                message: msg.clone(),
                                handle: None,
                                sensor_key: old_key.wrapping_add(1),
                                expanded: false,
                                animation: anim,
                                dismissing: false,
                            });
                        }
                        if e.get().message != *msg {
                            // Error message changed — new error
                            if let Some(h) = &e.get().handle {
                                h.abort();
                            }
                            let old_key = e.get().sensor_key;
                            let mut anim = Animation::new(false).quick().easing(Easing::EaseOut);
                            anim.go_mut(true, now);
                            e.insert(ErrorEntry {
                                message: msg.clone(),
                                handle: None,
                                sensor_key: old_key.wrapping_add(1),
                                expanded: false,
                                animation: anim,
                                dismissing: false,
                            });
                        }
                    }
                    Entry::Vacant(e) => {
                        let mut anim = Animation::new(false).quick().easing(Easing::EaseOut);
                        anim.go_mut(true, now);
                        e.insert(ErrorEntry {
                            message: msg.clone(),
                            handle: None,
                            sensor_key: 0,
                            expanded: false,
                            animation: anim,
                            dismissing: false,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    fn build_library(
        lib_crate_name: &'static str,
        target_dir: String,
        feature: Option<String>,
    ) -> impl Stream<Item = Message<P>> {
        stream::channel(200, async move |mut output| {
            let metadata = MetadataCommand::new()
                .exec()
                .expect("Failed to get cargo metadata");

            let workspace_root = metadata.workspace_root.as_std_path();
            info!(
                "Working directory for build command: {}",
                workspace_root.display()
            );

            let result = Command::new("cargo")
                .current_dir(workspace_root)
                .args(build_args(lib_crate_name, feature.as_deref()))
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
        feature: Option<String>,
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

            let mut command = Command::new("cargo");
            command
                .current_dir(&workspace_root)
                .arg("watch")
                .arg("-w")
                .arg(watch_dir)
                .arg("-d")
                .arg("0.01")
                .arg("-x")
                .arg(build_args(lib_crate_name, feature.as_deref()).join(" "))
                .environment_variables(&target_dir)
                .stderr(Stdio::piped());

            // On Unix, set up process group and death signal so child dies when parent dies
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                // SAFETY: These are async-signal-safe operations
                unsafe {
                    command.pre_exec(|| {
                        // Create a new process group with this process as the leader
                        libc::setpgid(0, 0);
                        // Set the process to receive SIGTERM when the parent dies
                        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                        Ok(())
                    });
                }
            }

            let result = command.spawn();

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

            log::info!("cargo watch started successfully (pid: {})", child.id());

            // Register the cleanup handler and store the child handle
            register_cleanup_handler();

            // Take stderr before storing the child
            let stderr = child.stderr.take();

            // Store the child handle globally for cleanup
            let child_mutex = CARGO_WATCH_CHILD.get_or_init(|| Mutex::new(None));
            if let Ok(mut guard) = child_mutex.lock() {
                *guard = Some(child);
            }

            if let Some(stderr) = stderr {
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
                        match reloader.update() {
                            Ok(result) => {
                                let retired_wrapper =
                                    if let crate::lib_reloader::UpdateResult::Reloaded { retired } =
                                        result
                                    {
                                        retired.map(|r| {
                                            log::info!(
                                                "Library reloaded, retired old: {:?}",
                                                r.file_path
                                            );
                                            Arc::new(Mutex::new(Some(r)))
                                        })
                                    } else {
                                        None
                                    };

                                if let Err(err) =
                                    output.try_send(Message::ReloadComplete(retired_wrapper))
                                {
                                    log::error!("Failed to send reload complete message: {err}");
                                }
                                break;
                            }
                            Err(err) => log::error!("{err}"),
                        }
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }

                log::info!("Reload complete");
            }
        })
    }

    fn serialize_state(&mut self) -> Result<(), HotIceError> {
        let reloader = self
            .lib_reloader
            .as_ref()
            .expect("reloader not initialized");

        let Ok(reloader) = reloader.lock() else {
            return Err(HotIceError::LockAcquisitionError);
        };

        if !self.serialized_state_ptr.is_null() && self.serialized_state_len > 0 {
            let Ok(free_fn) = (unsafe {
                reloader
                    .get_symbol::<fn(*mut u8, usize)>(FREE_SERIALIZED_DATA_FUNCTION_NAME.as_bytes())
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
            reloader
                .get_symbol::<fn(&P::State, *mut *mut u8, *mut usize) -> Result<(), HotIceError>>(
                    SERIALIZE_STATE_FUNCTION_NAME.as_bytes(),
                )
        }) else {
            return Err(HotIceError::FunctionNotFound(SERIALIZE_STATE_FUNCTION_NAME));
        };

        serialize_fn(
            &self.state,
            &mut self.serialized_state_ptr,
            &mut self.serialized_state_len,
        )?;

        info!("Size of serialized state: {}", self.serialized_state_len);
        Ok(())
    }

    fn deserialize_state(&mut self) -> Result<(), HotIceError> {
        let reloader = self
            .lib_reloader
            .as_ref()
            .expect("reloader not initialized");

        let Ok(reloader) = reloader.lock() else {
            return Err(HotIceError::LockAcquisitionError);
        };

        let Ok(deserialize_fn) = (unsafe {
            reloader.get_symbol::<fn(&mut P::State, *const u8, usize) -> Result<(), HotIceError>>(
                DESERIALIZE_STATE_FUNCTION_NAME.as_bytes(),
            )
        }) else {
            return Err(HotIceError::FunctionNotFound(
                DESERIALIZE_STATE_FUNCTION_NAME,
            ));
        };

        deserialize_fn(
            &mut self.state,
            self.serialized_state_ptr,
            self.serialized_state_len,
        )?;

        // Free the memory after successful deserialization
        if !self.serialized_state_ptr.is_null() && self.serialized_state_len > 0 {
            let Ok(free_fn) = (unsafe {
                reloader
                    .get_symbol::<fn(*mut u8, usize)>(FREE_SERIALIZED_DATA_FUNCTION_NAME.as_bytes())
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

        Ok(())
    }

    /// Sync all tracked fonts to the loaded library's font system
    fn sync_fonts_to_library(&self) {
        log::debug!(
            "sync_fonts_to_library called with {} fonts",
            self.loaded_fonts.len()
        );

        let Some(lib_reloader) = &self.lib_reloader else {
            log::debug!("lib_reloader is None");
            return;
        };

        let Ok(reloader) = lib_reloader.lock() else {
            log::debug!("Failed to acquire lock on lib_reloader");
            return;
        };

        log::debug!("Attempting to get font loading function symbol");

        // Get the font loading function from the library
        let Ok(load_font_fn) = (unsafe {
            reloader.get_symbol::<fn(*const u8, usize)>(
                hot_ice_common::LOAD_FONT_FUNCTION_NAME.as_bytes(),
            )
        }) else {
            log::debug!(
                "Font loading function not found in library. Function name: {}",
                hot_ice_common::LOAD_FONT_FUNCTION_NAME
            );
            return;
        };

        log::debug!(
            "Font loading function found, loading {} fonts",
            self.loaded_fonts.len()
        );

        // Load each tracked font into the library
        for (i, font_cow) in self.loaded_fonts.iter().enumerate() {
            let font_bytes: &[u8] = font_cow.as_ref();
            log::debug!("Loading font {} with {} bytes", i, font_bytes.len());
            load_font_fn(font_bytes.as_ptr(), font_bytes.len());
        }

        log::info!("Synced {} fonts to loaded library", self.loaded_fonts.len());
    }

    /// Starts a cdylib worker thread from the currently loaded library.
    ///
    /// The worker thread runs inside the cdylib's executor TLS context,
    /// allowing `tokio::spawn()` and similar calls to work correctly.
    /// Streams from app tasks are sent to the worker for async polling.
    fn start_worker_from_library(&mut self) {
        let Some(lib_reloader) = &self.lib_reloader else {
            log::warn!("Cannot start worker: lib_reloader is None");
            return;
        };

        let Ok(lib) = lib_reloader.lock() else {
            log::error!("Cannot start worker: failed to lock lib_reloader");
            return;
        };

        let Some(proxy) = crate::executor::get_global_proxy::<Message<P>>() else {
            log::error!("Cannot start worker: global proxy not set");
            return;
        };

        match unsafe { CdylibWorker::start(&lib, proxy) } {
            Ok(worker) => {
                log::info!("Started cdylib worker thread");
                self.worker = Some(Arc::new(worker));
            }
            Err(err) => {
                log::warn!("Failed to start worker from library: {}", err);
                // Not fatal — the library may not export worker functions
                // (e.g. if export_executor! was not used)
            }
        }
    }

    /// Intercepts a task returned by app update, sending its stream to the
    /// cdylib worker thread for polling.
    ///
    /// The worker runs inside the cdylib's executor TLS context, so async
    /// futures that call `tokio::spawn()` work correctly. Actions produced
    /// by the stream are sent back to the event loop via `Proxy::send_action()`.
    ///
    /// If no worker is running (e.g. the library doesn't export executor
    /// functions), the task is returned as-is and runs on the binary's
    /// executor — which works for non-tokio-dependent tasks.
    fn intercept_app_task(&self, task: Task<Message<P>>) -> Task<Message<P>> {
        let Some(worker) = self.worker.as_ref() else {
            return task;
        };

        let Some(stream) = iced_winit::runtime::task::into_stream(task) else {
            return Task::none();
        };

        worker.run_stream(stream);
        Task::none()
    }
}

fn build_args<'a>(library_name: &'a str, feature: Option<&'a str>) -> Vec<&'a str> {
    let mut args = vec![
        "rustc",
        "--package",
        library_name,
        "--lib",
        "--crate-type",
        "cdylib",
        "--profile",
        "dev",
    ];
    if let Some(feature) = feature {
        args.push("--features");
        args.push(feature);
    }
    args
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
