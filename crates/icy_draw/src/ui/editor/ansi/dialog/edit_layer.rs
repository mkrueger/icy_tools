//! Edit Layer Dialog
//!
//! Combined dialog for editing layer properties and size.
//! Replaces the separate EditLayerDialog and ResizeLayerDialog from the egui version.

use iced::{
    Alignment, Element, Length,
    widget::{Space, checkbox, column, container, pick_list, row, text, text_input},
};
use icy_engine::{Mode, Properties, Size};
use icy_engine_gui::ButtonType;
use icy_engine_gui::settings::effect_box;
use icy_engine_gui::ui::{
    DIALOG_SPACING, DIALOG_WIDTH_MEDIUM, Dialog, DialogAction, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL, button_row, dialog_area, dialog_title, left_label_small,
    modal_container, primary_button, secondary_button, separator,
};

use crate::fl;
use crate::ui::Message;
use crate::ui::editor::ansi::AnsiEditorMessage;

/// Helper function to wrap edit layer dialog messages
fn msg(m: EditLayerDialogMessage) -> Message {
    Message::AnsiEditor(AnsiEditorMessage::EditLayerDialog(m))
}

/// Wrapper type for Mode to implement Display (orphan rule workaround)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeOption(pub Mode);

impl std::fmt::Display for ModeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Mode::Normal => write!(f, "Normal"),
            Mode::Chars => write!(f, "Chars only"),
            Mode::Attributes => write!(f, "Attributes only"),
        }
    }
}

/// Messages for the Edit Layer dialog
#[derive(Debug, Clone)]
pub enum EditLayerDialogMessage {
    /// Name input changed
    SetName(String),
    /// Width input changed
    SetWidth(String),
    /// Height input changed
    SetHeight(String),
    /// X offset input changed
    SetOffsetX(String),
    /// Y offset input changed
    SetOffsetY(String),
    /// Visibility checkbox changed
    SetVisible(bool),
    /// Locked checkbox changed
    SetLocked(bool),
    /// Position locked checkbox changed
    SetPositionLocked(bool),
    /// Alpha channel checkbox changed
    SetHasAlpha(bool),
    /// Alpha locked checkbox changed
    SetAlphaLocked(bool),
    /// Mode selection changed
    SetMode(Mode),
    /// Apply the changes
    Apply,
    /// Cancel the dialog
    Cancel,
}

/// Result of the Edit Layer dialog
#[derive(Debug, Clone)]
pub struct EditLayerResult {
    /// Layer index that was edited
    pub layer_index: usize,
    /// Updated properties
    pub properties: Properties,
    /// New size (if changed)
    pub new_size: Option<Size>,
}

/// State for the Edit Layer dialog
#[derive(Debug, Clone)]
pub struct EditLayerDialog {
    /// Layer index being edited
    layer_index: usize,
    /// Original size (for comparison)
    original_size: Size,
    /// Current properties being edited
    properties: Properties,
    /// Width input string
    width: String,
    /// Height input string
    height: String,
    /// X offset input string
    offset_x: String,
    /// Y offset input string
    offset_y: String,
}

impl EditLayerDialog {
    /// Create a new Edit Layer dialog
    pub fn new(layer_index: usize, properties: Properties, size: Size) -> Self {
        Self {
            layer_index,
            original_size: size,
            offset_x: properties.offset.x.to_string(),
            offset_y: properties.offset.y.to_string(),
            properties,
            width: size.width.to_string(),
            height: size.height.to_string(),
        }
    }

    /// Parse the current width value
    pub fn parsed_width(&self) -> Option<i32> {
        self.width.parse::<i32>().ok().filter(|&w| w >= 1)
    }

    /// Parse the current height value
    pub fn parsed_height(&self) -> Option<i32> {
        self.height.parse::<i32>().ok().filter(|&h| h >= 1)
    }

    /// Parse the current X offset value
    pub fn parsed_offset_x(&self) -> Option<i32> {
        self.offset_x.parse::<i32>().ok()
    }

    /// Parse the current Y offset value
    pub fn parsed_offset_y(&self) -> Option<i32> {
        self.offset_y.parse::<i32>().ok()
    }

    /// Check if all inputs are valid
    pub fn is_valid(&self) -> bool {
        self.parsed_width().is_some() && self.parsed_height().is_some() && self.parsed_offset_x().is_some() && self.parsed_offset_y().is_some()
    }

    /// Get the result if valid
    pub fn result(&self) -> Option<EditLayerResult> {
        let offset_x = self.parsed_offset_x()?;
        let offset_y = self.parsed_offset_y()?;
        let width = self.parsed_width()?;
        let height = self.parsed_height()?;

        let mut properties = self.properties.clone();
        properties.offset.x = offset_x;
        properties.offset.y = offset_y;

        let new_size = if width != self.original_size.width || height != self.original_size.height {
            Some(Size::new(width, height))
        } else {
            None
        };

        Some(EditLayerResult {
            layer_index: self.layer_index,
            properties,
            new_size,
        })
    }
}

impl Dialog<Message> for EditLayerDialog {
    fn view(&self) -> Element<'_, Message> {
        let title = dialog_title(fl!("edit-layer-dialog-title"));

        // Name input
        let name_input = text_input("", &self.properties.title)
            .on_input(|s| msg(EditLayerDialogMessage::SetName(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let name_row = row![left_label_small(fl!("edit-layer-dialog-name-label")), name_input]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Size section - separate rows for width and height
        let width_valid = self.parsed_width().is_some();
        let width_input = text_input("", &self.width)
            .on_input(|s| msg(EditLayerDialogMessage::SetWidth(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(80.0));

        let width_error = if !width_valid && !self.width.is_empty() {
            text("≥1").size(TEXT_SIZE_SMALL).style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().danger.base.color),
            })
        } else {
            text("").size(TEXT_SIZE_SMALL)
        };

        let width_row = row![left_label_small(fl!("edit-canvas-size-width-label")), width_input, width_error,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        let height_valid = self.parsed_height().is_some();
        let height_input = text_input("", &self.height)
            .on_input(|s| msg(EditLayerDialogMessage::SetHeight(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(80.0));

        let height_error = if !height_valid && !self.height.is_empty() {
            text("≥1").size(TEXT_SIZE_SMALL).style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().danger.base.color),
            })
        } else {
            text("").size(TEXT_SIZE_SMALL)
        };

        let height_row = row![left_label_small(fl!("edit-canvas-size-height-label")), height_input, height_error,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Offset section - separate rows for X and Y
        let offset_x_input = text_input("", &self.offset_x)
            .on_input(|s| msg(EditLayerDialogMessage::SetOffsetX(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(80.0));

        let offset_x_row = row![left_label_small(fl!("edit-layer-dialog-is-x-offset-label")), offset_x_input,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        let offset_y_input = text_input("", &self.offset_y)
            .on_input(|s| msg(EditLayerDialogMessage::SetOffsetY(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(80.0));

        let offset_y_row = row![left_label_small(fl!("edit-layer-dialog-is-y-offset-label")), offset_y_input,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Checkboxes - using row with labels and checkbox (no label parameter)
        let visible_checkbox = row![
            left_label_small(fl!("edit-layer-dialog-is-visible-checkbox")),
            checkbox(self.properties.is_visible)
                .on_toggle(|v| msg(EditLayerDialogMessage::SetVisible(v)))
                .size(16)
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let locked_checkbox = row![
            left_label_small(fl!("edit-layer-dialog-is-edit-locked-checkbox")),
            checkbox(self.properties.is_locked)
                .on_toggle(|v| msg(EditLayerDialogMessage::SetLocked(v)))
                .size(16)
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let pos_locked_checkbox = row![
            left_label_small(fl!("edit-layer-dialog-is-position-locked-checkbox")),
            checkbox(self.properties.is_position_locked)
                .on_toggle(|v| msg(EditLayerDialogMessage::SetPositionLocked(v)))
                .size(16)
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let alpha_checkbox = row![
            left_label_small(fl!("edit-layer-dialog-has-alpha-checkbox")),
            checkbox(self.properties.has_alpha_channel)
                .on_toggle(|v| msg(EditLayerDialogMessage::SetHasAlpha(v)))
                .size(16)
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let alpha_locked_checkbox = row![
            left_label_small(fl!("edit-layer-dialog-is-alpha-locked-checkbox")),
            checkbox(self.properties.is_alpha_channel_locked)
                .on_toggle(|v| msg(EditLayerDialogMessage::SetAlphaLocked(v)))
                .size(16)
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        // Mode picker with wrapper type
        let mode_options = vec![ModeOption(Mode::Normal), ModeOption(Mode::Chars), ModeOption(Mode::Attributes)];
        let mode_picker = pick_list(mode_options, Some(ModeOption(self.properties.mode)), |m| {
            msg(EditLayerDialogMessage::SetMode(m.0))
        })
        .width(Length::Fixed(150.0));

        let mode_row = row![left_label_small("Mode:".to_string()), mode_picker]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Build checkbox columns
        let checkbox_col1 = column![visible_checkbox, locked_checkbox, pos_locked_checkbox].spacing(8);

        let checkbox_col2 = if self.properties.has_alpha_channel {
            column![alpha_checkbox, alpha_locked_checkbox].spacing(8)
        } else {
            column![alpha_checkbox].spacing(8)
        };

        let checkboxes_row = row![checkbox_col1, Space::new().width(24.0), checkbox_col2].spacing(DIALOG_SPACING);

        // Content wrapped in effect_box
        let content_column = column![
            name_row,
            Space::new().height(DIALOG_SPACING),
            width_row,
            height_row,
            Space::new().height(DIALOG_SPACING),
            offset_x_row,
            offset_y_row,
            Space::new().height(DIALOG_SPACING * 2.0),
            checkboxes_row,
            Space::new().height(DIALOG_SPACING * 2.0),
            mode_row,
        ]
        .spacing(DIALOG_SPACING);

        let content_box = effect_box(content_column.into());

        let can_apply = self.is_valid();

        let buttons = button_row(vec![
            secondary_button(
                format!("{}", ButtonType::Cancel),
                Some(msg(EditLayerDialogMessage::Cancel)),
            )
            .into(),
            primary_button(
                format!("{}", ButtonType::Ok),
                can_apply.then(|| msg(EditLayerDialogMessage::Apply)),
            )
            .into(),
        ]);

        let dialog_content = dialog_area(column![title, Space::new().height(DIALOG_SPACING), content_box].into());

        let button_area = dialog_area(buttons.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area].into(),
            DIALOG_WIDTH_MEDIUM,
        )
        .into()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        let Message::AnsiEditor(AnsiEditorMessage::EditLayerDialog(dialog_msg)) = message else {
            return None;
        };
        match dialog_msg {
            EditLayerDialogMessage::SetName(name) => {
                self.properties.title = name.clone();
                Some(DialogAction::None)
            }
            EditLayerDialogMessage::SetWidth(w) => {
                self.width = w.clone();
                Some(DialogAction::None)
            }
            EditLayerDialogMessage::SetHeight(h) => {
                self.height = h.clone();
                Some(DialogAction::None)
            }
            EditLayerDialogMessage::SetOffsetX(x) => {
                self.offset_x = x.clone();
                Some(DialogAction::None)
            }
            EditLayerDialogMessage::SetOffsetY(y) => {
                self.offset_y = y.clone();
                Some(DialogAction::None)
            }
            EditLayerDialogMessage::SetVisible(v) => {
                self.properties.is_visible = *v;
                Some(DialogAction::None)
            }
            EditLayerDialogMessage::SetLocked(v) => {
                self.properties.is_locked = *v;
                Some(DialogAction::None)
            }
            EditLayerDialogMessage::SetPositionLocked(v) => {
                self.properties.is_position_locked = *v;
                Some(DialogAction::None)
            }
            EditLayerDialogMessage::SetHasAlpha(v) => {
                self.properties.has_alpha_channel = *v;
                // If disabling alpha, also disable alpha locked
                if !*v {
                    self.properties.is_alpha_channel_locked = false;
                }
                Some(DialogAction::None)
            }
            EditLayerDialogMessage::SetAlphaLocked(v) => {
                self.properties.is_alpha_channel_locked = *v;
                Some(DialogAction::None)
            }
            EditLayerDialogMessage::SetMode(mode) => {
                self.properties.mode = *mode;
                Some(DialogAction::None)
            }
            EditLayerDialogMessage::Apply => {
                if let Some(result) = self.result() {
                    Some(DialogAction::CloseWith(Message::AnsiEditor(AnsiEditorMessage::ApplyEditLayer(result))))
                } else {
                    Some(DialogAction::None)
                }
            }
            EditLayerDialogMessage::Cancel => Some(DialogAction::Close),
        }
    }

    fn request_cancel(&mut self) -> DialogAction<Message> {
        DialogAction::Close
    }

    fn request_confirm(&mut self) -> DialogAction<Message> {
        if self.is_valid() {
            if let Some(result) = self.result() {
                return DialogAction::CloseWith(Message::AnsiEditor(AnsiEditorMessage::ApplyEditLayer(result)));
            }
        }
        DialogAction::None
    }
}
