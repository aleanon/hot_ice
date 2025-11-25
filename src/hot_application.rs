use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use iced_core::{Element, Font, Settings, Size, theme, window};
use iced_futures::Executor;
use iced_winit::{Error, runtime::Task};

use crate::{
    DynMessage, boot,
    hot_fn::HotFn,
    hot_program::{self, HotProgram},
    hot_subscription::IntoHotSubscription,
    hot_theme::ThemeFn,
    hot_update::{self, HotUpdate},
    hot_view::{self, HotView},
    lib_reloader::LibReloader,
    message::MessageSource,
    reloader::{
        FunctionState, LIB_RELOADER, Reload, ReloadEvent, SUBSCRIPTION_CHANNEL, UPDATE_CHANNEL,
    },
};

pub fn hot_application<State, Message, Theme, Renderer>(
    dylib_path: &'static str,
    boot: impl boot::Boot<State, Message>,
    update: impl hot_update::IntoHotUpdate<State, Message>,
    view: impl for<'a> hot_view::IntoHotView<'a, State, Message, Theme, Renderer>,
) -> HotIce<impl HotProgram<State = State, Message = Message, Theme = Theme, Renderer = Renderer>>
where
    State: 'static,
    Message: DynMessage + Clone,
    Theme: theme::Base,
    Renderer: hot_program::Renderer,
{
    let hot_view = HotView::new(view);
    let hot_update = HotUpdate::new(update);

    assert_eq!(
        hot_view.lib_name, hot_update.lib_name,
        "Application must be defined in a single library crate"
    );

    initiate_lib_reloaders(&hot_view, &hot_update, dylib_path);

    struct Instance<State, Message, Theme, Renderer, Boot, Update, View> {
        boot: Boot,
        update: HotUpdate<Update, State, Message>,
        view: HotView<View, State, Message, Theme, Renderer>,
    }

    impl<State, Message, Theme, Renderer, Boot, Update, View> HotProgram
        for Instance<State, Message, Theme, Renderer, Boot, Update, View>
    where
        State: 'static,
        Message: DynMessage + Clone,
        Theme: theme::Base,
        Renderer: hot_program::Renderer,
        Boot: boot::Boot<State, Message>,
        Update: hot_update::IntoHotUpdate<State, Message>,
        View: for<'a> hot_view::IntoHotView<'a, State, Message, Theme, Renderer>,
    {
        type State = State;
        type Message = Message;
        type Theme = Theme;
        type Renderer = Renderer;
        type Executor = iced_futures::backend::default::Executor;

        fn name() -> &'static str {
            let name = std::any::type_name::<State>();

            name.split("::").next().unwrap_or("an_ice_hot_application")
        }

        fn boot(&self) -> (State, Task<MessageSource<Self::Message>>) {
            let (state, task) = self.boot.boot();
            (state, task.map(|message| MessageSource::Static(message)))
        }

        fn update(
            &self,
            state: &mut Self::State,
            message: MessageSource<Self::Message>,
            fn_state: &mut FunctionState,
        ) -> Task<MessageSource<Self::Message>> {
            self.update.update(state, message, fn_state)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            _window: window::Id,
            fn_state: &mut FunctionState,
        ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>
        where
            Theme: 'a,
            Renderer: 'a,
        {
            self.view.view(state, fn_state)
        }

        fn settings(&self) -> Settings {
            Settings::default()
        }

        fn window(&self) -> Option<window::Settings> {
            Some(window::Settings::default())
        }

        fn library_name(&self) -> Option<&str> {
            Some(self.view.lib_name)
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
    P: HotProgram,
{
    program: P,
    settings: Settings,
    window: window::Settings,
}

impl<P> HotIce<P>
where
    P: HotProgram + 'static,
    P::Message: Clone,
{
    pub fn run(self) -> Result<(), Error> {
        let program = Reload::new(self.program);

        #[cfg(all(feature = "debug", not(target_arch = "wasm32")))]
        let program = {
            iced_debug::init(iced_debug::Metadata {
                name: P::name(),
                theme: None,
                can_time_travel: false,
            });

            iced_devtools::attach(program)
        };

        iced_winit::run(program)
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
    ) -> HotIce<impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme>> {
        HotIce {
            program: hot_program::with_title(self.program, move |state, _window| {
                title.title(state)
            }),
            settings: self.settings,
            window: self.window,
        }
    }

    /// Sets the subscription logic of the [`Application`].
    pub fn subscription(
        self,
        f: impl IntoHotSubscription<P::State, P::Message>,
    ) -> HotIce<impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme>> {
        HotIce {
            program: hot_program::with_subscription(self.program, f),
            settings: self.settings,
            window: self.window,
        }
    }

    /// Sets the theme logic of the [`Application`].
    pub fn theme(
        self,
        f: impl ThemeFn<P::State, P::Theme>,
    ) -> HotIce<impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme>> {
        HotIce {
            program: hot_program::with_theme(self.program, move |state, _window| f.theme(state)),
            settings: self.settings,
            window: self.window,
        }
    }

    /// Sets the style logic of the [`Application`].
    pub fn style(
        self,
        f: impl Fn(&P::State, &P::Theme) -> theme::Style,
    ) -> HotIce<impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme>> {
        HotIce {
            program: hot_program::with_style(self.program, f),
            settings: self.settings,
            window: self.window,
        }
    }

    /// Sets the scale factor of the [`Application`].
    pub fn scale_factor(
        self,
        f: impl Fn(&P::State) -> f32,
    ) -> HotIce<impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme>> {
        HotIce {
            program: hot_program::with_scale_factor(self.program, move |state, _window| f(state)),
            settings: self.settings,
            window: self.window,
        }
    }

    /// Sets the executor of the [`Application`].
    pub fn executor<E>(
        self,
    ) -> HotIce<impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme>>
    where
        E: Executor,
    {
        HotIce {
            program: hot_program::with_executor::<P, E>(self.program),
            settings: self.settings,
            window: self.window,
        }
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
    lib_reloaders.entry(f.library_name()).or_insert_with(|| {
        let (_, update_ch_rx) = UPDATE_CHANNEL
            .get_or_init(|| crossfire::mpmc::bounded_tx_async_rx_blocking(1))
            .clone();
        let (subscription_ch_tx, _) = SUBSCRIPTION_CHANNEL
            .get_or_init(|| crossfire::mpmc::bounded_tx_blocking_rx_async(1))
            .clone();

        let mut lib_reloader = LibReloader::new(
            dylib_path,
            f.library_name(),
            Some(Duration::from_millis(25)),
            None,
        )
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
    });
}
