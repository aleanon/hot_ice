use iced::{futures::io::Take, Length, widget::{container, text, text_input}, Element, Task};

#[derive(Debug, Clone)]
pub enum Message {
    UpdateName(String),
}

pub struct NewStruct {
    yet_another_name: String,
}


impl NewStruct {
    pub fn new() -> Self {
        Self {
            yet_another_name: String::from("chuck")
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::UpdateName(name) => self.yet_another_name = name,
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        container(
            text_input("Enter name", &self.yet_another_name)
                .on_input(Message::UpdateName)
        )
        .padding(50)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    }
}