use iced::{widget::{button, column, container, text_input, row}, window, Length};
pub use iced::{time, widget::text, Element, Subscription, Task};
use serde::{Deserialize, Serialize};


#[derive(Debug, Clone)]
pub enum Message {
    None,
    Close,
    UpdateName(String),
    UpdateOtherName(String),
}

pub enum Action {
    None,
    Task(Task<Message>),
}
#[derive(Serialize, Deserialize)]
pub struct Reloadable {
    pub other_name: String,
    pub name: String,
}

impl Reloadable {
    pub fn new() -> Self {
        Self {
            name: String::from("test text"),
            other_name: String::from("other_name")
        }
    }

    #[no_mangle]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::UpdateName(name) => self.name = name,
            Message::UpdateOtherName(name) => self.name = name,
            Message::Close => return window::get_latest().and_then(window::close),
            Message::None => {}
        }
        Task::none()
    }

    #[no_mangle]
    pub fn view(&self) -> Element<Message> {
        container(column![
            text_input("Enter name", &self.name)
                .on_input(Message::UpdateName).size(20),
            text_input("Enter name", &self.other_name)
                .on_input(Message::UpdateOtherName).size(20),
            button("exit").on_press(Message::Close),
            button("exit").on_press(Message::Close),
            button("exit").on_press(Message::Close),
            button("exit").on_press(Message::Close),
            button("exit").on_press(Message::Close),
            button("exit").on_press(Message::Close),
        ]
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    }

    pub fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            time::every(time::Duration::from_millis(500))
                .map(|_| Message::None),
        ])
    }


}

