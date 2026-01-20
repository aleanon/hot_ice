//! Hot-reloadable application builder for Iced.
//!
//! This module provides the [`application`] function and [`HotIce`] builder
//! for creating Iced applications with hot reloading support.

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

/// Creates a new hot-reloadable Iced application.
///
/// This is the main entry point for creating a hot-reloadable application.
/// It returns a [`HotIce`] builder that can be configured with additional
/// settings before running.
///
/// # Arguments
///
/// * `boot` - The initialization function that creates the initial state and startup task
/// * `update` - The message handler function that updates state based on messages
/// * `view` - The view function that renders the UI based on current state
///
/// # Type Parameters
///
/// * `State` - Your application's state type
/// * `Message` - Your application's message enum (must implement [`DynMessage`] + [`Clone`])
/// * `Theme` - The theme type (typically [`iced::Theme`])
/// * `Renderer` - The renderer type (automatically inferred)
///
/// # Example
///
/// ## Basic Usage
///
/// ```rust,ignore
/// use hot_ice::iced::{Element, Task};
///
/// fn main() {
///     hot_ice::application(State::boot, State::update, State::view)
///         .run()
///         .unwrap();
/// }
///
/// struct State {
///     value: i32,
/// }
///
/// #[derive(Debug, Clone)]
/// enum Message {
///     Increment,
/// }
///
/// impl State {
///     fn boot() -> (Self, Task<Message>) {
///         (State { value: 0 }, Task::none())
///     }
///
///     fn update(&mut self, message: Message) -> Task<Message> {
///         match message {
///             Message::Increment => self.value += 1,
///         }
///         Task::none()
///     }
///
///     fn view(&self) -> Element<'_, Message> {
///         // Your UI here
///         todo!()
///     }
/// }
/// ```
///
/// ## With All Options
///
/// ```rust,ignore
/// use hot_ice::iced::{Element, Subscription, Task, Theme, theme, window};
///
/// fn main() {
///     hot_ice::application(State::boot, State::update, State::view)
///         .subscription(State::subscription)
///         .theme(State::theme)
///         .style(State::style)
///         .scale_factor(State::scale_factor)
///         .title(State::title)
///         .window_size((800, 600))
///         .centered()
///         .antialiasing(true)
///         .run()
///         .unwrap();
/// }
/// ```
///
/// ## With Hot Reloading Macros
///
/// For full hot reloading support, use the [`hot_fn`](crate::hot_fn) macro:
///
/// ```rust,ignore
/// use hot_ice::iced::{Element, Task};
///
/// struct State { value: i32 }
///
/// #[derive(Debug, Clone)]
/// enum Message { Increment }
///
/// impl State {
///     #[hot_ice::hot_fn]
///     fn boot() -> (Self, Task<Message>) {
///         (State { value: 0 }, Task::none())
///     }
///
///     #[hot_ice::hot_fn]
///     fn update(&mut self, message: Message) -> Task<Message> {
///         Task::none()
///     }
///
///     #[hot_ice::hot_fn]
///     fn view(&self) -> Element<'_, Message> {
///         todo!()
///     }
/// }
/// ```
///
/// ## With Hot State Persistence
///
/// For state that persists across reloads, use [`hot_state`](crate::hot_state):
///
/// ```rust,ignore
/// #[hot_ice::hot_state]
/// #[derive(Debug, Clone)]
/// struct State { value: i32 }
///
/// impl State {
///     #[hot_ice::hot_fn(hot_state)]
///     fn boot() -> (Self, Task<Message>) {
///         (State { value: 0 }, Task::none())
///     }
///
///     #[hot_ice::hot_fn(hot_state)]
///     fn update(&mut self, message: Message) -> Task<Message> {
///         Task::none()
///     }
///
///     #[hot_ice::hot_fn(hot_state)]
///     fn view(&self) -> Element<'_, Message> {
///         todo!()
///     }
/// }
/// ```
///
/// # Project Structure
///
/// Hot Ice expects a workspace with separate crates for the main binary
/// and the hot-reloadable UI:
///
/// ```text
/// my_app/
/// ├── Cargo.toml          # Workspace manifest
/// ├── my_app/             # Main binary crate
/// │   ├── Cargo.toml
/// │   └── src/
/// │       └── main.rs     # Calls hot_ice::application()
/// └── ui/                 # Hot-reloadable library crate
///     ├── Cargo.toml      # [lib] crate-type = ["rlib", "cdylib"]
///     └── src/
///         └── lib.rs      # State, Message, and #[hot_fn] functions
/// ```
///
///
/// # How It Works
///
/// 1. On startup, Hot Ice compiles your UI crate as a dynamic library
/// 2. It watches for file changes in your UI crate
/// 3. When changes are detected, it recompiles and hot-reloads the library
/// 4. Your application continues running with the new code
///
/// The reloader displays a status bar showing which functions are:
/// - **Static** (white): Not hot-reloadable
/// - **Hot** (green): Successfully loaded from dynamic library
/// - **Fallback** (orange): Failed to load, using static version
/// - **Error** (red): Function returned an error
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

/// A hot-reloadable Iced application builder.
///
/// This struct is returned by [`application`] and provides a builder pattern
/// for configuring your application before running it.
///
/// # Example
///
/// ```rust,ignore
/// hot_ice::application(State::boot, State::update, State::view)
///     // Optional callbacks
///     .subscription(State::subscription)
///     .theme(State::theme)
///     .style(State::style)
///     .scale_factor(State::scale_factor)
///     .title(State::title)
///     // Window settings
///     .window_size((1024, 768))
///     .centered()
///     .resizable(true)
///     .decorations(true)
///     .transparent(false)
///     // Rendering settings
///     .antialiasing(true)
///     .default_font(Font::MONOSPACE)
///     .font(include_bytes!("../fonts/custom.ttf").as_slice())
///     // Hot reloading settings
///     .reloader_settings(ReloaderSettings {
///         compile_in_reloader: true,
///         ..Default::default()
///     })
///     // Run the application
///     .run()
///     .unwrap();
/// ```
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
    /// Runs the application.
    ///
    /// This starts the hot reloader, compiles the UI library, and opens
    /// the application window. The function blocks until the window is closed.
    ///
    /// # Errors
    ///
    /// Returns an error if the application fails to start or encounters
    /// a fatal error during execution.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn main() -> Result<(), iced::Error> {
    ///     hot_ice::application(State::boot, State::update, State::view)
    ///         .run()
    /// }
    /// ```
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

    /// Sets the hot reloader configuration.
    ///
    /// Use this to customize how the hot reloader compiles and watches
    /// for changes in your UI crate.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use hot_ice::ReloaderSettings;
    /// use std::time::Duration;
    ///
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .reloader_settings(ReloaderSettings {
    ///         // Use a custom target directory
    ///         target_dir: "target/hot".to_string(),
    ///         lib_dir: "target/hot/debug".to_string(),
    ///         // Disable automatic compilation (manual cargo watch)
    ///         compile_in_reloader: false,
    ///         // Faster file change detection
    ///         file_watch_debounce: Duration::from_millis(10),
    ///         // Watch a specific directory
    ///         watch_dir: Some("ui/src".into()),
    ///     })
    ///     .run()
    ///     .unwrap();
    /// ```
    pub fn reloader_settings(self, reloader_settings: ReloaderSettings) -> Self {
        Self {
            reloader_settings,
            ..self
        }
    }

    /// Sets the [`Settings`] that will be used to run the application.
    ///
    /// This overwrites all previous settings. For individual settings,
    /// use the specific methods like [`antialiasing`](Self::antialiasing)
    /// or [`default_font`](Self::default_font).
    pub fn settings(self, settings: Settings) -> Self {
        Self { settings, ..self }
    }

    /// Enables or disables antialiasing.
    ///
    /// Antialiasing smooths the edges of shapes and text. Enabled by default
    /// on most platforms.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .antialiasing(true)
    ///     .run()
    ///     .unwrap();
    /// ```
    pub fn antialiasing(self, antialiasing: bool) -> Self {
        Self {
            settings: Settings {
                antialiasing,
                ..self.settings
            },
            ..self
        }
    }

    /// Sets the default font for text rendering.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use hot_ice::iced::Font;
    ///
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .default_font(Font::MONOSPACE)
    ///     .run()
    ///     .unwrap();
    /// ```
    pub fn default_font(self, default_font: Font) -> Self {
        Self {
            settings: Settings {
                default_font,
                ..self.settings
            },
            ..self
        }
    }

    /// Adds a font to be loaded at application startup.
    ///
    /// Fonts can be loaded from static byte slices (embedded in the binary)
    /// or from vectors of bytes.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .font(include_bytes!("../fonts/FiraSans-Regular.ttf").as_slice())
    ///     .font(include_bytes!("../fonts/FiraMono-Regular.ttf").as_slice())
    ///     .run()
    ///     .unwrap();
    /// ```
    pub fn font(mut self, font: impl Into<Cow<'static, [u8]>>) -> Self {
        self.settings.fonts.push(font.into());
        self
    }

    /// Sets the window settings.
    ///
    /// This overwrites any previous window settings. For individual settings,
    /// use the specific methods like [`window_size`](Self::window_size)
    /// or [`centered`](Self::centered).
    pub fn window(self, window: window::Settings) -> Self {
        Self { window, ..self }
    }

    /// Centers the window on the screen.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .window_size((800, 600))
    ///     .centered()
    ///     .run()
    ///     .unwrap();
    /// ```
    pub fn centered(self) -> Self {
        Self {
            window: window::Settings {
                position: window::Position::Centered,
                ..self.window
            },
            ..self
        }
    }

    /// Sets whether the application should exit when the close button is clicked.
    ///
    /// Set to `false` if you want to handle the close request manually
    /// (e.g., to show a confirmation dialog).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .exit_on_close_request(false) // Handle close manually
    ///     .run()
    ///     .unwrap();
    /// ```
    pub fn exit_on_close_request(self, exit_on_close_request: bool) -> Self {
        Self {
            window: window::Settings {
                exit_on_close_request,
                ..self.window
            },
            ..self
        }
    }

    /// Sets the initial window size.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .window_size((1024, 768))
    ///     .run()
    ///     .unwrap();
    /// ```
    pub fn window_size(self, size: impl Into<Size>) -> Self {
        Self {
            window: window::Settings {
                size: size.into(),
                ..self.window
            },
            ..self
        }
    }

    /// Sets whether the window background is transparent.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .transparent(true)
    ///     .decorations(false) // Usually combined with no decorations
    ///     .run()
    ///     .unwrap();
    /// ```
    pub fn transparent(self, transparent: bool) -> Self {
        Self {
            window: window::Settings {
                transparent,
                ..self.window
            },
            ..self
        }
    }

    /// Sets whether the window can be resized by the user.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .window_size((400, 300))
    ///     .resizable(false) // Fixed size window
    ///     .run()
    ///     .unwrap();
    /// ```
    pub fn resizable(self, resizable: bool) -> Self {
        Self {
            window: window::Settings {
                resizable,
                ..self.window
            },
            ..self
        }
    }

    /// Sets whether the window has decorations (title bar, borders).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .decorations(false) // Borderless window
    ///     .run()
    ///     .unwrap();
    /// ```
    pub fn decorations(self, decorations: bool) -> Self {
        Self {
            window: window::Settings {
                decorations,
                ..self.window
            },
            ..self
        }
    }

    /// Sets the initial window position.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use hot_ice::iced::window;
    ///
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .position(window::Position::Specific(100.0, 100.0))
    ///     .run()
    ///     .unwrap();
    /// ```
    pub fn position(self, position: window::Position) -> Self {
        Self {
            window: window::Settings {
                position,
                ..self.window
            },
            ..self
        }
    }

    /// Sets the window level (normal, always on top, always on bottom).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use hot_ice::iced::window;
    ///
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .level(window::Level::AlwaysOnTop)
    ///     .run()
    ///     .unwrap();
    /// ```
    pub fn level(self, level: window::Level) -> Self {
        Self {
            window: window::Settings {
                level,
                ..self.window
            },
            ..self
        }
    }

    /// Sets the window title function.
    ///
    /// The title function is called to get the window title, allowing
    /// dynamic titles based on application state.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// impl State {
    ///     fn title(&self) -> String {
    ///         format!("My App - {} items", self.items.len())
    ///     }
    /// }
    ///
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .title(State::title)
    ///     .run()
    ///     .unwrap();
    /// ```
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

    /// Sets the subscription function.
    ///
    /// Subscriptions allow your application to listen for external events
    /// like time, keyboard input, or custom async streams.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use hot_ice::iced::Subscription;
    /// use hot_ice::iced::time;
    /// use std::time::Duration;
    ///
    /// impl State {
    ///     fn subscription(&self) -> Subscription<Message> {
    ///         time::every(Duration::from_secs(1))
    ///             .map(|_| Message::Tick)
    ///     }
    /// }
    ///
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .subscription(State::subscription)
    ///     .run()
    ///     .unwrap();
    /// ```
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

    /// Sets the theme function.
    ///
    /// The theme function returns the current theme for the application.
    /// Return `None` to use the default theme.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use hot_ice::iced::Theme;
    ///
    /// impl State {
    ///     fn theme(&self) -> Option<Theme> {
    ///         Some(if self.dark_mode {
    ///             Theme::Dark
    ///         } else {
    ///             Theme::Light
    ///         })
    ///     }
    /// }
    ///
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .theme(State::theme)
    ///     .run()
    ///     .unwrap();
    /// ```
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

    /// Sets the style function.
    ///
    /// The style function customizes the application's background and text colors.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use hot_ice::iced::{Theme, theme};
    ///
    /// impl State {
    ///     fn style(&self, theme: &Theme) -> theme::Style {
    ///         theme::default(theme)
    ///     }
    /// }
    ///
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .style(State::style)
    ///     .run()
    ///     .unwrap();
    /// ```
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

    /// Sets the scale factor function.
    ///
    /// The scale factor controls the size of UI elements. A value of `2.0`
    /// makes everything twice as large (useful for high-DPI displays).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// impl State {
    ///     fn scale_factor(&self) -> f32 {
    ///         self.ui_scale // e.g., 1.0, 1.25, 1.5, 2.0
    ///     }
    /// }
    ///
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .scale_factor(State::scale_factor)
    ///     .run()
    ///     .unwrap();
    /// ```
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

    /// Sets a custom executor for async tasks.
    ///
    /// By default, Hot Ice uses the platform's default executor. Use this
    /// to specify a custom executor like Tokio or smol.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use hot_ice::iced::executor;
    ///
    /// hot_ice::application(State::boot, State::update, State::view)
    ///     .executor::<executor::Default>()
    ///     .run()
    ///     .unwrap();
    /// ```
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
