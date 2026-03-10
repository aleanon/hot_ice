use std::collections::VecDeque;

use iced::keyboard::{self, Key, Modifiers};
use iced::widget::{column, container, row, scrollable, text};
use iced::{Element, Length, Subscription, Task, Theme, theme};

/// Maximum number of key events to keep in the log.
const MAX_EVENTS: usize = 20;

#[derive(Debug, Clone)]
pub enum Message {
    KeyEvent(keyboard::Event),
}

#[cfg_attr(
    feature = "reload",
    hot_ice::hot_state,
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Debug, Clone, Default)]
pub struct State {
    /// Running count of key presses received.
    count: u64,
    /// Log of recent key names for display.
    recent_keys: VecDeque<String>,
}

impl State {
    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn new() -> (State, Task<Message>) {
        (State::default(), Task::none())
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::KeyEvent(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                self.count += 1;

                let key_name = match &key {
                    Key::Named(named) => format!("{named:?}"),
                    Key::Character(c) => c.to_string(),
                    Key::Unidentified => "?".to_string(),
                };

                let entry = format_with_modifiers(&key_name, modifiers);

                self.recent_keys.push_back(entry);
                if self.recent_keys.len() > MAX_EVENTS {
                    self.recent_keys.pop_front();
                }
            }
            // Ignore key release events
            Message::KeyEvent(_) => {}
        }
        Task::none()
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn view(&self) -> Element<'_, Message> {
        let header = column![
            text("Keyboard Events").size(28),
            text(format!("Total key presses: {}", self.count)).size(16),
        ]
        .spacing(8);

        let log: Element<'_, Message> = if self.recent_keys.is_empty() {
            text("Press any key…").size(14).into()
        } else {
            let items: Vec<Element<'_, Message>> = self
                .recent_keys
                .iter()
                .rev()
                .enumerate()
                .map(|(i, key)| {
                    let n = self.count as usize - i;
                    row![text(format!("{n:>4}")).size(13), text(key).size(14),]
                        .spacing(12)
                        .into()
                })
                .collect();
            scrollable(column(items).spacing(4)).into()
        };

        let content = column![header, log].spacing(20).max_width(480);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .padding(20)
            .into()
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn subscription(&self) -> Subscription<Message> {
        keyboard::listen().map(Message::KeyEvent)
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn theme(&self) -> Option<Theme> {
        Some(Theme::Dark)
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn style(&self, theme: &Theme) -> theme::Style {
        theme::default(theme)
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn title(&self) -> String {
        format!("Keyboard Events — {} presses", self.count)
    }
}

fn format_with_modifiers(key_name: &str, modifiers: Modifiers) -> String {
    if modifiers.is_empty() {
        return key_name.to_string();
    }
    let mut parts = Vec::new();
    if modifiers.control() {
        parts.push("Ctrl");
    }
    if modifiers.alt() {
        parts.push("Alt");
    }
    if modifiers.shift() {
        parts.push("Shift");
    }
    if modifiers.logo() {
        parts.push("Super");
    }
    parts.push(key_name);
    parts.join("+")
}
