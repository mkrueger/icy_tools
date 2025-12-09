//! Font Size Dialog for BitFont Editor
//!
//! Allows changing the width and height of the font.

use iced::{
    Alignment, Element, Length,
    widget::{Space, column, container, row, text, text_input},
};
use icy_engine_gui::ButtonType;
use icy_engine_gui::settings::effect_box;
use icy_engine_gui::ui::{
    DIALOG_SPACING, DIALOG_WIDTH_SMALL, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL, button_row, dialog_area, dialog_title, left_label_small, modal_container,
    modal_overlay, primary_button, secondary_button, separator,
};

use crate::fl;

/// Messages for the Font Size dialog
#[derive(Debug, Clone)]
pub enum FontSizeDialogMessage {
    /// Width input changed
    SetWidth(String),
    /// Height input changed
    SetHeight(String),
    /// Apply the new size
    Apply,
    /// Cancel the dialog
    Cancel,
}

/// State for the Font Size dialog
#[derive(Debug, Clone)]
pub struct FontSizeDialog {
    /// Current width input
    pub width: String,
    /// Current height input
    pub height: String,
}

impl FontSizeDialog {
    /// Create a new Font Size dialog with the current font dimensions
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            width: width.to_string(),
            height: height.to_string(),
        }
    }

    /// Parse the current width value (1-8 for old-style bit fonts)
    pub fn parsed_width(&self) -> Option<i32> {
        self.width.parse::<i32>().ok().filter(|&w| w >= 1 && w <= 8)
    }

    /// Parse the current height value (1-32 for old-style bit fonts)
    pub fn parsed_height(&self) -> Option<i32> {
        self.height.parse::<i32>().ok().filter(|&h| h >= 1 && h <= 32)
    }

    /// Check if the input is valid
    pub fn is_valid(&self) -> bool {
        self.parsed_width().is_some() && self.parsed_height().is_some()
    }

    /// Update the dialog state
    pub fn update(&mut self, message: FontSizeDialogMessage) -> Option<FontSizeDialogResult> {
        match message {
            FontSizeDialogMessage::SetWidth(w) => {
                self.width = w;
                None
            }
            FontSizeDialogMessage::SetHeight(h) => {
                self.height = h;
                None
            }
            FontSizeDialogMessage::Apply => {
                if let (Some(w), Some(h)) = (self.parsed_width(), self.parsed_height()) {
                    Some(FontSizeDialogResult::Apply(w, h))
                } else {
                    None
                }
            }
            FontSizeDialogMessage::Cancel => Some(FontSizeDialogResult::Cancel),
        }
    }

    /// Build the dialog view
    pub fn view<'a, Message: 'a + Clone + 'static>(
        &'a self,
        background: Element<'a, Message>,
        on_message: impl Fn(FontSizeDialogMessage) -> Message + 'a + Clone,
    ) -> Element<'a, Message> {
        let title = dialog_title(fl!("menu-set-font-size").trim_end_matches('…').to_string());

        // Width input
        let width_valid = self.parsed_width().is_some();
        let width_input = text_input("1-8", &self.width)
            .on_input({
                let on_message = on_message.clone();
                move |s| on_message(FontSizeDialogMessage::SetWidth(s))
            })
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(80.0));

        let width_error = if !width_valid && !self.width.is_empty() {
            text("1-8").size(TEXT_SIZE_SMALL).style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().danger.base.color),
            })
        } else {
            text("").size(TEXT_SIZE_SMALL)
        };

        let width_row = row![left_label_small(fl!("font-size-width")), width_input, Space::new().width(4.0), width_error,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Height input
        let height_valid = self.parsed_height().is_some();
        let height_input = text_input("1-32", &self.height)
            .on_input({
                let on_message = on_message.clone();
                move |s| on_message(FontSizeDialogMessage::SetHeight(s))
            })
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(80.0));

        let height_error = if !height_valid && !self.height.is_empty() {
            text("1-32").size(TEXT_SIZE_SMALL).style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().danger.base.color),
            })
        } else {
            text("").size(TEXT_SIZE_SMALL)
        };

        let height_row = row![left_label_small(fl!("font-size-height")), height_input, Space::new().width(4.0), height_error,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Content wrapped in effect_box
        let content_column = column![width_row, Space::new().height(DIALOG_SPACING), height_row,].spacing(0);

        let content_box = effect_box(content_column.into());

        let can_apply = self.is_valid();

        let buttons = button_row(vec![
            secondary_button(format!("{}", ButtonType::Cancel), Some(on_message(FontSizeDialogMessage::Cancel))).into(),
            primary_button(format!("{}", ButtonType::Ok), can_apply.then(|| on_message(FontSizeDialogMessage::Apply))).into(),
        ]);

        let dialog_content = dialog_area(column![title, Space::new().height(DIALOG_SPACING), content_box].into());

        let button_area = dialog_area(buttons.into());

        let modal = modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area,].into(),
            DIALOG_WIDTH_SMALL,
        );

        let modal_element: Element<'a, Message> = container(modal)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();

        modal_overlay(background, modal_element)
    }
}

/// Result of the Font Size dialog
#[derive(Debug, Clone)]
pub enum FontSizeDialogResult {
    /// Apply new dimensions (width, height)
    Apply(i32, i32),
    /// Cancel the dialog
    Cancel,
}
