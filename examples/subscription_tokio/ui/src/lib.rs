use iced::time::Instant;
use iced::widget::{button, column, container, row, text};
use iced::{Element, Length, Subscription, Task, Theme, theme, time};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Message {
    Tick(Instant),
    Toggle,
    Reset,
}

#[cfg_attr(
    feature = "reload",
    hot_ice::hot_state,
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Debug, Clone, Default)]
pub struct State {
    /// Elapsed time tracked in milliseconds.
    elapsed_ms: u64,
    running: bool,
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
                if self.running {
                    self.elapsed_ms += 10;
                }
            }
            Message::Toggle => {
                self.running = !self.running;
            }
            Message::Reset => {
                self.elapsed_ms = 0;
                self.running = false;
            }
        }
        Task::none()
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn view(&self) -> Element<'_, Message> {
        let minutes = self.elapsed_ms / 60_000;
        let seconds = (self.elapsed_ms % 60_000) / 1_000;
        let centis = (self.elapsed_ms % 1_000) / 10;

        let time_display = text(format!("{:02}:{:02}.{:02}", minutes, seconds, centis)).size(80);

        let controls = row![
            button(if self.running { "Pause" } else { "Start" })
                .on_press(Message::Toggle)
                .padding(12),
            button("Reset").on_press(Message::Reset).padding(12),
        ]
        .spacing(16);

        let content = column![
            text("Stopwatch").size(28),
            time_display,
            controls,
            text("Ticks every 10 ms via iced::time::every (tokio)").size(13),
        ]
        .spacing(28)
        .align_x(iced::Alignment::Center);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .into()
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn subscription(&self) -> Subscription<Message> {
        if self.running {
            time::every(Duration::from_millis(10)).map(Message::Tick)
        } else {
            Subscription::none()
        }
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
        let minutes = self.elapsed_ms / 60_000;
        let seconds = (self.elapsed_ms % 60_000) / 1_000;
        let centis = (self.elapsed_ms % 1_000) / 10;
        format!("Stopwatch — {:02}:{:02}.{:02}", minutes, seconds, centis)
    }
}
