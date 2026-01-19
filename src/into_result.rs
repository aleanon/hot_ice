use iced::{Element, Subscription, Task};
use iced_core::theme;

use crate::error::{HotIceError, HotResult};

pub trait IntoResult<T> {
    fn into_result(self) -> Result<T, HotIceError>;
}

// ============================================================================
// Element: blanket impl for any T: Into<Element>
// ============================================================================
impl<'a, T, Message, Theme, Renderer> IntoResult<Element<'a, Message, Theme, Renderer>> for T
where
    T: Into<Element<'a, Message, Theme, Renderer>>,
{
    fn into_result(self) -> Result<Element<'a, Message, Theme, Renderer>, HotIceError> {
        Ok(self.into())
    }
}

// Element: impl for HotResult<T> where T: Into<Element>
impl<'a, T, Message, Theme, Renderer> IntoResult<Element<'a, Message, Theme, Renderer>>
    for HotResult<T>
where
    T: Into<Element<'a, Message, Theme, Renderer>>,
{
    fn into_result(self) -> Result<Element<'a, Message, Theme, Renderer>, HotIceError> {
        self.0.map(Into::into)
    }
}

// ============================================================================
// Task
// ============================================================================
impl<T: Into<Task<Message>>, Message> IntoResult<Task<Message>> for T {
    fn into_result(self) -> Result<Task<Message>, HotIceError> {
        Ok(self.into())
    }
}

impl<Message> IntoResult<Task<Message>> for HotResult<Task<Message>> {
    fn into_result(self) -> Result<Task<Message>, HotIceError> {
        self.0
    }
}

// ============================================================================
// f32 (scale factor)
// ============================================================================
impl<T: Into<f32>> IntoResult<f32> for T {
    fn into_result(self) -> Result<f32, HotIceError> {
        Ok(self.into())
    }
}

impl IntoResult<f32> for HotResult<f32> {
    fn into_result(self) -> Result<f32, HotIceError> {
        self.0
    }
}

// ============================================================================
// String (title)
// ============================================================================
impl<T: Into<String>> IntoResult<String> for T {
    fn into_result(self) -> Result<String, HotIceError> {
        Ok(self.into())
    }
}

impl IntoResult<String> for HotResult<String> {
    fn into_result(self) -> Result<String, HotIceError> {
        self.0
    }
}

// ============================================================================
// theme::Style
// ============================================================================
impl<T: Into<theme::Style>> IntoResult<theme::Style> for T {
    fn into_result(self) -> Result<theme::Style, HotIceError> {
        Ok(self.into())
    }
}

impl IntoResult<theme::Style> for HotResult<theme::Style> {
    fn into_result(self) -> Result<theme::Style, HotIceError> {
        self.0
    }
}

// ============================================================================
// Subscription
// ============================================================================
impl<T, Message> IntoResult<Subscription<Message>> for T
where
    T: Into<Subscription<Message>>,
{
    fn into_result(self) -> Result<Subscription<Message>, HotIceError> {
        Ok(self.into())
    }
}

impl<Message> IntoResult<Subscription<Message>> for HotResult<Subscription<Message>> {
    fn into_result(self) -> Result<Subscription<Message>, HotIceError> {
        self.0
    }
}

// ============================================================================
// Option<Theme> (theme function returns Option)
// ============================================================================
impl<T, Theme> IntoResult<Option<Theme>> for T
where
    T: Into<Option<Theme>>,
{
    fn into_result(self) -> Result<Option<Theme>, HotIceError> {
        Ok(self.into())
    }
}

impl<Theme> IntoResult<Option<Theme>> for HotResult<Option<Theme>> {
    fn into_result(self) -> Result<Option<Theme>, HotIceError> {
        self.0
    }
}
