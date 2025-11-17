/// Currently just uses the trait from the Iced crate, Not hot yet.
use iced_core::Theme;

/// The theme logic of some [`Application`].
///
/// Any implementors of this trait can be provided as an argument to
/// [`Application::theme`].
///
/// `iced` provides two implementors:
/// - the built-in [`Theme`] itself
/// - and any `Fn(&State) -> impl Into<Option<Theme>>`.
pub trait ThemeFn<State, Theme> {
    /// Returns the theme of the [`Application`] for the current state.
    ///
    /// If `None` is returned, `iced` will try to use a theme that
    /// matches the system color scheme.
    fn theme(&self, state: &State) -> Option<Theme>;
}

impl<State> ThemeFn<State, Theme> for Theme {
    fn theme(&self, _state: &State) -> Option<Theme> {
        Some(self.clone())
    }
}

impl<F, T, State, Theme> ThemeFn<State, Theme> for F
where
    F: Fn(&State) -> T,
    T: Into<Option<Theme>>,
{
    fn theme(&self, state: &State) -> Option<Theme> {
        (self)(state).into()
    }
}
