use iced::{
    widget::{button, column, container, row, text, Space, svg},
    Alignment, Element, Length, Border, Color,
};
use i18n_embed_fl::fl;
// use iced_aw::{menu, menu_bar, menu_items};

use crate::ui::Message;

// Icon SVG constants
const DISCONNECT_SVG: &[u8] = include_bytes!("../../data/icons/logout.svg");
const PHONEBOOK_SVG: &[u8] = include_bytes!("../../data/icons/call.svg");
const UPLOAD_SVG: &[u8] = include_bytes!("../../data/icons/upload.svg");
const DOWNLOAD_SVG: &[u8] = include_bytes!("../../data/icons/download.svg");
const SETTINGS_SVG: &[u8] = include_bytes!("../../data/icons/menu.svg");

pub struct TerminalWindow {
    is_connected: bool,
    is_capturing: bool,
}

impl TerminalWindow {
    pub fn new() -> Self {
        Self {
            is_connected: false,
            is_capturing: false,
        }
    }
    
    pub fn view(&self) -> Element<'_, Message> {
        // Create the button bar at the top
        let button_bar = self.create_button_bar();
        
        // Create the main terminal area (placeholder for now)
        let terminal_area = container(
            text("Terminal area - TODO: Implement terminal rendering")
                .size(14)
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(10)
        .style(|theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb(0.05, 0.05, 0.05))),
            text_color: Some(iced::Color::from_rgb(0.7, 0.7, 0.7)),
            border: iced::Border {
                color: iced::Color::from_rgb(0.2, 0.2, 0.2),
                width: 1.0,
                radius: 0.0.into(),
            },
            shadow: Default::default(),
            snap: false,
        });
        
        // Status bar at the bottom
        let status_bar = self.create_status_bar();
        
        // Combine all elements
        column![
            button_bar,
            terminal_area,
            status_bar
        ]
        .spacing(0)
        .into()
    }
    
    fn create_button_bar(&self) -> Element<'_, Message> {
        // Phonebook/Connect button (serves dual purpose)
        let phonebook_btn = if self.is_connected {
            // When connected, show disconnect button
            button(row![
                svg(svg::Handle::from_memory(DISCONNECT_SVG))
                    .width(Length::Fixed(16.0))
                    .height(Length::Fixed(16.0)),
                text(fl!(crate::LANGUAGE_LOADER, "terminal-hangup"))
                    .size(12)
            ].spacing(3).align_y(Alignment::Center))
            .on_press(Message::Disconnect)
            .padding([4, 6])
            .style(button::danger)
        } else {
            // When disconnected, show phonebook (connect) button
            button(row![
                svg(svg::Handle::from_memory(PHONEBOOK_SVG))
                    .width(Length::Fixed(16.0))
                    .height(Length::Fixed(16.0)),
                text(fl!(crate::LANGUAGE_LOADER, "terminal-dialing_directory"))
                    .size(12)
            ].spacing(3).align_y(Alignment::Center))
            .on_press(Message::ShowDialingDirectory)
            .padding([4, 6])
            .style(button::primary)
        };
        
        // Upload button
        let upload_btn = button(row![
            svg(svg::Handle::from_memory(UPLOAD_SVG))
                .width(Length::Fixed(16.0))
                .height(Length::Fixed(16.0)),
            text(fl!(crate::LANGUAGE_LOADER, "terminal-upload"))
                .size(12)
        ].spacing(3).align_y(Alignment::Center))
        .on_press(Message::Upload)
        .padding([4, 6]);
        
        // Download button
        let download_btn = button(row![
            svg(svg::Handle::from_memory(DOWNLOAD_SVG))
                .width(Length::Fixed(16.0))
                .height(Length::Fixed(16.0)),
            text(fl!(crate::LANGUAGE_LOADER, "terminal-download"))
                .size(12)
        ].spacing(3).align_y(Alignment::Center))
        .on_press(Message::Download)
        .padding([4, 6]);
        
        // Settings dropdown menu
//        let settings_menu = self.create_settings_menu();
        
        container(
            row![
                phonebook_btn,
                container(text(" | ").size(10)).padding([0, 2]),
                upload_btn,
                download_btn,
                container(text(" | ").size(10)).padding([0, 2]),
                //settings_menu,
                Space::new().width(Length::Fill),
            ]
            .spacing(3)
            .align_y(Alignment::Center)
            .padding([3, 6])
        )
        .style(|theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(theme.extended_palette().background.weak.color)),
            border: iced::Border {
                color: theme.extended_palette().background.strong.color,
                width: 0.0,
                radius: 0.0.into(),
            },
            text_color: None,
            shadow: Default::default(),
            snap: false,
        })
        .into()
    }

    /*
    fn create_settings_menu(&self) -> Element<'_, Message> {
        // Create text-based menu items
        let capture_item = if self.is_capturing {
            menu_button(
                text(fl!(crate::LANGUAGE_LOADER, "terminal-stop-capture"))
                    .width(Length::Fill),
                Message::StopCapture
            )
        } else {
            menu_button(
                text(fl!(crate::LANGUAGE_LOADER, "terminal-start-capture"))
                    .width(Length::Fill),
                Message::StartCapture
            )
        };
        
        let settings_item = menu_button(
            text(fl!(crate::LANGUAGE_LOADER, "terminal-settings"))
                .width(Length::Fill),
            Message::ShowSettings
        );
        
        // Separator
        let separator = menu::Quad {
            quad_color: Color::from([0.5; 3]).into(),
            quad_border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            inner_bounds: menu::InnerBounds::Ratio(0.98, 0.2),
            height: Length::Fixed(20.0),
            ..Default::default()
        };
        
        let quit_item = menu_button(
            text(fl!(crate::LANGUAGE_LOADER, "terminal-quit"))
                .width(Length::Fill)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().danger.base.color),
                    ..Default::default()
                }),
            Message::Quit
        );
        
        // Create the dropdown menu
        let menu = menu::Menu::new(menu_items!(
            (capture_item),
            (settings_item),
            (separator),
            (quit_item)
        ))
        .width(180.0)
        .offset(5.0)
        .spacing(5.0);
        
        // Create the settings button that triggers the menu
        let settings_button = button(row![
            svg(svg::Handle::from_memory(SETTINGS_SVG))
                .width(Length::Fixed(20.0))
                .height(Length::Fixed(20.0)),
            text(fl!(crate::LANGUAGE_LOADER, "terminal-menu"))
                .size(14),
            text(" ▼").size(10)  // Dropdown indicator
        ].spacing(4).align_y(Alignment::Center))
        .padding(8)
        .style(|theme: &iced::Theme, status| {
            use iced::widget::button::{Status, Style};
            
            let palette = theme.extended_palette();
            let base = Style {
                text_color: palette.background.base.text,
                border: Border::default().rounded(4.0),
                ..Style::default()
            };
            
            match status {
                Status::Active => base.with_background(Color::TRANSPARENT),
                Status::Hovered => base.with_background(Color::from_rgba(
                    palette.primary.weak.color.r,
                    palette.primary.weak.color.g,
                    palette.primary.weak.color.b,
                    0.2,
                )),
                Status::Pressed => base.with_background(palette.primary.weak.color),
                _ => base,
            }
        })
        .on_press(Message::None);
        
        // Wrap in menu_bar
        menu_bar!(
            (settings_button, menu)
        )
        .draw_path(menu::DrawPath::Backdrop)
        .close_on_item_click(true)
        .style(|theme: &iced::Theme, _status| menu::Style {
            bar_background: Color::TRANSPARENT.into(),
            menu_background: theme.extended_palette().background.weak.color.into(),
            menu_border: Border {
                color: theme.extended_palette().background.strong.color,
                width: 1.0,
                radius: 4.0.into(),
            },
            menu_shadow: iced::Shadow {
                color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                offset: iced::Vector::new(0.0, 2.0),
                blur_radius: 8.0,
            },
            path: theme.extended_palette().background.base.color.into(),
            path_border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
        })
        .into()
    }
*/
    fn create_status_bar(&self) -> Element<'_, Message> {
        let connection_status = if self.is_connected {
            text("● Connected")
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().success.base.color),
                    ..Default::default()
                })
        } else {
            text("○ Disconnected")
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().secondary.weak.color),
                    ..Default::default()
                })
        };
        
        let capture_status = if self.is_capturing {
            text("● REC")
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().danger.base.color),
                    ..Default::default()
                })
        } else {
            text("")
        };
        
        container(
            row![
                connection_status,
                Space::new().width(Length::Fill),
                capture_status,
                text(" | "),
                text("ANSI • 80x25 • 9600 baud").size(12),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .padding([4, 12])
        )
        .style(|theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(theme.extended_palette().background.weak.color)),
            border: iced::Border {
                color: theme.extended_palette().background.strong.color,
                width: 1.0,
                radius: 0.0.into(),
            },
            text_color: Some(theme.extended_palette().secondary.base.color),
            shadow: Default::default(),
            snap: false,
        })
        .into()
    }
    
    // Helper methods for terminal operations
    pub fn connect(&mut self) {
        self.is_connected = true;
    }
    
    pub fn disconnect(&mut self) {
        self.is_connected = false;
    }
    
    pub fn toggle_capture(&mut self) {
        self.is_capturing = !self.is_capturing;
    }
}

// Helper function to create menu buttons
fn menu_button<'a>(
    content: impl Into<Element<'a, Message>>,
    msg: Message,
) -> button::Button<'a, Message> {
    button(content)
        .padding([6, 12])
        .width(Length::Fill)
        .style(|theme: &iced::Theme, status| {
            use iced::widget::button::{Status, Style};
            
            let palette = theme.extended_palette();
            let base = Style {
                text_color: palette.background.base.text,
                border: Border::default().rounded(4.0),
                ..Style::default()
            };
            
            match status {
                Status::Active => base.with_background(Color::TRANSPARENT),
                Status::Hovered => base.with_background(Color::from_rgba(
                    palette.primary.weak.color.r,
                    palette.primary.weak.color.g,
                    palette.primary.weak.color.b,
                    0.3,
                )),
                Status::Pressed => base.with_background(palette.primary.weak.color),
                _ => base,
            }
        })
        .on_press(msg)
}

// Update Message enum to include new variants
#[derive(Debug, Clone)]
pub enum TerminalMessage {
    Disconnect,
    ShowDialingDirectory,
    StartCapture,
    StopCapture,
    Upload,
    Download,
    ShowSettings,
    Quit,
    None,
}