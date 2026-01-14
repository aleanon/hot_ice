use hot_ice::iced::widget::{button, checkbox, column, container, row, space, text, text_input};
use hot_ice::iced::{Element, Length, Subscription, Task, Theme, theme};

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    AddTodo,
    ToggleTodo(usize),
    DeleteTodo(usize),
}

#[derive(Debug, Clone)]
pub struct TodoItem {
    text: String,
    completed: bool,
}

#[derive(Debug, Clone)]
pub struct State {
    input: String,
    todos: Vec<TodoItem>,
}

impl State {
    pub fn boot() -> (State, Task<Message>) {
        (
            State {
                input: String::new(),
                todos: vec![
                    TodoItem {
                        text: "Learn hot_ice".to_string(),
                        completed: false,
                    },
                    TodoItem {
                        text: "Build something cool".to_string(),
                        completed: false,
                    },
                ],
            },
            Task::none(),
        )
    }

    #[unsafe(no_mangle)]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::InputChanged(value) => {
                self.input = value;
            }
            Message::AddTodo => {
                if !self.input.trim().is_empty() {
                    self.todos.push(TodoItem {
                        text: self.input.clone(),
                        completed: false,
                    });
                    self.input.clear();
                }
            }
            Message::ToggleTodo(index) => {
                if let Some(todo) = self.todos.get_mut(index) {
                    todo.completed = !todo.completed;
                }
            }
            Message::DeleteTodo(index) => {
                if index < self.todos.len() {
                    self.todos.remove(index);
                }
            }
        }

        Task::none()
    }

    #[unsafe(no_mangle)]
    pub fn view(&self) -> Element<'_, Message> {
        let input_row = row![
            text_input("Add a todo...", &self.input)
                .on_input(Message::InputChanged)
                .on_submit(Message::AddTodo)
                .padding(10),
            button("Add").on_press(Message::AddTodo).padding(10),
        ]
        .spacing(10);

        let mut todo_column = column![text("Todo List").size(24), input_row].spacing(10);

        for (index, todo) in self.todos.iter().enumerate() {
            let todo_row = row![
                checkbox(todo.completed).on_toggle(move |_| Message::ToggleTodo(index)),
                text(&todo.text).size(16),
                button("Delete")
                    .on_press(Message::DeleteTodo(index))
                    .padding(5),
            ]
            .spacing(10)
            .align_y(hot_ice::iced::Alignment::Center);

            todo_column = todo_column.push(todo_row);
        }

        let completed_count = self.todos.iter().filter(|t| t.completed).count();
        let stats = text(format!(
            "{}/{} completed",
            completed_count,
            self.todos.len()
        ))
        .size(12);

        todo_column = todo_column.push(stats);

        let content = row![
            space().width(Length::Fill),
            todo_column,
            space().width(Length::Fill)
        ];

        container(content).center(Length::Fill).into()
    }

    #[unsafe(no_mangle)]
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    #[unsafe(no_mangle)]
    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    #[unsafe(no_mangle)]
    pub fn style(&self, theme: &Theme) -> theme::Style {
        theme::default(theme)
    }

    #[unsafe(no_mangle)]
    pub fn scale_factor(&self) -> f32 {
        1.0
    }

    #[unsafe(no_mangle)]
    pub fn title(&self) -> String {
        format!("Todo List: {} items", self.todos.len())
    }
}
