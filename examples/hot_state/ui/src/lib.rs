use iced::widget::{column, container, row, text};
use iced::{Element, Length, Subscription, Task, Theme, theme};
use serde::{Deserialize, Serialize};

pub mod counter;
pub mod settings;
pub mod todo_list;

#[derive(Debug, Clone)]
pub enum Message {
    Counter(counter::Message),
    TodoList(todo_list::Message),
    Settings(settings::Message),
}

#[cfg_attr(feature = "reload", hot_ice::hot_state)]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    counter: counter::State,
    todo_list: todo_list::State,
    settings: settings::State,
}

impl State {
    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn new() -> (State, Task<Message>) {
        let (counter, counter_task) = counter::State::new();
        let (todo_list, todo_task) = todo_list::State::new();
        let (settings, settings_task) = settings::State::new();

        (
            State {
                counter,
                todo_list,
                settings,
            },
            Task::batch([
                counter_task.map(Message::Counter),
                todo_task.map(Message::TodoList),
                settings_task.map(Message::Settings),
            ]),
        )
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Counter(msg) => self.counter.update(msg).map(Message::Counter),
            Message::TodoList(msg) => self.todo_list.update(msg).map(Message::TodoList),
            Message::Settings(msg) => self.settings.update(msg).map(Message::Settings),
        }
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn view(&self) -> Element<'_, Message> {
        let content = column![
            text("Hot State Example").size(34),
            text("This demonstrates nested state with hot reloading").size(16),
            row![
                container(self.counter.view().map(Message::Counter))
                    .padding(20)
                    .width(Length::Fill),
                container(self.todo_list.view().map(Message::TodoList))
                    .padding(20)
                    .width(Length::Fill),
            ]
            .spacing(20),
            container(self.settings.view().map(Message::Settings))
                .padding(20)
                .width(Length::Fill),
        ]
        .spacing(20)
        .padding(20);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            self.counter.subscription().map(Message::Counter),
            self.todo_list.subscription().map(Message::TodoList),
            self.settings.subscription().map(Message::Settings),
        ])
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn theme(&self) -> Option<Theme> {
        Some(self.settings.theme())
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn style(&self, theme: &Theme) -> theme::Style {
        self.settings.style(theme)
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn scale_factor(&self) -> f32 {
        self.settings.scale_factor()
    }

    #[cfg_attr(feature = "reload", hot_ice::hot_fn(hot_state))]
    pub fn title(&self) -> String {
        format!("Hot State Example - Counter: {}", self.counter.value())
    }
}
