//! Font Dialogs for CharFont (TDF) Editor
//!
//! Provides dialogs for:
//! - Adding a new font (selecting type, name, spacing)
//! - Editing font settings (name, spacing)

use std::fmt;

use iced::{
    widget::{column, container, pick_list, row, text, text_input, Space},
    Alignment, Element, Length,
};
use icy_engine_gui::settings::effect_box;
use icy_engine_gui::ui::{
    button_row, dialog_area, dialog_title, left_label_small, modal_container, primary_button, secondary_button, separator, Dialog, DialogAction,
    DIALOG_SPACING, DIALOG_WIDTH_SMALL, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL,
};
use icy_engine_gui::ButtonType;

use super::CharFontEditorMessage;
use crate::fl;
use crate::ui::Message;
use icy_engine_edit::charset::TdfFontType;

/// Wrapper for TdfFontType that implements Display for use in pick_list
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontTypeOption(pub TdfFontType);

impl fmt::Display for FontTypeOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            TdfFontType::Color => write!(f, "{}", fl!("tdf-editor-font_type_color")),
            TdfFontType::Block => write!(f, "{}", fl!("tdf-editor-font_type_block")),
            TdfFontType::Outline => write!(f, "{}", fl!("tdf-editor-font_type_outline")),
        }
    }
}

impl From<TdfFontType> for FontTypeOption {
    fn from(t: TdfFontType) -> Self {
        FontTypeOption(t)
    }
}

impl From<FontTypeOption> for TdfFontType {
    fn from(t: FontTypeOption) -> Self {
        t.0
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Add Font Dialog
// ═══════════════════════════════════════════════════════════════════════════

/// Helper to wrap AddFontDialogMessage in Message
fn add_msg(m: AddFontDialogMessage) -> Message {
    Message::CharFontEditor(CharFontEditorMessage::AddFontDialog(m))
}

/// Messages for the Add Font dialog
#[derive(Debug, Clone)]
pub enum AddFontDialogMessage {
    /// Font type changed
    SetFontType(TdfFontType),
    /// Name input changed
    SetName(String),
    /// Spacing input changed
    SetSpacing(String),
    /// Apply (create the font)
    Apply,
    /// Cancel the dialog
    Cancel,
}

/// State for the Add Font dialog
#[derive(Debug, Clone)]
pub struct AddFontDialog {
    /// Selected font type
    pub font_type: TdfFontType,
    /// Font name
    pub name: String,
    /// Spacing value as string
    pub spacing: String,
}

impl Default for AddFontDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl AddFontDialog {
    /// Create a new Add Font dialog with default values
    pub fn new() -> Self {
        Self {
            font_type: TdfFontType::Color,
            name: "New Font".to_string(),
            spacing: "0".to_string(),
        }
    }

    /// Parse the spacing value
    pub fn parsed_spacing(&self) -> Option<i32> {
        self.spacing.parse::<i32>().ok().filter(|&s| s >= -10 && s <= 10)
    }

    /// Check if the input is valid
    pub fn is_valid(&self) -> bool {
        !self.name.trim().is_empty() && self.parsed_spacing().is_some()
    }
}

impl Dialog<Message> for AddFontDialog {
    fn view(&self) -> Element<'_, Message> {
        let title = dialog_title(fl!("tdf-dialog-add-font-title"));

        // Font type picker - use FontTypeOption wrapper for Display
        let font_types = vec![
            FontTypeOption(TdfFontType::Color),
            FontTypeOption(TdfFontType::Block),
            FontTypeOption(TdfFontType::Outline),
        ];
        let selected = FontTypeOption(self.font_type);
        let type_picker =
            pick_list(font_types, Some(selected), |t: FontTypeOption| add_msg(AddFontDialogMessage::SetFontType(t.0))).width(Length::Fixed(120.0));

        let type_row = row![left_label_small(fl!("tdf-dialog-font-type")), type_picker,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Name input
        let name_input = text_input("", &self.name)
            .on_input(|s| add_msg(AddFontDialogMessage::SetName(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(150.0));

        let name_row = row![left_label_small(fl!("tdf-dialog-font-name")), name_input,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Spacing input
        let spacing_valid = self.parsed_spacing().is_some();
        let spacing_input = text_input("0-40", &self.spacing)
            .on_input(|s| add_msg(AddFontDialogMessage::SetSpacing(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(80.0));

        let spacing_error = if !spacing_valid && !self.spacing.is_empty() {
            text("-10 to 10").size(TEXT_SIZE_SMALL).style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.destructive.base),
            })
        } else {
            text("").size(TEXT_SIZE_SMALL)
        };

        let spacing_row = row![
            left_label_small(fl!("tdf-dialog-spacing")),
            spacing_input,
            Space::new().width(4.0),
            spacing_error,
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        // Content wrapped in effect_box
        let content_column = column![
            type_row,
            Space::new().height(DIALOG_SPACING),
            name_row,
            Space::new().height(DIALOG_SPACING),
            spacing_row,
        ]
        .spacing(0);

        let content_box = effect_box(content_column.into());

        let can_apply = self.is_valid();

        let buttons = button_row(vec![
            secondary_button(format!("{}", ButtonType::Cancel), Some(add_msg(AddFontDialogMessage::Cancel))).into(),
            primary_button(format!("{}", ButtonType::Ok), can_apply.then(|| add_msg(AddFontDialogMessage::Apply))).into(),
        ]);

        let dialog_content = dialog_area(column![title, Space::new().height(DIALOG_SPACING), content_box].into());

        let button_area = dialog_area(buttons.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area,].into(),
            DIALOG_WIDTH_SMALL,
        )
        .into()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        let Message::CharFontEditor(CharFontEditorMessage::AddFontDialog(msg)) = message else {
            return None;
        };
        match msg {
            AddFontDialogMessage::SetFontType(t) => {
                self.font_type = *t;
                Some(DialogAction::None)
            }
            AddFontDialogMessage::SetName(n) => {
                self.name = n.clone();
                Some(DialogAction::None)
            }
            AddFontDialogMessage::SetSpacing(s) => {
                self.spacing = s.clone();
                Some(DialogAction::None)
            }
            AddFontDialogMessage::Apply => {
                if let Some(spacing) = self.parsed_spacing() {
                    Some(DialogAction::CloseWith(Message::CharFontEditor(CharFontEditorMessage::AddFontApply(
                        self.font_type,
                        self.name.clone(),
                        spacing,
                    ))))
                } else {
                    Some(DialogAction::None)
                }
            }
            AddFontDialogMessage::Cancel => Some(DialogAction::Close),
        }
    }

    fn request_cancel(&mut self) -> DialogAction<Message> {
        DialogAction::Close
    }

    fn request_confirm(&mut self) -> DialogAction<Message> {
        if let Some(spacing) = self.parsed_spacing() {
            DialogAction::CloseWith(Message::CharFontEditor(CharFontEditorMessage::AddFontApply(
                self.font_type,
                self.name.clone(),
                spacing,
            )))
        } else {
            DialogAction::None
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Edit Font Settings Dialog
// ═══════════════════════════════════════════════════════════════════════════

/// Helper to wrap EditFontSettingsDialogMessage in Message
fn edit_msg(m: EditFontSettingsDialogMessage) -> Message {
    Message::CharFontEditor(CharFontEditorMessage::EditFontSettingsDialog(m))
}

/// Messages for the Edit Font Settings dialog
#[derive(Debug, Clone)]
pub enum EditFontSettingsDialogMessage {
    /// Name input changed
    SetName(String),
    /// Spacing input changed
    SetSpacing(String),
    /// Apply the changes
    Apply,
    /// Cancel the dialog
    Cancel,
}

/// State for the Edit Font Settings dialog
#[derive(Debug, Clone)]
pub struct EditFontSettingsDialog {
    /// Font name
    pub name: String,
    /// Spacing value as string
    pub spacing: String,
}

impl EditFontSettingsDialog {
    /// Create a new Edit Font Settings dialog with current font values
    pub fn new(name: String, spacing: i32) -> Self {
        Self {
            name,
            spacing: spacing.to_string(),
        }
    }

    /// Parse the spacing value
    pub fn parsed_spacing(&self) -> Option<i32> {
        self.spacing.parse::<i32>().ok().filter(|&s| s >= -10 && s <= 10)
    }

    /// Check if the input is valid
    pub fn is_valid(&self) -> bool {
        !self.name.trim().is_empty() && self.parsed_spacing().is_some()
    }
}

impl Dialog<Message> for EditFontSettingsDialog {
    fn view(&self) -> Element<'_, Message> {
        let title = dialog_title(fl!("tdf-dialog-edit-settings-title"));

        // Name input
        let name_input = text_input("", &self.name)
            .on_input(|s| edit_msg(EditFontSettingsDialogMessage::SetName(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(150.0));

        let name_row = row![left_label_small(fl!("tdf-dialog-font-name")), name_input,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Spacing input
        let spacing_valid = self.parsed_spacing().is_some();
        let spacing_input = text_input("0-40", &self.spacing)
            .on_input(|s| edit_msg(EditFontSettingsDialogMessage::SetSpacing(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(80.0));

        let spacing_error = if !spacing_valid && !self.spacing.is_empty() {
            text("-10 to 10").size(TEXT_SIZE_SMALL).style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.destructive.base),
            })
        } else {
            text("").size(TEXT_SIZE_SMALL)
        };

        let spacing_row = row![
            left_label_small(fl!("tdf-dialog-spacing")),
            spacing_input,
            Space::new().width(4.0),
            spacing_error,
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        // Content wrapped in effect_box
        let content_column = column![name_row, Space::new().height(DIALOG_SPACING), spacing_row,].spacing(0);

        let content_box = effect_box(content_column.into());

        let can_apply = self.is_valid();

        let buttons = button_row(vec![
            secondary_button(format!("{}", ButtonType::Cancel), Some(edit_msg(EditFontSettingsDialogMessage::Cancel))).into(),
            primary_button(format!("{}", ButtonType::Ok), can_apply.then(|| edit_msg(EditFontSettingsDialogMessage::Apply))).into(),
        ]);

        let dialog_content = dialog_area(column![title, Space::new().height(DIALOG_SPACING), content_box].into());

        let button_area = dialog_area(buttons.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area,].into(),
            DIALOG_WIDTH_SMALL,
        )
        .into()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        let Message::CharFontEditor(CharFontEditorMessage::EditFontSettingsDialog(msg)) = message else {
            return None;
        };
        match msg {
            EditFontSettingsDialogMessage::SetName(n) => {
                self.name = n.clone();
                Some(DialogAction::None)
            }
            EditFontSettingsDialogMessage::SetSpacing(s) => {
                self.spacing = s.clone();
                Some(DialogAction::None)
            }
            EditFontSettingsDialogMessage::Apply => {
                if let Some(spacing) = self.parsed_spacing() {
                    Some(DialogAction::CloseWith(Message::CharFontEditor(CharFontEditorMessage::EditFontSettingsApply(
                        self.name.clone(),
                        spacing,
                    ))))
                } else {
                    Some(DialogAction::None)
                }
            }
            EditFontSettingsDialogMessage::Cancel => Some(DialogAction::Close),
        }
    }

    fn request_cancel(&mut self) -> DialogAction<Message> {
        DialogAction::Close
    }

    fn request_confirm(&mut self) -> DialogAction<Message> {
        if let Some(spacing) = self.parsed_spacing() {
            DialogAction::CloseWith(Message::CharFontEditor(CharFontEditorMessage::EditFontSettingsApply(
                self.name.clone(),
                spacing,
            )))
        } else {
            DialogAction::None
        }
    }
}
