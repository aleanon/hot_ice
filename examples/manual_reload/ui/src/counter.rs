use hot_ice::iced::widget::{button, column, container, row, text};
use hot_ice::iced::{Element, Length, Subscription, Task};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum Message {
    Increment,
    Decrement,
    Reset,
    Tick,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct State {
    value: i32,
    auto_increment: bool,
}

impl State {
    pub fn new() -> (State, Task<Message>) {
        (
            State {
                value: 0,
                auto_increment: false,
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Increment => {
                self.value += 1;
            }
            Message::Decrement => {
                self.value -= 1;
            }
            Message::Reset => {
                self.value = 0;
            }
            Message::Tick => {
                if self.auto_increment {
                    self.value += 1;
                }
            }
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content = column![
            text("Counter Module").size(24),
            text(self.value).size(50),
            row![
                button("−").on_press(Message::Decrement).padding(10),
                button("Reset").on_press(Message::Reset).padding(10),
                button("+").on_press(Message::Increment).padding(10),
            ]
            .spacing(10),
            text(if self.auto_increment {
                "Auto-increment: ON"
            } else {
                "Auto-increment: OFF"
            })
            .size(14),
        ]
        .spacing(20)
        .align_x(hot_ice::iced::Alignment::Center);

        container(content).center(Length::Fill).padding(20).into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    pub fn value(&self) -> i32 {
        self.value
    }
}
