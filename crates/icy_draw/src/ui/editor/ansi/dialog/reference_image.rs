//! Reference Image Dialog
//!
//! Dialog for setting a reference image with file path and alpha settings.

use std::path::PathBuf;

use iced::{
    widget::{button, column, container, row, slider, text, text_input, Space},
    Alignment, Element, Length, Task,
};
use icy_engine_gui::settings::effect_box;
use icy_engine_gui::ui::{
    dialog_area, dialog_title, modal_container, primary_button, secondary_button, separator, Dialog, DialogAction, DIALOG_SPACING, DIALOG_WIDTH_MEDIUM,
    TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL,
};
use icy_engine_gui::ButtonType;

use super::super::{AnsiEditorCoreMessage, AnsiEditorMessage};
use crate::fl;
use crate::ui::Message;

// ============================================================================
// Dialog Messages
// ============================================================================

/// Messages for the Reference Image dialog
#[derive(Debug, Clone)]
pub enum ReferenceImageDialogMessage {
    /// Set the file path
    SetPath(String),
    /// Browse for a file
    Browse,
    /// File selected from browse dialog
    FileSelected(PathBuf),
    /// Set the alpha value (0.0 - 1.0)
    SetAlpha(f32),
    /// Clear the reference image
    Clear,
    /// Apply settings
    Apply,
    /// Cancel dialog
    Cancel,
}

/// Helper to wrap AnsiEditorMessage in Message
fn msg(m: AnsiEditorMessage) -> Message {
    Message::AnsiEditor(m)
}

// ============================================================================
// Dialog State
// ============================================================================

/// State for the Reference Image dialog
#[derive(Debug, Clone)]
pub struct ReferenceImageDialog {
    /// Path to the reference image
    path: String,
    /// Alpha value (0.0 - 1.0)
    alpha: f32,
}

impl Default for ReferenceImageDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl ReferenceImageDialog {
    /// Create a new Reference Image dialog
    pub fn new() -> Self {
        Self {
            path: String::new(),
            alpha: 0.5,
        }
    }

    /// Check if the path is valid (non-empty and file exists)
    fn is_valid(&self) -> bool {
        if self.path.is_empty() {
            return false;
        }
        let path = PathBuf::from(&self.path);
        path.exists() && path.is_file()
    }

    /// Get the parsed path
    fn parsed_path(&self) -> Option<PathBuf> {
        if self.path.is_empty() {
            return None;
        }
        let path = PathBuf::from(&self.path);
        if path.exists() && path.is_file() {
            Some(path)
        } else {
            None
        }
    }
}

impl Dialog<Message> for ReferenceImageDialog {
    fn view(&self) -> Element<'_, Message> {
        let title = dialog_title(fl!("reference-image-dialog-title"));

        let label_width = Length::Fixed(80.0);

        // ═══════════════════════════════════════════════════════════════════════
        // Path: [Input] [Browse]
        // ═══════════════════════════════════════════════════════════════════════
        let path_valid = self.path.is_empty() || self.is_valid();
        let path_input = text_input(&fl!("reference-image-path-placeholder"), &self.path)
            .on_input(|s| msg(AnsiEditorMessage::ReferenceImageDialog(ReferenceImageDialogMessage::SetPath(s))))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill)
            .style(move |theme: &iced::Theme, status| {
                let mut style = iced::widget::text_input::default(theme, status);
                if !path_valid {
                    style.border.color = theme.destructive.base;
                }
                style
            });

        let browse_button = button(text(fl!("reference-image-browse")).size(TEXT_SIZE_NORMAL))
            .on_press(msg(AnsiEditorMessage::ReferenceImageDialog(ReferenceImageDialogMessage::Browse)))
            .padding([4, 12]);

        let path_row = row![
            container(text(fl!("reference-image-path")).size(TEXT_SIZE_NORMAL)).width(label_width),
            path_input,
            browse_button,
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        // ═══════════════════════════════════════════════════════════════════════
        // Alpha: [Slider] [Value]
        // ═══════════════════════════════════════════════════════════════════════
        let alpha_slider = slider(0.0..=1.0, self.alpha, |v| {
            msg(AnsiEditorMessage::ReferenceImageDialog(ReferenceImageDialogMessage::SetAlpha(v)))
        })
        .step(0.01)
        .width(Length::Fixed(200.0));

        let alpha_percent = format!("{:.0}%", self.alpha * 100.0);
        let alpha_label = text(alpha_percent).size(TEXT_SIZE_SMALL).width(Length::Fixed(40.0));

        let alpha_row = row![
            container(text(fl!("reference-image-alpha")).size(TEXT_SIZE_NORMAL)).width(label_width),
            alpha_slider,
            alpha_label,
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        // ═══════════════════════════════════════════════════════════════════════
        // Info text
        // ═══════════════════════════════════════════════════════════════════════
        let info_text = text(fl!("reference-image-info"))
            .size(TEXT_SIZE_SMALL)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.secondary.on),
            });

        // ═══════════════════════════════════════════════════════════════════════
        // Combine all sections
        // ═══════════════════════════════════════════════════════════════════════
        let content_column = column![path_row, alpha_row, Space::new().height(DIALOG_SPACING), info_text,].spacing(DIALOG_SPACING);

        let content_box = effect_box(content_column.into());

        let can_apply = self.is_valid();

        // Clear button (only enabled if there's a path)
        let has_path = !self.path.is_empty();
        let clear_button = secondary_button(
            fl!("reference-image-clear"),
            has_path.then(|| msg(AnsiEditorMessage::ReferenceImageDialog(ReferenceImageDialogMessage::Clear))),
        );

        // Button row with Clear on left, Cancel/OK on right
        let button_row_content = row![
            clear_button,
            Space::new().width(Length::Fill),
            secondary_button(
                format!("{}", ButtonType::Cancel),
                Some(msg(AnsiEditorMessage::ReferenceImageDialog(ReferenceImageDialogMessage::Cancel))),
            ),
            primary_button(
                format!("{}", ButtonType::Ok),
                can_apply.then(|| msg(AnsiEditorMessage::ReferenceImageDialog(ReferenceImageDialogMessage::Apply))),
            ),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let dialog_content = dialog_area(column![title, Space::new().height(DIALOG_SPACING), content_box].into());

        let button_area = dialog_area(button_row_content.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area].into(),
            DIALOG_WIDTH_MEDIUM,
        )
        .into()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        let Message::AnsiEditor(AnsiEditorMessage::ReferenceImageDialog(msg)) = message else {
            return None;
        };

        match msg {
            ReferenceImageDialogMessage::SetPath(p) => {
                self.path = p.clone();
                Some(DialogAction::None)
            }
            ReferenceImageDialogMessage::Browse => {
                // Open file dialog - this will be handled by the main window
                // which will send FileSelected back to us
                Some(DialogAction::RunTask(Task::perform(
                    async {
                        let handle = rfd::AsyncFileDialog::new()
                            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "bmp", "webp"])
                            .add_filter("All files", &["*"])
                            .pick_file()
                            .await;
                        handle.map(|h| h.path().to_path_buf())
                    },
                    |result| {
                        if let Some(path) = result {
                            Message::AnsiEditor(AnsiEditorMessage::ReferenceImageDialog(ReferenceImageDialogMessage::FileSelected(path)))
                        } else {
                            // No file selected, just update nothing
                            Message::AnsiEditor(AnsiEditorMessage::ReferenceImageDialog(ReferenceImageDialogMessage::SetPath(String::new())))
                        }
                    },
                )))
            }
            ReferenceImageDialogMessage::FileSelected(path) => {
                self.path = path.to_string_lossy().to_string();
                Some(DialogAction::None)
            }
            ReferenceImageDialogMessage::SetAlpha(a) => {
                self.alpha = *a;
                Some(DialogAction::None)
            }
            ReferenceImageDialogMessage::Clear => {
                // Clear the reference image and close the dialog
                Some(DialogAction::CloseWith(Message::AnsiEditor(AnsiEditorMessage::Core(
                    AnsiEditorCoreMessage::ClearReferenceImage,
                ))))
            }
            ReferenceImageDialogMessage::Apply => {
                if let Some(path) = self.parsed_path() {
                    Some(DialogAction::CloseWith(Message::AnsiEditor(AnsiEditorMessage::Core(
                        AnsiEditorCoreMessage::ApplyReferenceImage(path, self.alpha),
                    ))))
                } else {
                    Some(DialogAction::None)
                }
            }
            ReferenceImageDialogMessage::Cancel => Some(DialogAction::Close),
        }
    }

    fn request_cancel(&mut self) -> DialogAction<Message> {
        DialogAction::Close
    }

    fn request_confirm(&mut self) -> DialogAction<Message> {
        if self.is_valid() {
            if let Some(path) = self.parsed_path() {
                return DialogAction::CloseWith(Message::AnsiEditor(AnsiEditorMessage::Core(AnsiEditorCoreMessage::ApplyReferenceImage(
                    path, self.alpha,
                ))));
            }
        }
        DialogAction::None
    }
}
