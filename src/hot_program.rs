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

use crate::DynMessage;
use crate::hot_subscription::HotSubscription;
use crate::hot_subscription::IntoHotSubscription;
use crate::lib_reloader::LibReloader;
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
        fn_state: &mut FunctionState,
        reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Task<MessageSource<Self::Message>>;

    fn view<'a>(
        &self,
        state: &'a Self::State,
        window: window::Id,
        fn_state: &mut FunctionState,
        reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>
    where
        Self::Theme: 'a,
        Self::Renderer: 'a;

    fn title(&self, _state: &Self::State, _window: window::Id) -> String {
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

        format!("{title} - Iced")
    }

    fn subscription(
        &self,
        _state: &Self::State,
        _fn_state: &mut FunctionState,
        _reloader: Option<&Arc<Mutex<LibReloader>>>,
    ) -> Subscription<MessageSource<Self::Message>> {
        Subscription::none()
    }

    fn theme(&self, _state: &Self::State, _window: window::Id) -> Option<Self::Theme> {
        None
    }

    fn settings(&self) -> Settings;

    fn window(&self) -> Option<window::Settings>;

    fn style(&self, _state: &Self::State, theme: &Self::Theme) -> theme::Style {
        theme::Base::base(theme)
    }

    fn scale_factor(&self, _state: &Self::State, _window: window::Id) -> f32 {
        1.0
    }
}

/// Decorates a [`Program`] with the given title function.
pub fn with_title<P: HotProgram>(
    program: P,
    title: impl Fn(&P::State, window::Id) -> String,
) -> impl HotProgram<
    State = P::State,
    Message = P::Message,
    Theme = P::Theme,
    Renderer = P::Renderer,
    Executor = P::Executor,
> {
    struct WithTitle<P, Title> {
        program: P,
        title: Title,
    }

    impl<P, Title> HotProgram for WithTitle<P, Title>
    where
        P: HotProgram,
        Title: Fn(&P::State, window::Id) -> String,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = P::Executor;

        fn title(&self, state: &Self::State, window: window::Id) -> String {
            (self.title)(state, window)
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
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Task<MessageSource<Self::Message>> {
            self.program.update(state, message, fn_state, reloader)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>
        where
            Self::Theme: 'a,
            Self::Renderer: 'a,
        {
            self.program.view(state, window, fn_state, reloader)
        }

        fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
            self.program.theme(state, window)
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
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Subscription<MessageSource<Self::Message>> {
            self.program.subscription(state, fn_state, reloader)
        }

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
            self.program.scale_factor(state, window)
        }
    }

    WithTitle { program, title }
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
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Subscription<MessageSource<Self::Message>> {
            self.subscription.subscription(state, fn_state, reloader)
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
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Task<MessageSource<Self::Message>> {
            self.program.update(state, message, fn_state, reloader)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>
        where
            Self::Theme: 'a,
            Self::Renderer: 'a,
        {
            self.program.view(state, window, fn_state, reloader)
        }

        fn settings(&self) -> Settings {
            self.program.settings()
        }

        fn window(&self) -> Option<window::Settings> {
            self.program.window()
        }

        fn title(&self, state: &Self::State, window: window::Id) -> String {
            self.program.title(state, window)
        }

        fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
            self.program.theme(state, window)
        }

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
            self.program.scale_factor(state, window)
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
    f: impl Fn(&P::State, window::Id) -> Option<P::Theme>,
) -> impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme> {
    struct WithTheme<P, F> {
        program: P,
        theme: F,
    }

    impl<P: HotProgram, F> HotProgram for WithTheme<P, F>
    where
        F: Fn(&P::State, window::Id) -> Option<P::Theme>,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = P::Executor;

        fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
            (self.theme)(state, window)
        }

        fn name() -> &'static str {
            P::name()
        }

        fn boot(&self) -> (Self::State, Task<MessageSource<Self::Message>>) {
            self.program.boot()
        }

        fn title(&self, state: &Self::State, window: window::Id) -> String {
            self.program.title(state, window)
        }

        fn update(
            &self,
            state: &mut Self::State,
            message: MessageSource<Self::Message>,
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Task<MessageSource<Self::Message>> {
            self.program.update(state, message, fn_state, reloader)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>
        where
            Self::Theme: 'a,
            Self::Renderer: 'a,
        {
            self.program.view(state, window, fn_state, reloader)
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
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Subscription<MessageSource<Self::Message>> {
            self.program.subscription(state, fn_state, reloader)
        }

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
            self.program.scale_factor(state, window)
        }
    }

    WithTheme { program, theme: f }
}

/// Decorates a [`Program`] with the given style function.
pub fn with_style<P: HotProgram>(
    program: P,
    f: impl Fn(&P::State, &P::Theme) -> theme::Style,
) -> impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme> {
    struct WithStyle<P, F> {
        program: P,
        style: F,
    }

    impl<P: HotProgram, F> HotProgram for WithStyle<P, F>
    where
        F: Fn(&P::State, &P::Theme) -> theme::Style,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = P::Executor;

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            (self.style)(state, theme)
        }

        fn name() -> &'static str {
            P::name()
        }

        fn boot(&self) -> (Self::State, Task<MessageSource<Self::Message>>) {
            self.program.boot()
        }

        fn title(&self, state: &Self::State, window: window::Id) -> String {
            self.program.title(state, window)
        }

        fn update(
            &self,
            state: &mut Self::State,
            message: MessageSource<Self::Message>,
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Task<MessageSource<Self::Message>> {
            self.program.update(state, message, fn_state, reloader)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>
        where
            Self::Theme: 'a,
            Self::Renderer: 'a,
        {
            self.program.view(state, window, fn_state, reloader)
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
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Subscription<MessageSource<Self::Message>> {
            self.program.subscription(state, fn_state, reloader)
        }

        fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
            self.program.theme(state, window)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
            self.program.scale_factor(state, window)
        }
    }

    WithStyle { program, style: f }
}

/// Decorates a [`Program`] with the given scale factor function.
pub fn with_scale_factor<P: HotProgram>(
    program: P,
    f: impl Fn(&P::State, window::Id) -> f32,
) -> impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme> {
    struct WithScaleFactor<P, F> {
        program: P,
        scale_factor: F,
    }

    impl<P: HotProgram, F> HotProgram for WithScaleFactor<P, F>
    where
        F: Fn(&P::State, window::Id) -> f32,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = P::Executor;

        fn title(&self, state: &Self::State, window: window::Id) -> String {
            self.program.title(state, window)
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
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Task<MessageSource<Self::Message>> {
            self.program.update(state, message, fn_state, reloader)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>
        where
            Self::Theme: 'a,
            Self::Renderer: 'a,
        {
            self.program.view(state, window, fn_state, reloader)
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
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Subscription<MessageSource<Self::Message>> {
            self.program.subscription(state, fn_state, reloader)
        }

        fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
            self.program.theme(state, window)
        }

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
            (self.scale_factor)(state, window)
        }
    }

    WithScaleFactor {
        program,
        scale_factor: f,
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

        fn title(&self, state: &Self::State, window: window::Id) -> String {
            self.program.title(state, window)
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
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Task<MessageSource<Self::Message>> {
            self.program.update(state, message, fn_state, reloader)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>
        where
            Self::Theme: 'a,
            Self::Renderer: 'a,
        {
            self.program.view(state, window, fn_state, reloader)
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
            fn_state: &mut FunctionState,
            reloader: Option<&Arc<Mutex<LibReloader>>>,
        ) -> Subscription<MessageSource<Self::Message>> {
            self.program.subscription(state, fn_state, reloader)
        }

        fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
            self.program.theme(state, window)
        }

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
            self.program.scale_factor(state, window)
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
