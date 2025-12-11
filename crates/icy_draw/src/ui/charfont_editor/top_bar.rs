//! Top bar component for CharFont editor
//!
//! Shows font name, spacing, and font type info

use iced::{
    Element, Length, Task,
    widget::{Space, column, container, row, text, text_input},
};

use icy_engine_edit::charset::{FontType, TdfFont, TdfFontExt};

/// Messages from the top bar
#[derive(Clone, Debug)]
pub enum TopBarMessage {
    /// Font name changed
    FontNameChanged(String),
    /// Spacing changed
    SpacingChanged(i32),
}

/// Top bar state
pub struct TopBar {
    /// Font name input value
    font_name: String,
    /// Spacing value
    spacing: i32,
}

impl Default for TopBar {
    fn default() -> Self {
        Self::new()
    }
}

impl TopBar {
    pub fn new() -> Self {
        Self {
            font_name: String::new(),
            spacing: 1,
        }
    }

    /// Update the top bar state
    pub fn update(&mut self, message: TopBarMessage) -> Task<TopBarMessage> {
        match message {
            TopBarMessage::FontNameChanged(name) => {
                self.font_name = name;
            }
            TopBarMessage::SpacingChanged(spacing) => {
                self.spacing = spacing;
            }
        }
        Task::none()
    }

    /// Render the top bar
    pub fn view(&self, font: &TdfFont) -> Element<'_, TopBarMessage> {
        let font_type: FontType = font.font_type.into();
        let font_type_str = match font_type {
            FontType::Outline => crate::fl!("tdf-editor-font_type_outline"),
            FontType::Block => crate::fl!("tdf-editor-font_type_block"),
            FontType::Color => crate::fl!("tdf-editor-font_type_color"),
        };

        let name_label = text(crate::fl!("tdf-editor-font_name_label")).size(12);
        let name_input = text_input("", &font.name)
            .on_input(TopBarMessage::FontNameChanged)
            .width(Length::Fixed(200.0))
            .size(12);

        let type_label = text(crate::fl!("tdf-editor-font_type_label")).size(12);
        let type_value = text(font_type_str).size(12);

        let spacing_label = text(crate::fl!("tdf-editor-spacing_label")).size(12);
        let spacing_value = text(format!("{}", font.get_spacing())).size(12);

        let row1 = row![name_label, name_input, Space::new().width(Length::Fixed(16.0)), type_label, type_value,]
            .spacing(8)
            .align_y(iced::Alignment::Center);

        let row2 = row![spacing_label, spacing_value,].spacing(8).align_y(iced::Alignment::Center);

        container(column![row1, row2].spacing(4))
            .padding(8)
            .height(Length::Fixed(60.0))
            .width(Length::Fill)
            .into()
    }
}
