use iced::{
    Element, Length, Task,
    widget::{column, container},
};
use iced_engine_gui::ui::{button_row, dialog_area, primary_button, separator};
use iced_engine_gui::{MonitorSettings, Terminal, TerminalView};
use icy_engine::{AttributedChar, Position, Screen, TextAttribute, TextBuffer, TextPane, TextScreen};
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
                highlight_version(&mut buffer);

                for y in 0..buffer.get_height() {
                    for x in 0..buffer.get_width() {
                        let ch = buffer.get_char((x, y).into());

                        if ch.ch == '@' {
                            // Build version string with colors
                            let build_date = option_env!("ICY_BUILD_DATE").unwrap_or("-").to_string().to_string();

                            // Place the colored version at the @ position
                            for (i, ch) in build_date.chars().enumerate() {
                                let new_x = x + i as i32;
                                if new_x < buffer.get_width() {
                                    let new_ch = AttributedChar::new(ch, TextAttribute::from_u8(0x08, icy_engine::IceMode::Ice));
                                    buffer.layers[0].set_char(Position::new(new_x, y), new_ch);
                                }
                            }
                        }
                    }
                }
                buffer.update_hyperlinks();

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
        settings.use_pixel_perfect_scaling = false;

        let terminal_view = TerminalView::show_with_effects(&self.terminal, settings).map(|terminal_msg| match terminal_msg {
            iced_engine_gui::Message::OpenLink(url) => Message::OpenLink(url),
            _ => Message::None,
        });

        let ok_button = primary_button(
            format!("{}", iced_engine_gui::ButtonType::Ok),
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

fn highlight_version(buffer: &mut TextBuffer) {
    for y in 0..buffer.get_height() {
        for x in 0..buffer.get_width() {
            let ch = buffer.get_char((x, y).into());

            if ch.ch == '@' {
                // Build version string with colors
                let mut version_chars = Vec::new();

                // 'v' in white (color 7)
                version_chars.push(AttributedChar::new('v', TextAttribute::from_u8(0x07, icy_engine::IceMode::Ice)));

                // Major version in yellow (color 14)
                let major_str = VERSION.major.to_string();
                for ch in major_str.chars() {
                    version_chars.push(AttributedChar::new(ch, TextAttribute::from_u8(0x0E, icy_engine::IceMode::Ice)));
                }

                // First dot in green (color 10)
                version_chars.push(AttributedChar::new('.', TextAttribute::from_u8(0x0A, icy_engine::IceMode::Ice)));

                // Minor version in light red (color 12)
                let minor_str = VERSION.minor.to_string();
                for ch in minor_str.chars() {
                    version_chars.push(AttributedChar::new(ch, TextAttribute::from_u8(0x0C, icy_engine::IceMode::Ice)));
                }

                // Second dot in green (color 10)
                version_chars.push(AttributedChar::new('.', TextAttribute::from_u8(0x0A, icy_engine::IceMode::Ice)));

                // Patch/build version in magenta (color 13)
                let patch_str = VERSION.patch.to_string();
                for ch in patch_str.chars() {
                    version_chars.push(AttributedChar::new(ch, TextAttribute::from_u8(0x0D, icy_engine::IceMode::Ice)));
                }

                // Place the colored version at the @ position
                for (i, new_ch) in version_chars.into_iter().enumerate() {
                    let new_x = x + i as i32;
                    if new_x < buffer.get_width() {
                        buffer.layers[0].set_char(Position::new(new_x, y), new_ch);
                    }
                }
                return;
            }
        }
    }
}
