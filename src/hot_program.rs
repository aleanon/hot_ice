use std::sync::Arc;
use std::sync::Mutex;

use iced_core::Element;
use iced_core::Font;
use iced_core::Settings;
use iced_core::renderer;
use iced_core::text;
use iced_core::theme;
use iced_core::window;
use iced_futures::{Executor, Subscription};
use iced_winit::graphics::compositor;
use iced_winit::runtime::Task;

use crate::error::HotIceError;
use crate::functions::hot_scale_factor::HotScaleFactor;
use crate::functions::hot_scale_factor::IntoHotScaleFactor;
use crate::functions::hot_style::HotStyle;
use crate::functions::hot_style::IntoHotStyle;
use crate::functions::hot_subscription::HotSubscription;
use crate::functions::hot_subscription::IntoHotSubscription;
use crate::functions::hot_theme::HotTheme;
use crate::functions::hot_theme::IntoHotTheme;
use crate::functions::hot_title::HotTitle;
use crate::functions::hot_title::IntoHotTitle;
use crate::lib_reloader::LibReloader;
use crate::message::DynMessage;
use crate::message::MessageSource;
use crate::reloader::FunctionState;

/// An interactive, native, cross-platform, multi-windowed application.
///
/// A [`Program`] can execute asynchronous actions by returning a
/// [`Task`] in some of its methods.
#[allow(missing_docs)]
pub trait HotProgram {
    /// The state of the program.
    type State;

    /// The message of the program.
    type Message: DynMessage + Clone;

    /// The theme of the program.
    type Theme: theme::Base;

    /// The renderer of the program.
    type Renderer: Renderer;

    /// The executor of the program.
    type Executor: Executor;

    /// Returns the unique name of the [`Program`].
    fn name() -> &'static str;

    fn boot(&self) -> (Self::State, Task<MessageSource<Self::Message>>);

    fn update(
        &self,
        state: &mut Self::State,
        message: MessageSource<Self::Message>,
        reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Result<(Task<MessageSource<Self::Message>>, FunctionState), HotIceError>;

    fn view<'a>(
        &self,
        state: &'a Self::State,
        window: window::Id,
        reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Result<
        (
            Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>,
            FunctionState,
        ),
        HotIceError,
    >
    where
        Self::Theme: 'a,
        Self::Renderer: 'a;

    fn title(
        &self,
        _state: &Self::State,
        _window: window::Id,
        _reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Result<(String, FunctionState), HotIceError> {
        let mut title = String::new();

        for (i, part) in Self::name().split("_").enumerate() {
            use std::borrow::Cow;

            let part = match part {
                "a" | "an" | "of" | "in" | "and" => Cow::Borrowed(part),
                _ => {
                    let mut part = part.to_owned();

                    if let Some(first_letter) = part.get_mut(0..1) {
                        first_letter.make_ascii_uppercase();
                    }

                    Cow::Owned(part)
                }
            };

            if i > 0 {
                title.push(' ');
            }

            title.push_str(&part);
        }

        Ok((format!("{title} - Iced"), FunctionState::Static))
    }

    fn subscription(
        &self,
        _state: &Self::State,
        _reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Result<(Subscription<MessageSource<Self::Message>>, FunctionState), HotIceError> {
        Ok((Subscription::none(), FunctionState::Static))
    }

    fn theme(
        &self,
        _state: &Self::State,
        _window: window::Id,
        _reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Result<(Option<Self::Theme>, FunctionState), HotIceError> {
        Ok((None, FunctionState::Static))
    }

    fn settings(&self) -> Settings;

    fn window(&self) -> Option<window::Settings>;

    fn style(
        &self,
        _state: &Self::State,
        theme: &Self::Theme,
        _reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Result<(theme::Style, FunctionState), HotIceError> {
        Ok((theme::Base::base(theme), FunctionState::Static))
    }

    fn scale_factor(
        &self,
        _state: &Self::State,
        _window: window::Id,
        _reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Result<(f32, FunctionState), HotIceError> {
        Ok((1.0, FunctionState::Static))
    }
}

/// Decorates a [`Program`] with the given title function.
pub fn with_title<P: HotProgram>(
    program: P,
    f: impl IntoHotTitle<P::State>,
) -> impl HotProgram<
    State = P::State,
    Message = P::Message,
    Theme = P::Theme,
    Renderer = P::Renderer,
    Executor = P::Executor,
> {
    let hot_title = HotTitle::new(f);

    struct WithTitle<P, F>
    where
        P: HotProgram,
    {
        program: P,
        title: HotTitle<F, P::State>,
    }

    impl<P, F> HotProgram for WithTitle<P, F>
    where
        P: HotProgram,
        F: IntoHotTitle<P::State>,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = P::Executor;

        fn title(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(String, FunctionState), HotIceError> {
            self.title.title(state, window, reloader)
        }

        fn name() -> &'static str {
            P::name()
        }

        fn boot(&self) -> (Self::State, Task<MessageSource<Self::Message>>) {
            self.program.boot()
        }

        fn update(
            &self,
            state: &mut Self::State,
            message: MessageSource<Self::Message>,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Task<MessageSource<Self::Message>>, FunctionState), HotIceError> {
            self.program.update(state, message, reloader)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<
            (
                Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>,
                FunctionState,
            ),
            HotIceError,
        >
        where
            Self::Theme: 'a,
            Self::Renderer: 'a,
        {
            self.program.view(state, window, reloader)
        }

        fn theme(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Option<Self::Theme>, FunctionState), HotIceError> {
            self.program.theme(state, window, reloader)
        }

        fn settings(&self) -> Settings {
            self.program.settings()
        }

        fn window(&self) -> Option<window::Settings> {
            self.program.window()
        }

        fn subscription(
            &self,
            state: &Self::State,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Subscription<MessageSource<Self::Message>>, FunctionState), HotIceError>
        {
            self.program.subscription(state, reloader)
        }

        fn style(
            &self,
            state: &Self::State,
            theme: &Self::Theme,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(theme::Style, FunctionState), HotIceError> {
            self.program.style(state, theme, reloader)
        }

        fn scale_factor(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(f32, FunctionState), HotIceError> {
            self.program.scale_factor(state, window, reloader)
        }
    }

    WithTitle {
        program,
        title: hot_title,
    }
}

/// Decorates a [`Program`] with the given subscription function.
pub fn with_subscription<P: HotProgram>(
    program: P,
    f: impl IntoHotSubscription<P::State, P::Message>,
) -> impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme> {
    let hot_sub = HotSubscription::new(f);

    struct WithSubscription<P, F>
    where
        P: HotProgram,
    {
        program: P,
        subscription: HotSubscription<F, P::State, P::Message>,
    }

    impl<P: HotProgram, F> HotProgram for WithSubscription<P, F>
    where
        F: IntoHotSubscription<P::State, P::Message>,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = P::Executor;

        fn subscription(
            &self,
            state: &Self::State,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Subscription<MessageSource<Self::Message>>, FunctionState), HotIceError>
        {
            self.subscription.subscription(state, reloader)
        }

        fn name() -> &'static str {
            P::name()
        }

        fn boot(&self) -> (Self::State, Task<MessageSource<Self::Message>>) {
            self.program.boot()
        }

        fn update(
            &self,
            state: &mut Self::State,
            message: MessageSource<Self::Message>,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Task<MessageSource<Self::Message>>, FunctionState), HotIceError> {
            self.program.update(state, message, reloader)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<
            (
                Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>,
                FunctionState,
            ),
            HotIceError,
        >
        where
            Self::Theme: 'a,
            Self::Renderer: 'a,
        {
            self.program.view(state, window, reloader)
        }

        fn settings(&self) -> Settings {
            self.program.settings()
        }

        fn window(&self) -> Option<window::Settings> {
            self.program.window()
        }

        fn title(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(String, FunctionState), HotIceError> {
            self.program.title(state, window, reloader)
        }

        fn theme(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Option<Self::Theme>, FunctionState), HotIceError> {
            self.program.theme(state, window, reloader)
        }

        fn style(
            &self,
            state: &Self::State,
            theme: &Self::Theme,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(theme::Style, FunctionState), HotIceError> {
            self.program.style(state, theme, reloader)
        }

        fn scale_factor(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(f32, FunctionState), HotIceError> {
            self.program.scale_factor(state, window, reloader)
        }
    }

    WithSubscription {
        program,
        subscription: hot_sub,
    }
}

/// Decorates a [`Program`] with the given theme function.
pub fn with_theme<P: HotProgram>(
    program: P,
    f: impl IntoHotTheme<P::State, P::Theme>,
) -> impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme> {
    let hot_theme = HotTheme::new(f);

    struct WithTheme<P, F>
    where
        P: HotProgram,
    {
        program: P,
        theme: HotTheme<F, P::State, P::Theme>,
    }

    impl<P: HotProgram, F> HotProgram for WithTheme<P, F>
    where
        F: IntoHotTheme<P::State, P::Theme>,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = P::Executor;

        fn theme(
            &self,
            state: &Self::State,
            _window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Option<Self::Theme>, FunctionState), HotIceError> {
            self.theme.theme(state, reloader)
        }

        fn name() -> &'static str {
            P::name()
        }

        fn boot(&self) -> (Self::State, Task<MessageSource<Self::Message>>) {
            self.program.boot()
        }

        fn title(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(String, FunctionState), HotIceError> {
            self.program.title(state, window, reloader)
        }

        fn update(
            &self,
            state: &mut Self::State,
            message: MessageSource<Self::Message>,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Task<MessageSource<Self::Message>>, FunctionState), HotIceError> {
            self.program.update(state, message, reloader)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<
            (
                Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>,
                FunctionState,
            ),
            HotIceError,
        >
        where
            Self::Theme: 'a,
            Self::Renderer: 'a,
        {
            self.program.view(state, window, reloader)
        }

        fn settings(&self) -> Settings {
            self.program.settings()
        }

        fn window(&self) -> Option<window::Settings> {
            self.program.window()
        }

        fn subscription(
            &self,
            state: &Self::State,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Subscription<MessageSource<Self::Message>>, FunctionState), HotIceError>
        {
            self.program.subscription(state, reloader)
        }

        fn style(
            &self,
            state: &Self::State,
            theme: &Self::Theme,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(theme::Style, FunctionState), HotIceError> {
            self.program.style(state, theme, reloader)
        }

        fn scale_factor(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(f32, FunctionState), HotIceError> {
            self.program.scale_factor(state, window, reloader)
        }
    }

    WithTheme {
        program,
        theme: hot_theme,
    }
}

/// Decorates a [`Program`] with the given style function.
pub fn with_style<P: HotProgram>(
    program: P,
    f: impl IntoHotStyle<P::State, P::Theme>,
) -> impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme> {
    let hot_style = HotStyle::new(f);

    struct WithStyle<P, F>
    where
        P: HotProgram,
    {
        program: P,
        style: HotStyle<F, P::State, P::Theme>,
    }

    impl<P: HotProgram, F> HotProgram for WithStyle<P, F>
    where
        F: IntoHotStyle<P::State, P::Theme>,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = P::Executor;

        fn style(
            &self,
            state: &Self::State,
            theme: &Self::Theme,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(theme::Style, FunctionState), HotIceError> {
            self.style.style(state, theme, reloader)
        }

        fn name() -> &'static str {
            P::name()
        }

        fn boot(&self) -> (Self::State, Task<MessageSource<Self::Message>>) {
            self.program.boot()
        }

        fn title(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(String, FunctionState), HotIceError> {
            self.program.title(state, window, reloader)
        }

        fn update(
            &self,
            state: &mut Self::State,
            message: MessageSource<Self::Message>,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Task<MessageSource<Self::Message>>, FunctionState), HotIceError> {
            self.program.update(state, message, reloader)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<
            (
                Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>,
                FunctionState,
            ),
            HotIceError,
        >
        where
            Self::Theme: 'a,
            Self::Renderer: 'a,
        {
            self.program.view(state, window, reloader)
        }

        fn settings(&self) -> Settings {
            self.program.settings()
        }

        fn window(&self) -> Option<window::Settings> {
            self.program.window()
        }

        fn subscription(
            &self,
            state: &Self::State,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Subscription<MessageSource<Self::Message>>, FunctionState), HotIceError>
        {
            self.program.subscription(state, reloader)
        }

        fn theme(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Option<Self::Theme>, FunctionState), HotIceError> {
            self.program.theme(state, window, reloader)
        }

        fn scale_factor(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(f32, FunctionState), HotIceError> {
            self.program.scale_factor(state, window, reloader)
        }
    }

    WithStyle {
        program,
        style: hot_style,
    }
}

/// Decorates a [`Program`] with the given scale factor function.
pub fn with_scale_factor<P: HotProgram>(
    program: P,
    f: impl IntoHotScaleFactor<P::State>,
) -> impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme> {
    let hot_scale_factor = HotScaleFactor::new(f);

    struct WithScaleFactor<P, F>
    where
        P: HotProgram,
    {
        program: P,
        scale_factor: HotScaleFactor<F, P::State>,
    }

    impl<P: HotProgram, F> HotProgram for WithScaleFactor<P, F>
    where
        F: IntoHotScaleFactor<P::State>,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = P::Executor;

        fn title(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(String, FunctionState), HotIceError> {
            self.program.title(state, window, reloader)
        }

        fn name() -> &'static str {
            P::name()
        }

        fn boot(&self) -> (Self::State, Task<MessageSource<Self::Message>>) {
            self.program.boot()
        }

        fn update(
            &self,
            state: &mut Self::State,
            message: MessageSource<Self::Message>,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Task<MessageSource<Self::Message>>, FunctionState), HotIceError> {
            self.program.update(state, message, reloader)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<
            (
                Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>,
                FunctionState,
            ),
            HotIceError,
        >
        where
            Self::Theme: 'a,
            Self::Renderer: 'a,
        {
            self.program.view(state, window, reloader)
        }

        fn settings(&self) -> Settings {
            self.program.settings()
        }

        fn window(&self) -> Option<window::Settings> {
            self.program.window()
        }

        fn subscription(
            &self,
            state: &Self::State,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Subscription<MessageSource<Self::Message>>, FunctionState), HotIceError>
        {
            self.program.subscription(state, reloader)
        }

        fn theme(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Option<Self::Theme>, FunctionState), HotIceError> {
            self.program.theme(state, window, reloader)
        }

        fn style(
            &self,
            state: &Self::State,
            theme: &Self::Theme,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(theme::Style, FunctionState), HotIceError> {
            self.program.style(state, theme, reloader)
        }

        fn scale_factor(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(f32, FunctionState), HotIceError> {
            self.scale_factor.scale_factor(state, window, reloader)
        }
    }

    WithScaleFactor {
        program,
        scale_factor: hot_scale_factor,
    }
}

/// Decorates a [`Program`] with the given executor function.
pub fn with_executor<P: HotProgram, E: Executor>(
    program: P,
) -> impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme> {
    use std::marker::PhantomData;

    struct WithExecutor<P, E> {
        program: P,
        executor: PhantomData<E>,
    }

    impl<P: HotProgram, E> HotProgram for WithExecutor<P, E>
    where
        E: Executor,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = E;

        fn title(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(String, FunctionState), HotIceError> {
            self.program.title(state, window, reloader)
        }

        fn name() -> &'static str {
            P::name()
        }

        fn boot(&self) -> (Self::State, Task<MessageSource<Self::Message>>) {
            self.program.boot()
        }

        fn update(
            &self,
            state: &mut Self::State,
            message: MessageSource<Self::Message>,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Task<MessageSource<Self::Message>>, FunctionState), HotIceError> {
            self.program.update(state, message, reloader)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<
            (
                Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>,
                FunctionState,
            ),
            HotIceError,
        >
        where
            Self::Theme: 'a,
            Self::Renderer: 'a,
        {
            self.program.view(state, window, reloader)
        }

        fn settings(&self) -> Settings {
            self.program.settings()
        }

        fn window(&self) -> Option<window::Settings> {
            self.program.window()
        }

        fn subscription(
            &self,
            state: &Self::State,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Subscription<MessageSource<Self::Message>>, FunctionState), HotIceError>
        {
            self.program.subscription(state, reloader)
        }

        fn theme(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(Option<Self::Theme>, FunctionState), HotIceError> {
            self.program.theme(state, window, reloader)
        }

        fn style(
            &self,
            state: &Self::State,
            theme: &Self::Theme,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(theme::Style, FunctionState), HotIceError> {
            self.program.style(state, theme, reloader)
        }

        fn scale_factor(
            &self,
            state: &Self::State,
            window: window::Id,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Result<(f32, FunctionState), HotIceError> {
            self.program.scale_factor(state, window, reloader)
        }
    }

    WithExecutor {
        program,
        executor: PhantomData::<E>,
    }
}

///The renderer of some [`Program`].
pub trait Renderer: text::Renderer<Font = Font> + compositor::Default + renderer::Headless {}

impl<T> Renderer for T where
    T: text::Renderer<Font = Font> + compositor::Default + renderer::Headless
{
}
