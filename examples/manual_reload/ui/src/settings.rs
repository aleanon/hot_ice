use hot_ice::iced::widget::{column, container, pick_list, row, slider, text};
use hot_ice::iced::{Element, Length, Subscription, Task, Theme, theme, window};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum Message {
    ThemeChanged(ThemeChoice),
    ScaleChanged(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeChoice {
    Light,
    #[default]
    Dark,
    Dracula,
    Nord,
    SolarizedLight,
    SolarizedDark,
    GruvboxLight,
    GruvboxDark,
    CatppuccinLatte,
    CatppuccinFrappe,
    CatppuccinMacchiato,
    CatppuccinMocha,
    TokyoNight,
    TokyoNightStorm,
    TokyoNightLight,
    KanagawaWave,
    KanagawaDragon,
    KanagawaLotus,
    Moonfly,
    Nightfly,
    Oxocarbon,
    Ferra,
}

impl ThemeChoice {
    const ALL: [ThemeChoice; 22] = [
        ThemeChoice::Light,
        ThemeChoice::Dark,
        ThemeChoice::Dracula,
        ThemeChoice::Nord,
        ThemeChoice::SolarizedLight,
        ThemeChoice::SolarizedDark,
        ThemeChoice::GruvboxLight,
        ThemeChoice::GruvboxDark,
        ThemeChoice::CatppuccinLatte,
        ThemeChoice::CatppuccinFrappe,
        ThemeChoice::CatppuccinMacchiato,
        ThemeChoice::CatppuccinMocha,
        ThemeChoice::TokyoNight,
        ThemeChoice::TokyoNightStorm,
        ThemeChoice::TokyoNightLight,
        ThemeChoice::KanagawaWave,
        ThemeChoice::KanagawaDragon,
        ThemeChoice::KanagawaLotus,
        ThemeChoice::Moonfly,
        ThemeChoice::Nightfly,
        ThemeChoice::Oxocarbon,
        ThemeChoice::Ferra,
    ];

    fn to_theme(self) -> Theme {
        match self {
            ThemeChoice::Light => Theme::Light,
            ThemeChoice::Dark => Theme::Dark,
            ThemeChoice::Dracula => Theme::Dracula,
            ThemeChoice::Nord => Theme::Nord,
            ThemeChoice::SolarizedLight => Theme::SolarizedLight,
            ThemeChoice::SolarizedDark => Theme::SolarizedDark,
            ThemeChoice::GruvboxLight => Theme::GruvboxLight,
            ThemeChoice::GruvboxDark => Theme::GruvboxDark,
            ThemeChoice::CatppuccinLatte => Theme::CatppuccinLatte,
            ThemeChoice::CatppuccinFrappe => Theme::CatppuccinFrappe,
            ThemeChoice::CatppuccinMacchiato => Theme::CatppuccinMacchiato,
            ThemeChoice::CatppuccinMocha => Theme::CatppuccinMocha,
            ThemeChoice::TokyoNight => Theme::TokyoNight,
            ThemeChoice::TokyoNightStorm => Theme::TokyoNightStorm,
            ThemeChoice::TokyoNightLight => Theme::TokyoNightLight,
            ThemeChoice::KanagawaWave => Theme::KanagawaWave,
            ThemeChoice::KanagawaDragon => Theme::KanagawaDragon,
            ThemeChoice::KanagawaLotus => Theme::KanagawaLotus,
            ThemeChoice::Moonfly => Theme::Moonfly,
            ThemeChoice::Nightfly => Theme::Nightfly,
            ThemeChoice::Oxocarbon => Theme::Oxocarbon,
            ThemeChoice::Ferra => Theme::Ferra,
        }
    }
}

impl std::fmt::Display for ThemeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ThemeChoice::Light => "Light",
                ThemeChoice::Dark => "Dark",
                ThemeChoice::Dracula => "Dracula",
                ThemeChoice::Nord => "Nord",
                ThemeChoice::SolarizedLight => "Solarized Light",
                ThemeChoice::SolarizedDark => "Solarized Dark",
                ThemeChoice::GruvboxLight => "Gruvbox Light",
                ThemeChoice::GruvboxDark => "Gruvbox Dark",
                ThemeChoice::CatppuccinLatte => "Catppuccin Latte",
                ThemeChoice::CatppuccinFrappe => "Catppuccin Frappe",
                ThemeChoice::CatppuccinMacchiato => "Catppuccin Macchiato",
                ThemeChoice::CatppuccinMocha => "Catppuccin Mocha",
                ThemeChoice::TokyoNight => "Tokyo Night",
                ThemeChoice::TokyoNightStorm => "Tokyo Night Storm",
                ThemeChoice::TokyoNightLight => "Tokyo Night Light",
                ThemeChoice::KanagawaWave => "Kanagawa Wave",
                ThemeChoice::KanagawaDragon => "Kanagawa Dragon",
                ThemeChoice::KanagawaLotus => "Kanagawa Lotus",
                ThemeChoice::Moonfly => "Moonfly",
                ThemeChoice::Nightfly => "Nightfly",
                ThemeChoice::Oxocarbon => "Oxocarbon",
                ThemeChoice::Ferra => "Ferra",
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    theme: ThemeChoice,
    scale: f32,
}

impl Default for State {
    fn default() -> Self {
        State {
            theme: ThemeChoice::default(),
            scale: 1.0,
        }
    }
}

impl State {
    pub fn boot() -> (State, Task<Message>) {
        (
            State {
                theme: ThemeChoice::Dark,
                scale: 1.0,
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ThemeChanged(theme) => {
                self.theme = theme;
            }
            Message::ScaleChanged(scale) => {
                self.scale = scale;
            }
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content = column![
            text("Settings").size(24),
            row![
                text("Theme:").size(16),
                pick_list(
                    &ThemeChoice::ALL[..],
                    Some(self.theme),
                    Message::ThemeChanged
                )
            ]
            .spacing(10)
            .align_y(hot_ice::iced::Alignment::Center),
            row![
                text("Scale:").size(18),
                slider(0.5..=2.0, self.scale, Message::ScaleChanged).step(0.1),
                text(format!("{:.1}x", self.scale)).size(16),
            ]
            .spacing(10)
            .align_y(hot_ice::iced::Alignment::Center),
        ]
        .spacing(20);

        container(content).padding(20).width(Length::Fill).into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    pub fn theme(&self) -> Theme {
        self.theme.to_theme()
    }

    pub fn scale_factor(&self) -> f32 {
        self.scale
    }
}
