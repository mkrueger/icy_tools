use iced::{
    Element, Length, Task,
    widget::{column, container},
};
use icy_engine::{Screen, TextBuffer, TextScreen};
use icy_engine_gui::{ui::{button_row, dialog_area, primary_button, separator}, version_helper::replace_version_marker};
use icy_engine_gui::{MonitorSettings, Terminal, TerminalView};
use icy_parser_core::MusicOption;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::{
    VERSION,
    ui::{MainWindowMode, Message},
};

// Include the help ANSI file at compile time
pub const ABOUT_ANSI: &[u8] = include_bytes!("../../../data/about.icy");

pub struct AboutDialog {
    terminal: Terminal,
}

impl AboutDialog {
    pub fn new(ansi: &[u8]) -> Self {
        // Create an edit state and load the help ANSI
        let mut screen = TextScreen::new(icy_engine::Size::new(80, 25));

        // Load the help ANSI file
        match TextBuffer::from_bytes(std::path::Path::new("a.icy"), true, ansi, Some(MusicOption::Off), None) {
            Ok(mut buffer) => {
                let build_date = option_env!("ICY_BUILD_DATE").unwrap_or("-").to_string().to_string();
                replace_version_marker(&mut buffer, &VERSION, Some(build_date));
                screen.buffer = buffer;
            }
            Err(e) => {
                panic!("Failed to load help ANSI: {}", e);
            }
        }
        screen.caret.visible = false;

        let edit_screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(screen)));
        let terminal = Terminal::new(edit_screen.clone());

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
        settings.use_integer_scaling = false;

        let terminal_view = TerminalView::show_with_effects(&self.terminal, settings).map(|terminal_msg| match terminal_msg {
            icy_engine_gui::Message::OpenLink(url) => Message::OpenLink(url),
            _ => Message::None,
        });

        let ok_button = primary_button(
            format!("{}", icy_engine_gui::ButtonType::Ok),
            Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal))),
        );

        let buttons = button_row(vec![ok_button.into()]);

        let content = column![container(terminal_view).height(Length::Fill), separator(), dialog_area(buttons),].spacing(0);
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

