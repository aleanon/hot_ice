use std::{any::type_name, borrow::Cow, marker::PhantomData, pin::Pin};

use iced::{advanced::{self, graphics::compositor, text, Renderer}, application::{self, Boot, Title, Update}, theme, window, Application, Element, Executor, Font, Program, Result, Settings, Size, Task};
use ui::Subscription;

use crate::{program, reloadable::HotView, reloader::{Message, ReadyToReload, Reload, ReloadEvent, Reloader, SUBSCRIPTION_CHANNEL, UPDATE_CHANNEL}, unsafe_reference::UnsafeRefMut};


// pub fn application<State, Message, Theme, Renderer> (
//     dylib_name: &'static str,
//     boot: impl Boot<State, Message>,
//     update: impl Update<State, Message>,
//     view: impl for<'a> self::View<'a, State, Message, Theme, Renderer>,
// ) -> HotIce<impl Program<State = State, Message = Message, Theme = Theme>>
// where
//     State: 'static,
//     Message: Send + std::fmt::Debug + 'static + Clone,
//     Theme: Default + theme::Base,
//     Renderer: advanced::text::Renderer + compositor::Default,
// {
//     use std::marker::PhantomData;

//     struct Instance<State, Message, Theme, Renderer, Boot, Update, View> {
//         boot: Boot,
//         update: Update,
//         view: View,
//         _state: PhantomData<State>,
//         _message: PhantomData<Message>,
//         _theme: PhantomData<Theme>,
//         _renderer: PhantomData<Renderer>,
//     }

        

//     impl<State, Message, Theme, Renderer, Boot, Update, View> Program
//         for Instance<State, Message, Theme, Renderer, Boot, Update, View>
//     where
//         Message: Send + std::fmt::Debug + 'static + Clone,
//         Theme: Default + theme::Base,
//         Renderer: iced::advanced::text::Renderer + compositor::Default,
//         Boot: self::Boot<State, Message>,
//         Update: self::Update<State, Message>,
//         View: for<'a> self::View<'a, State, Message, Theme, Renderer>,
//     {
//         type State = State;
//         type Message = Message;
//         type Theme = Theme;
//         type Renderer = Renderer;
//         type Executor = iced::executor::Default;

//         fn name() -> &'static str {
//             let name = std::any::type_name::<State>();

//             name.split("::").next().unwrap_or("a_cool_application")
//         }

//         fn boot(&self) -> (State, Task<Message>) {
//             self.boot.boot()
//         }

//         fn update(
//             &self,
//             state: &mut Self::State,
//             message: Self::Message,
//         ) -> Task<Self::Message> {
//             self.update.update(state, message).into()
//         }

//         fn view<'a>(
//             &self,
//             state: &'a Self::State,
//             _window: window::Id,
//         ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
//             self.view.view(state).into()
//         }
//     }

//     HotIce {
//         dylib_name,
//         program: Instance {
//             boot,
//             update,
//             view,
//             _state: PhantomData,
//             _message: PhantomData,
//             _theme: PhantomData,
//             _renderer: PhantomData,
//         },
//         settings: Settings::default(),
//         window: window::Settings::default()
//     }
// }

pub struct HotIce<P> where 
    P: Program {
    dylib_name: &'static str,
    program: P,
    settings: Settings,
    window: window::Settings,
}



impl<P> HotIce<P>
where
    P: Program + 'static,
    P::Message: Clone {

    pub fn with_program(dylib_name: &'static str, program: P) -> Self {
        Self {
            dylib_name,
            program,
            settings: Settings::default(),
            window: window::Settings::default(),
        }
    }

    pub fn run(self) -> Result {
        let reloader = Reload::new(self.program);
        Ok(iced::shell::run(reloader, self.settings, Some(self.window))?)
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
    ) -> HotIce<
        impl Program<State = P::State, Message = P::Message, Theme = P::Theme>,
    > {
        HotIce {
            dylib_name: self.dylib_name,
            program: program::with_title(self.program, move |state, _window| {
                title.title(state)
            }),
            settings: self.settings,
            window: self.window,
        }
    }

    /// Sets the subscription logic of the [`Application`].
    pub fn subscription(
        self,
        f: impl Fn(&P::State) -> Subscription<P::Message>,
    ) -> HotIce<
        impl Program<State = P::State, Message = P::Message, Theme = P::Theme>,
    > {
        HotIce {
            dylib_name: self.dylib_name,
            program: program::with_subscription(self.program, f),
            settings: self.settings,
            window: self.window,
        }
    }

    /// Sets the theme logic of the [`Application`].
    pub fn theme(
        self,
        f: impl Fn(&P::State) -> P::Theme,
    ) -> HotIce<
        impl Program<State = P::State, Message = P::Message, Theme = P::Theme>,
    > {
        HotIce {
            dylib_name: self.dylib_name,
            program: program::with_theme(self.program, move |state, _window| f(state)),
            settings: self.settings,
            window: self.window,
        }
    }

    /// Sets the style logic of the [`Application`].
    pub fn style(
        self,
        f: impl Fn(&P::State, &P::Theme) -> theme::Style,
    ) -> HotIce<
        impl Program<State = P::State, Message = P::Message, Theme = P::Theme>,
    > {
        HotIce {
            dylib_name: self.dylib_name,
            program: program::with_style(self.program, f),
            settings: self.settings,
            window: self.window,
        }
    }

    /// Sets the scale factor of the [`Application`].
    pub fn scale_factor(
        self,
        f: impl Fn(&P::State) -> f64,
    ) -> HotIce<
        impl Program<State = P::State, Message = P::Message, Theme = P::Theme>,
    > {
        HotIce {
            dylib_name: self.dylib_name,
            program: program::with_scale_factor(self.program, move |state, _window| {
                f(state)
            }),
            settings: self.settings,
            window: self.window,
        }
    }

    /// Sets the executor of the [`Application`].
    pub fn executor<E>(
        self,
    ) -> HotIce<
        impl Program<State = P::State, Message = P::Message, Theme = P::Theme>,
    >
    where
        E: Executor,
    {
        HotIce {
            dylib_name: self.dylib_name,
            program: program::with_executor::<P, E>(self.program),
            settings: self.settings,
            window: self.window,
        }
    }
}

pub fn hot_application<View, State, Message, Theme, Renderer> (
    dylib_name: &'static str,
    dylib_path: &'static str,
    boot: View,
    update: impl Update<State, Message>,
    view: impl for<'a> application::View<'a, State, Message, Theme, Renderer>,
) -> HotIce<impl Program<State = State, Message = Message, Theme = Theme>>
where
    View: Boot<State, Message>,
    State: 'static,
    Message: Send + std::fmt::Debug + 'static + Clone,
    Theme: Default + theme::Base,
    Renderer: advanced::text::Renderer + compositor::Default,
{
    use std::marker::PhantomData;


    struct Instance<State, Message, Theme, Renderer, Boot, Update, View> {
        boot: Boot,
        update: Update,
        view: View,
        _state: PhantomData<State>,
        _message: PhantomData<Message>,
        _theme: PhantomData<Theme>,
        _renderer: PhantomData<Renderer>,
    }

    let view_path = type_name::<View>();
    println!("{view_path}");

    impl<State, Message, Theme, Renderer, Boot, Update, View> Program
        for Instance<State, Message, Theme, Renderer, Boot, Update, View>
    where
        Message: Send + std::fmt::Debug + 'static + Clone,
        Theme: Default + theme::Base,
        Renderer: iced::advanced::text::Renderer + compositor::Default,
        Boot: self::Boot<State, Message>,
        Update: self::Update<State, Message>,
        View: for<'a> application::View<'a, State, Message, Theme, Renderer>,
    {
        type State = State;
        type Message = Message;
        type Theme = Theme;
        type Renderer = Renderer;
        type Executor = iced::executor::Default;

        fn name() -> &'static str {
            let name = std::any::type_name::<State>();

            name.split("::").next().unwrap_or("a_cool_application")
        }

        fn boot(&self) -> (State, Task<Message>) {
            self.boot.boot()
        }

        fn update(
            &self,
            state: &mut Self::State,
            message: Self::Message,
        ) -> Task<Self::Message> {
            self.update.update(state, message).into()
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
        dylib_name,
        program: Instance {
            boot,
            update,
            view,
            _state: PhantomData,
            _message: PhantomData,
            _theme: PhantomData,
            _renderer: PhantomData,
        },
        settings: Settings::default(),
        window: window::Settings::default()
    }
}