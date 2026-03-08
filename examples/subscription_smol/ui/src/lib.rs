use iced::time::Instant;
use iced::widget::{button, column, container, progress_bar, text};
use iced::{Element, Length, Subscription, Task, Theme, theme, time};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Message {
    Tick(Instant),
    ToggleSpeed,
}

/// Interval between ticks. Edit this constant and save — the subscription
/// frequency updates live without restarting the application.
const INTERVAL_MS: u64 = 1000;

#[cfg_attr(
    feature = "reload",
    hot_ice::hot_state,
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Debug, Clone, Default)]
pub struct State {
    count: u64,
    /// When true the interval is divided by 4 (250 ms instead of 1 s).
    fast: bool,
}

impl State {
    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn new() -> (State, Task<Message>) {
        (State::default(), Task::none())
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick(_) => {
                self.count += 1;
            }
            Message::ToggleSpeed => {
                self.fast = !self.fast;
            }
        }
        Task::none()
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn view(&self) -> Element<'_, Message> {
        let interval_ms = if self.fast {
            INTERVAL_MS / 4
        } else {
            INTERVAL_MS
        };
        let segment = self.count % 10;
        let fill = segment as f32 / 10.0;

        let speed_label = if self.fast {
            format!("Switch to {}ms", INTERVAL_MS)
        } else {
            format!("Switch to {}ms", INTERVAL_MS / 4)
        };

        let content = column![
            text("Pulse Counter").size(28),
            text(self.count).size(80),
            progress_bar(0.0..=1.0, fill),
            text(format!(
                "Tick every {}ms  ·  {}/10 until next bar fill",
                interval_ms, segment
            ))
            .size(13),
            button(text(speed_label))
                .on_press(Message::ToggleSpeed)
                .padding(12),
            text("Tip: change INTERVAL_MS and save to hot-reload the frequency").size(12),
        ]
        .spacing(20)
        .align_x(iced::Alignment::Center)
        .max_width(420);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .into()
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn subscription(&self) -> Subscription<Message> {
        let ms = if self.fast {
            INTERVAL_MS / 4
        } else {
            INTERVAL_MS
        };
        time::every(Duration::from_millis(ms)).map(Message::Tick)
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn theme(&self) -> Option<Theme> {
        Some(Theme::Light)
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn style(&self, theme: &Theme) -> theme::Style {
        theme::default(theme)
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn title(&self) -> String {
        format!("Pulse Counter — {}", self.count)
    }
}
