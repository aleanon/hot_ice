use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use iced_core::{theme, window, Element, Font, Settings, Size};
use iced_futures::{Executor, Subscription};
use iced_winit::{
    graphics::compositor,
    program::{self, Program},
    runtime::Task,
};

use crate::{
    hot_fn::{HotFn, HotUpdate, HotView},
    lib_reloader::LibReloader,
    reloader::{
        ReadyToReload, Reload, ReloadEvent, LIB_RELOADER, SUBSCRIPTION_CHANNEL, UPDATE_CHANNEL,
    },
};

pub fn hot_application<State, Message, Theme, Renderer>(
    dylib_path: &'static str,
    boot: impl Boot<State, Message>,
    update: impl Update<State, Message>,
    view: impl for<'a> self::View<'a, State, Message, Theme, Renderer>,
) -> HotIce<impl Program<State = State, Message = Message, Theme = Theme>>
where
    State: 'static,
    Message: Send + std::fmt::Debug + 'static + Clone,
    Theme: Default + theme::Base,
    Renderer: iced_core::text::Renderer + compositor::Default,
{
    let hot_view = HotView::new(view);
    let hot_update = HotUpdate::new(update);

    initiate_lib_reloaders(&hot_view, &hot_update, dylib_path);

    struct Instance<State, Message, Theme, Renderer, Boot, Update, View> {
        boot: Boot,
        update: HotUpdate<Update, State, Message>,
        view: HotView<View, State, Message, Theme, Renderer>,
    }

    impl<State, Message, Theme, Renderer, Boot, Update, View> Program
        for Instance<State, Message, Theme, Renderer, Boot, Update, View>
    where
        State: 'static,
        Message: Send + std::fmt::Debug + 'static + Clone,
        Theme: Default + theme::Base,
        Renderer: iced_core::text::Renderer + compositor::Default,
        Boot: self::Boot<State, Message>,
        Update: self::Update<State, Message>,
        View: for<'a> self::View<'a, State, Message, Theme, Renderer>,
    {
        type State = State;
        type Message = Message;
        type Theme = Theme;
        type Renderer = Renderer;
        type Executor = iced_futures::backend::default::Executor;

        fn name() -> &'static str {
            let name = std::any::type_name::<State>();

            name.split("::").next().unwrap_or("a_cool_application")
        }

        fn boot(&self) -> (State, Task<Message>) {
            self.boot.boot()
        }

        fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
            self.update.update(state, message)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            _window: window::Id,
        ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
            self.view.view(state).into()
        }
    }

    HotIce {
        program: Instance {
            boot,
            update: hot_update,
            view: hot_view,
        },
        settings: Settings::default(),
        window: window::Settings::default(),
    }
}

pub struct HotIce<P>
where
    P: Program,
{
    program: P,
    settings: Settings,
    window: window::Settings,
}

impl<P> HotIce<P>
where
    P: Program + 'static,
    P::Message: Clone,
{
    pub fn run(self) -> Result<(), ()> {
        let program = Reload::new(self.program);

        #[cfg(all(feature = "debug", not(target_arch = "wasm32")))]
        let program = {
            iced_debug::init(iced_debug::Metadata {
                name: P::name(),
                theme: None,
                can_time_travel: cfg!(feature = "time-travel"),
            });

            iced_devtools::attach(program)
        };

        Ok(iced_winit::run(program, self.settings, Some(self.window)).map_err(|_| ())?)
    }

    /// Sets the [`Settings`] that will be used to run the [`Application`].
    pub fn settings(self, settings: Settings) -> Self {
        Self { settings, ..self }
    }

    /// Sets the [`Settings::antialiasing`] of the [`Application`].
    pub fn antialiasing(self, antialiasing: bool) -> Self {
        Self {
            settings: Settings {
                antialiasing,
                ..self.settings
            },
            ..self
        }
    }

    /// Sets the default [`Font`] of the [`Application`].
    pub fn default_font(self, default_font: Font) -> Self {
        Self {
            settings: Settings {
                default_font,
                ..self.settings
            },
            ..self
        }
    }

    /// Adds a font to the list of fonts that will be loaded at the start of the [`Application`].
    pub fn font(mut self, font: impl Into<Cow<'static, [u8]>>) -> Self {
        self.settings.fonts.push(font.into());
        self
    }

    /// Sets the [`window::Settings`] of the [`Application`].
    ///
    /// Overwrites any previous [`window::Settings`].
    pub fn window(self, window: window::Settings) -> Self {
        Self { window, ..self }
    }

    /// Sets the [`window::Settings::position`] to [`window::Position::Centered`] in the [`Application`].
    pub fn centered(self) -> Self {
        Self {
            window: window::Settings {
                position: window::Position::Centered,
                ..self.window
            },
            ..self
        }
    }

    /// Sets the [`window::Settings::exit_on_close_request`] of the [`Application`].
    pub fn exit_on_close_request(self, exit_on_close_request: bool) -> Self {
        Self {
            window: window::Settings {
                exit_on_close_request,
                ..self.window
            },
            ..self
        }
    }

    /// Sets the [`window::Settings::size`] of the [`Application`].
    pub fn window_size(self, size: impl Into<Size>) -> Self {
        Self {
            window: window::Settings {
                size: size.into(),
                ..self.window
            },
            ..self
        }
    }

    /// Sets the [`window::Settings::transparent`] of the [`Application`].
    pub fn transparent(self, transparent: bool) -> Self {
        Self {
            window: window::Settings {
                transparent,
                ..self.window
            },
            ..self
        }
    }

    /// Sets the [`window::Settings::resizable`] of the [`Application`].
    pub fn resizable(self, resizable: bool) -> Self {
        Self {
            window: window::Settings {
                resizable,
                ..self.window
            },
            ..self
        }
    }

    /// Sets the [`window::Settings::decorations`] of the [`Application`].
    pub fn decorations(self, decorations: bool) -> Self {
        Self {
            window: window::Settings {
                decorations,
                ..self.window
            },
            ..self
        }
    }

    /// Sets the [`window::Settings::position`] of the [`Application`].
    pub fn position(self, position: window::Position) -> Self {
        Self {
            window: window::Settings {
                position,
                ..self.window
            },
            ..self
        }
    }

    /// Sets the [`window::Settings::level`] of the [`Application`].
    pub fn level(self, level: window::Level) -> Self {
        Self {
            window: window::Settings {
                level,
                ..self.window
            },
            ..self
        }
    }

    /// Sets the [`Title`] of the [`Application`].
    pub fn title(
        self,
        title: impl Title<P::State>,
    ) -> HotIce<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>> {
        HotIce {
            program: program::with_title(self.program, move |state, _window| title.title(state)),
            settings: self.settings,
            window: self.window,
        }
    }

    /// Sets the subscription logic of the [`Application`].
    pub fn subscription(
        self,
        f: impl Fn(&P::State) -> Subscription<P::Message>,
    ) -> HotIce<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>> {
        HotIce {
            program: program::with_subscription(self.program, f),
            settings: self.settings,
            window: self.window,
        }
    }

    /// Sets the theme logic of the [`Application`].
    pub fn theme(
        self,
        f: impl Fn(&P::State) -> P::Theme,
    ) -> HotIce<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>> {
        HotIce {
            program: program::with_theme(self.program, move |state, _window| f(state)),
            settings: self.settings,
            window: self.window,
        }
    }

    /// Sets the style logic of the [`Application`].
    pub fn style(
        self,
        f: impl Fn(&P::State, &P::Theme) -> theme::Style,
    ) -> HotIce<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>> {
        HotIce {
            program: program::with_style(self.program, f),
            settings: self.settings,
            window: self.window,
        }
    }

    /// Sets the scale factor of the [`Application`].
    pub fn scale_factor(
        self,
        f: impl Fn(&P::State) -> f64,
    ) -> HotIce<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>> {
        HotIce {
            program: program::with_scale_factor(self.program, move |state, _window| f(state)),
            settings: self.settings,
            window: self.window,
        }
    }

    /// Sets the executor of the [`Application`].
    pub fn executor<E>(
        self,
    ) -> HotIce<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>>
    where
        E: Executor,
    {
        HotIce {
            program: program::with_executor::<P, E>(self.program),
            settings: self.settings,
            window: self.window,
        }
    }
}

/// The logic to initialize the `State` of some [`Application`].
///
/// This trait is implemented for both `Fn() -> State` and
/// `Fn() -> (State, Task<Message>)`.
///
/// In practice, this means that [`application`] can both take
/// simple functions like `State::default` and more advanced ones
/// that return a [`Task`].
pub trait Boot<State, Message> {
    /// Initializes the [`Application`] state.
    fn boot(&self) -> (State, Task<Message>);
}

impl<T, C, State, Message> Boot<State, Message> for T
where
    T: Fn() -> C,
    C: IntoBoot<State, Message>,
{
    fn boot(&self) -> (State, Task<Message>) {
        self().into_boot()
    }
}

/// The initial state of some [`Application`].
pub trait IntoBoot<State, Message> {
    /// Turns some type into the initial state of some [`Application`].
    fn into_boot(self) -> (State, Task<Message>);
}

impl<State, Message> IntoBoot<State, Message> for State {
    fn into_boot(self) -> (State, Task<Message>) {
        (self, Task::none())
    }
}

impl<State, Message> IntoBoot<State, Message> for (State, Task<Message>) {
    fn into_boot(self) -> (State, Task<Message>) {
        self
    }
}

/// The title logic of some [`Application`].
///
/// This trait is implemented both for `&static str` and
/// any closure `Fn(&State) -> String`.
///
/// This trait allows the [`application`] builder to take any of them.
pub trait Title<State> {
    /// Produces the title of the [`Application`].
    fn title(&self, state: &State) -> String;
}

impl<State> Title<State> for &'static str {
    fn title(&self, _state: &State) -> String {
        self.to_string()
    }
}

impl<T, State> Title<State> for T
where
    T: Fn(&State) -> String,
{
    fn title(&self, state: &State) -> String {
        self(state)
    }
}

/// The update logic of some [`Application`].
///
/// This trait allows the [`application`] builder to take any closure that
/// returns any `Into<Task<Message>>`.
pub trait Update<State, Message> {
    /// Processes the message and updates the state of the [`Application`].
    fn update(&self, state: &mut State, message: Message) -> impl Into<Task<Message>>;
}

impl<State, Message> Update<State, Message> for () {
    fn update(&self, _state: &mut State, _message: Message) -> impl Into<Task<Message>> {}
}
impl<T, State, Message, C> Update<State, Message> for T
where
    T: Fn(&mut State, Message) -> C,
    C: Into<Task<Message>>,
{
    fn update(&self, state: &mut State, message: Message) -> impl Into<Task<Message>> {
        self(state, message)
    }
}

/// The view logic of some [`Application`].
///
/// This trait allows the [`application`] builder to take any closure that
/// returns any `Into<Element<'_, Message>>`.
pub trait View<'a, State, Message, Theme, Renderer> {
    /// Produces the widget of the [`Application`].
    fn view(&self, state: &'a State) -> impl Into<Element<'a, Message, Theme, Renderer>>;
}

impl<'a, T, State, Message, Theme, Renderer, Widget> View<'a, State, Message, Theme, Renderer> for T
where
    T: Fn(&'a State) -> Widget,
    State: 'static,
    Widget: Into<Element<'a, Message, Theme, Renderer>>,
{
    fn view(&self, state: &'a State) -> impl Into<Element<'a, Message, Theme, Renderer>> {
        self(state)
    }
}

pub fn initiate_lib_reloaders(
    hot_view: &impl HotFn,
    hot_update: &impl HotFn,
    dylib_path: &'static str,
) {
    let mut lib_reloaders = HashMap::new();
    register_hot_lib(&mut lib_reloaders, hot_view, dylib_path);
    register_hot_lib(&mut lib_reloaders, hot_update, dylib_path);

    LIB_RELOADER.set(lib_reloaders).ok();
}

pub fn register_hot_lib(
    lib_reloaders: &mut HashMap<&'static str, Arc<Mutex<LibReloader>>>,
    f: &impl HotFn,
    dylib_path: &'static str,
) {
    lib_reloaders.entry(f.module()).or_insert_with(|| {
        let (_, update_ch_rx) = UPDATE_CHANNEL
            .get_or_init(|| crossfire::mpmc::bounded_tx_future_rx_blocking(1))
            .clone();
        let (subscription_ch_tx, _) = SUBSCRIPTION_CHANNEL
            .get_or_init(|| crossfire::mpmc::bounded_tx_blocking_rx_future(1))
            .clone();

        let mut lib_reloader = LibReloader::new(
            dylib_path,
            f.module(),
            Some(Duration::from_millis(10)),
            None,
        )
        .expect("Unable to create LibReloader");
        let change_subscriber = lib_reloader.subscribe_to_file_changes();
        let lib_reloader = Arc::new(Mutex::new(lib_reloader));
        let lib = lib_reloader.clone();

        std::thread::spawn(move || loop {
            let Ok(_) = change_subscriber.recv() else {
                panic!("Sub channel closed")
            };
            if let Err(err) = subscription_ch_tx.send(ReloadEvent::AboutToReload) {
                println!("{err}")
            }

            let Ok(ReadyToReload) = update_ch_rx.recv() else {
                panic!("Update Channel closed")
            };
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

            if let Err(_) = subscription_ch_tx.send(ReloadEvent::ReloadComplete) {
                panic!("Subscription Channel closed")
            }
        });
        lib_reloader
    });
}
