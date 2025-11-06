use i18n_embed_fl::fl;
use iced::{
    Element, Length, Task,
    widget::{Space, button, column, container, row, text},
};
use iced_engine_gui::{MonitorSettings, Terminal, TerminalView};
use icy_engine::{Buffer, ansi::MusicOption, editor::EditState};
use std::sync::{Arc, Mutex};

use crate::ui::{MainWindowMode, Message};

// Include the help ANSI file at compile time
pub const HELP_ANSI: &[u8] = include_bytes!("../../../data/help.icy");
pub const ABOUT_ANSI: &[u8] = include_bytes!("../../../data/about.icy");

pub struct AnsiDialog {
    terminal: Terminal,
}

impl AnsiDialog {
    pub fn new(ansi: &[u8]) -> Self {
        // Create an edit state and load the help ANSI
        let mut edit_state = EditState::default();

        // Load the help ANSI file
        match Buffer::from_bytes(std::path::Path::new("a.icy"), true, ansi, Some(MusicOption::Off), None) {
            Ok(buffer) => {
                edit_state.set_buffer(buffer);
            }
            Err(e) => {
                panic!("Failed to load help ANSI: {}", e);
            }
        }
        edit_state.get_caret_mut().set_is_visible(false);

        let edit_state = Arc::new(Mutex::new(edit_state));
        let mut terminal = Terminal::new(edit_state.clone());
        terminal.redraw();

        Self { terminal }
    }

    pub fn show(&self) -> bool {
        // Return true if dialog should be shown
        true
    }

    pub fn update(&mut self, _message: Message) -> Task<Message> {
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut settings = MonitorSettings::neutral();
        settings.use_pixel_perfect_scaling = false;

        let content = column![
            // Terminal view showing the help ANSI
            container(TerminalView::show_with_effects(&self.terminal, settings).map(|_| Message::None)).padding(8),
            // Bottom button bar
            container(row![
                Space::new().width(Length::Fill),
                button(text(fl!(crate::LANGUAGE_LOADER, "dialog-ok_button")).size(14))
                    .on_press(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)))
                    .style(button::primary)
                    .padding([6, 20]),
            ])
            .padding([8, 12])
            .width(Length::Fill)
            .style(|theme: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(theme.extended_palette().background.weak.color)),
                ..Default::default()
            }),
        ]
        .spacing(0);
        /*
        // Wrap in a centered modal overlay
        container(
            container(content)
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(
                        theme.extended_palette().background.base.color
                    )),
                    border: iced::Border {
                        color: theme.extended_palette().background.strong.color,
                        width: 2.0,
                        radius: 8.0.into(),
                    },
                    shadow: iced::Shadow {
                        color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                        offset: iced::Vector::new(0.0, 4.0),
                        blur_radius: 16.0,
                    },
                    ..Default::default()
                })
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(
                iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7)
            )),
            ..Default::default()
        })*/
        content.into()
    }
}
