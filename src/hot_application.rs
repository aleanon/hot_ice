use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
};

use iced_core::{Element, Font, Settings, Size, theme, window};
use iced_futures::Executor;
use iced_winit::{Error, runtime::Task};

use crate::{
    boot,
    hot_program::{self, HotProgram},
    hot_scale_factor::IntoHotScaleFactor,
    hot_style::IntoHotStyle,
    hot_subscription::IntoHotSubscription,
    hot_theme::IntoHotTheme,
    hot_title::IntoHotTitle,
    hot_update::{self, HotUpdate},
    hot_view::{self, HotView},
    lib_reloader::LibReloader,
    message::DynMessage,
    message::MessageSource,
    reloader::{FunctionState, Reload, ReloaderSettings},
};

pub fn application<State, Message, Theme, Renderer>(
    boot: impl boot::Boot<State, Message>,
    update: impl hot_update::IntoHotUpdate<State, Message>,
    view: impl for<'a> hot_view::IntoHotView<'a, State, Message, Theme, Renderer>,
) -> HotIce<impl HotProgram<State = State, Message = Message, Theme = Theme, Renderer = Renderer>>
where
    State: 'static,
    Message: DynMessage + Clone,
    Theme: theme::Base + iced_widget::container::Catalog + iced_widget::text::Catalog,
    Renderer: hot_program::Renderer,
{
    let hot_view = HotView::new(view);
    let hot_update = HotUpdate::new(update);

    assert_eq!(
        hot_view.lib_name, hot_update.lib_name,
        "Application must be defined in a single library crate"
    );

    let lib_name = hot_view.lib_name;

    // initiate_lib_reloaders(&hot_view, &hot_update, dylib_path);

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
        Theme: theme::Base + iced_widget::container::Catalog + iced_widget::text::Catalog,
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
            (state, task.map(MessageSource::Static))
        }

        fn update(
            &self,
            state: &mut Self::State,
            message: MessageSource<Self::Message>,
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Task<MessageSource<Self::Message>> {
            self.update.update(state, message, fn_state, reloader)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            _window: window::Id,
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>
        where
            Theme: 'a,
            Renderer: 'a,
        {
            self.view.view(state, fn_state, reloader)
        }

        fn settings(&self) -> Settings {
            Settings::default()
        }

        fn window(&self) -> Option<window::Settings> {
            Some(window::Settings::default())
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
        reloader_settings: ReloaderSettings::default(),
        lib_name,
    }
}

pub struct HotIce<P>
where
    P: HotProgram,
{
    program: P,
    settings: Settings,
    window: window::Settings,
    reloader_settings: ReloaderSettings,
    lib_name: &'static str,
}

impl<P> HotIce<P>
where
    P: HotProgram + 'static,
    P::Message: Clone,
{
    pub fn run(self) -> Result<(), Error> {
        let fonts = self.settings.fonts.clone();

        let program = Reload::new(
            self.program,
            self.reloader_settings,
            self.settings,
            self.window,
            self.lib_name,
            fonts,
        );

        #[cfg(all(feature = "debug", not(target_arch = "wasm32")))]
        let program = {
            iced_debug::init(iced_debug::Metadata {
                name: P::name(),
                theme: None,
                can_time_travel: cfg!(feature = "time-travel"),
            });

            iced_devtools::attach(program)
        };

        iced_winit::run(program)
    }

    pub fn reloader_settings(self, reloader_settings: ReloaderSettings) -> Self {
        Self {
            reloader_settings,
            ..self
        }
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
        f: impl IntoHotTitle<P::State>,
    ) -> HotIce<impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme>> {
        HotIce {
            program: hot_program::with_title(self.program, f),
            settings: self.settings,
            window: self.window,
            reloader_settings: self.reloader_settings,
            lib_name: self.lib_name,
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
            reloader_settings: self.reloader_settings,
            lib_name: self.lib_name,
        }
    }

    /// Sets the theme logic of the [`Application`].
    pub fn theme(
        self,
        f: impl IntoHotTheme<P::State, P::Theme>,
    ) -> HotIce<impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme>> {
        HotIce {
            program: hot_program::with_theme(self.program, f),
            settings: self.settings,
            window: self.window,
            reloader_settings: self.reloader_settings,
            lib_name: self.lib_name,
        }
    }

    /// Sets the style logic of the [`Application`].
    pub fn style(
        self,
        f: impl IntoHotStyle<P::State, P::Theme>,
    ) -> HotIce<impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme>> {
        HotIce {
            program: hot_program::with_style(self.program, f),
            settings: self.settings,
            window: self.window,
            reloader_settings: self.reloader_settings,
            lib_name: self.lib_name,
        }
    }

    /// Sets the scale factor of the [`Application`].
    pub fn scale_factor(
        self,
        f: impl IntoHotScaleFactor<P::State>,
    ) -> HotIce<impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme>> {
        HotIce {
            program: hot_program::with_scale_factor(self.program, f),
            settings: self.settings,
            window: self.window,
            reloader_settings: self.reloader_settings,
            lib_name: self.lib_name,
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
            reloader_settings: self.reloader_settings,
            lib_name: self.lib_name,
        }
    }
}
