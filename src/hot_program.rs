//! The definition of an iced program.

use iced_core::text;
use iced_core::theme;
use iced_core::window;
use iced_core::Element;
use iced_futures::{Executor, Subscription};
use iced_winit::graphics::compositor;
use iced_winit::runtime::Task;

use crate::hot_subscription::HotSubscription;
use crate::hot_subscription::IntoHotSubscription;
use crate::message::MessageSource;
use crate::DynMessage;

/// An interactive, native, cross-platform, multi-windowed application.
///
/// A [`Program`] can execute asynchronous actions by returning a
/// [`Task`] in some of its methods.
#[allow(missing_docs)]
pub trait HotProgram: Sized {
    /// The state of the program.
    type State;

    /// The message of the program.
    type Message: DynMessage + Clone;

    /// The theme of the program.
    type Theme: Default + theme::Base;

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
    ) -> Task<MessageSource<Self::Message>>;

    fn view<'a>(
        &self,
        state: &'a Self::State,
        window: window::Id,
    ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer>;

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

    fn subscription(&self, _state: &Self::State) -> Subscription<MessageSource<Self::Message>> {
        Subscription::none()
    }

    fn theme(&self, _state: &Self::State, _window: window::Id) -> Self::Theme {
        <Self::Theme as Default>::default()
    }

    fn style(&self, _state: &Self::State, theme: &Self::Theme) -> theme::Style {
        theme::Base::base(theme)
    }

    fn scale_factor(&self, _state: &Self::State, _window: window::Id) -> f64 {
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
        ) -> Task<MessageSource<Self::Message>> {
            self.program.update(state, message)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
        ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer> {
            self.program.view(state, window)
        }

        fn theme(&self, state: &Self::State, window: window::Id) -> Self::Theme {
            self.program.theme(state, window)
        }

        fn subscription(&self, state: &Self::State) -> Subscription<MessageSource<Self::Message>> {
            self.program.subscription(state)
        }

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f64 {
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

        fn subscription(&self, state: &Self::State) -> Subscription<MessageSource<Self::Message>> {
            self.subscription.subscription(state)
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
        ) -> Task<MessageSource<Self::Message>> {
            self.program.update(state, message)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
        ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer> {
            self.program.view(state, window)
        }

        fn title(&self, state: &Self::State, window: window::Id) -> String {
            self.program.title(state, window)
        }

        fn theme(&self, state: &Self::State, window: window::Id) -> Self::Theme {
            self.program.theme(state, window)
        }

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f64 {
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
    f: impl Fn(&P::State, window::Id) -> P::Theme,
) -> impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme> {
    struct WithTheme<P, F> {
        program: P,
        theme: F,
    }

    impl<P: HotProgram, F> HotProgram for WithTheme<P, F>
    where
        F: Fn(&P::State, window::Id) -> P::Theme,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = P::Executor;

        fn theme(&self, state: &Self::State, window: window::Id) -> Self::Theme {
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
        ) -> Task<MessageSource<Self::Message>> {
            self.program.update(state, message)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
        ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer> {
            self.program.view(state, window)
        }

        fn subscription(&self, state: &Self::State) -> Subscription<MessageSource<Self::Message>> {
            self.program.subscription(state)
        }

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f64 {
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
        ) -> Task<MessageSource<Self::Message>> {
            self.program.update(state, message)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
        ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer> {
            self.program.view(state, window)
        }

        fn subscription(&self, state: &Self::State) -> Subscription<MessageSource<Self::Message>> {
            self.program.subscription(state)
        }

        fn theme(&self, state: &Self::State, window: window::Id) -> Self::Theme {
            self.program.theme(state, window)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f64 {
            self.program.scale_factor(state, window)
        }
    }

    WithStyle { program, style: f }
}

/// Decorates a [`Program`] with the given scale factor function.
pub fn with_scale_factor<P: HotProgram>(
    program: P,
    f: impl Fn(&P::State, window::Id) -> f64,
) -> impl HotProgram<State = P::State, Message = P::Message, Theme = P::Theme> {
    struct WithScaleFactor<P, F> {
        program: P,
        scale_factor: F,
    }

    impl<P: HotProgram, F> HotProgram for WithScaleFactor<P, F>
    where
        F: Fn(&P::State, window::Id) -> f64,
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
        ) -> Task<MessageSource<Self::Message>> {
            self.program.update(state, message)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
        ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer> {
            self.program.view(state, window)
        }

        fn subscription(&self, state: &Self::State) -> Subscription<MessageSource<Self::Message>> {
            self.program.subscription(state)
        }

        fn theme(&self, state: &Self::State, window: window::Id) -> Self::Theme {
            self.program.theme(state, window)
        }

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f64 {
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
        ) -> Task<MessageSource<Self::Message>> {
            self.program.update(state, message)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
        ) -> Element<'a, MessageSource<Self::Message>, Self::Theme, Self::Renderer> {
            self.program.view(state, window)
        }

        fn subscription(&self, state: &Self::State) -> Subscription<MessageSource<Self::Message>> {
            self.program.subscription(state)
        }

        fn theme(&self, state: &Self::State, window: window::Id) -> Self::Theme {
            self.program.theme(state, window)
        }

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f64 {
            self.program.scale_factor(state, window)
        }
    }

    WithExecutor {
        program,
        executor: PhantomData::<E>,
    }
}

/// The renderer of some [`Program`].
pub trait Renderer: text::Renderer + compositor::Default {}

impl<T> Renderer for T where T: text::Renderer + compositor::Default {}

/// A particular instance of a running [`Program`].
#[allow(missing_debug_implementations)]
pub struct Instance<P: HotProgram> {
    program: P,
    state: P::State,
}

impl<P: HotProgram> Instance<P> {
    /// Creates a new [`Instance`] of the given [`Program`].
    pub fn new(program: P) -> (Self, Task<MessageSource<P::Message>>) {
        let (state, task) = program.boot();

        (Self { program, state }, task)
    }

    /// Returns the current title of the [`Instance`].
    pub fn title(&self, window: window::Id) -> String {
        self.program.title(&self.state, window)
    }

    /// Processes the given message and updates the [`Instance`].
    pub fn update(
        &mut self,
        message: MessageSource<P::Message>,
    ) -> Task<MessageSource<P::Message>> {
        self.program.update(&mut self.state, message)
    }

    /// Produces the current widget tree of the [`Instance`].
    pub fn view(
        &self,
        window: window::Id,
    ) -> Element<'_, MessageSource<P::Message>, P::Theme, P::Renderer> {
        self.program.view(&self.state, window)
    }

    /// Returns the current [`Subscription`] of the [`Instance`].
    pub fn subscription(&self) -> Subscription<MessageSource<P::Message>> {
        self.program.subscription(&self.state)
    }

    /// Returns the current theme of the [`Instance`].
    pub fn theme(&self, window: window::Id) -> P::Theme {
        self.program.theme(&self.state, window)
    }

    /// Returns the current [`theme::Style`] of the [`Instance`].
    pub fn style(&self, theme: &P::Theme) -> theme::Style {
        self.program.style(&self.state, theme)
    }

    /// Returns the current scale factor of the [`Instance`].
    pub fn scale_factor(&self, window: window::Id) -> f64 {
        self.program.scale_factor(&self.state, window)
    }
}

/// A trait alias for the [`Message`](Program::Message) of a [`Program`].
#[cfg(feature = "time-travel")]
pub trait Message: Send + std::fmt::Debug + Clone {}

#[cfg(feature = "time-travel")]
impl<T: Send + std::fmt::Debug + Clone> Message for T {}

/// A trait alias for the [`Message`](Program::Message) of a [`Program`].
#[cfg(not(feature = "time-travel"))]
pub trait Message: Send + std::fmt::Debug {}

#[cfg(not(feature = "time-travel"))]
impl<T: Send + std::fmt::Debug> Message for T {}
